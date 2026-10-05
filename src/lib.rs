#![no_std]

use aidoku::{
    Chapter, DeepLinkHandler, DeepLinkResult, FilterValue, Listing, ListingProvider, Manga, MangaPageResult, Page,
    PageContent, Result, Source,
    alloc::{String, Vec, string::ToString},
    imports::{net::Request, html::Document, js::JsContext},
    prelude::*,
};

const BASE_URL: &str = "https://inkstory.net";
const USER_AGENT: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Version/17.0 Mobile/15E148 Safari/604.1";
const PER_PAGE: usize = 40;

fn get(url: &str) -> Result<aidoku::imports::net::Response> {
    Ok(Request::get(url)?
        .header("User-Agent", USER_AGENT)
        .header("Accept-Language", "ru-RU,ru;q=0.9,en;q=0.7")
        .header("Referer", BASE_URL)
        .send()?)
}

fn abs_url(value: &str) -> String {
    if value.starts_with("//") {
        format!("https:{value}")
    } else if value.starts_with("http://") || value.starts_with("https://") {
        value.to_string()
    } else if value.starts_with('/') {
        format!("{BASE_URL}{value}")
    } else {
        format!("{BASE_URL}/{value}")
    }
}

// Return a path only for InkStory URLs or root-relative links.
fn site_path(url: &str) -> Option<&str> {
    let path = if url.starts_with('/') && !url.starts_with("//") {
        url
    } else {
        let path = url.strip_prefix(BASE_URL)?;
        if !path.starts_with('/') {
            return None;
        }
        path
    };
    Some(path.split(['?', '#']).next().unwrap_or(path))
}

fn valid_chapter_id(id: &str) -> bool {
    id.len() == 36 && id.bytes().enumerate().all(|(i, c)| {
        if matches!(i, 8 | 13 | 18 | 23) { c == b'-' } else { c.is_ascii_hexdigit() }
    })
}

fn server_data(html: &Document, mode: &str, slug: &str, chapter_id: &str) -> Result<String> {
    let context = JsContext::new();
    context.eval(include_str!("bootstrap.js"))?;
    let mut loaded = false;
    for script in html.select("script").into_iter().flatten() {
        let source = script.data().unwrap_or_default();
        if source.contains("$_TSR.router=") || source.contains("($R=>") {
            context.eval(&format!("{source};'ok';"))?;
            loaded = true;
        }
    }
    if !loaded {
        bail!("InkStory: данные сайта не найдены. Откройте страницу в браузере и проверьте доступность.");
    }
    context.eval(include_str!("extract.js"))?;
    let args = serde_json::to_string(&(mode, slug, chapter_id))?;
    match context.eval(&format!("inkstoryExtract.apply(null, {args})")) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => { bail!("InkStory: список глав или страницы недоступны. Проверьте главу на сайте."); }
    }
}

fn parse_catalog(url: &str) -> Result<MangaPageResult> {
    let html = get(url)?.get_html()?;
    let mut entries: Vec<Manga> = Vec::new();

    for a in html.select("a[href*='/content/']").into_iter().flatten() {
        let Some(raw_href) = a.attr("href") else { continue };

        let href = abs_url(&raw_href);
        let Some(path) = site_path(&href) else { continue };
        let parts: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();

        // Catalog entries are /content/<slug>.
        if parts.len() != 2 || parts[0] != "content" {
            continue;
        }

        let Some(img) = a.select_first("img") else { continue };
        let title = img.attr("alt").filter(|s| !s.trim().is_empty())
            .or_else(|| a.text()).unwrap_or_default().trim().to_string();
        if title.is_empty() || title == "Каталог" {
            continue;
        }

        let key = parts[1].to_string();
        if entries.iter().any(|m: &Manga| m.key == key) {
            continue;
        }

        let cover = img.attr("abs:src").filter(|s| !s.is_empty())
            .or_else(|| img.attr("src"))
            .map(|x| abs_url(&x));

        entries.push(Manga {
            key,
            title,
            cover,
            url: Some(href),
            ..Default::default()
        });

    }

    let has_next_page = entries.len() >= PER_PAGE;

    Ok(MangaPageResult {
        entries,
        has_next_page,
    })
}

fn parse_detail(mut manga: Manga, needs_details: bool, needs_chapters: bool) -> Result<Manga> {
    let url = format!("{BASE_URL}/content/{}", manga.key);
    let html = get(&url)?.get_html()?;

    if needs_details {
        if let Some(h1) = html.select_first("h1") {
            if let Some(text) = h1.text() {
                let text = text.trim();
                if !text.is_empty() {
                    manga.title = text.to_string();
                }
            }
        }

        if let Some(img) = html
            .select_first("img[alt*='Постер']")
            .or_else(|| html.select_first("img"))
        {
            if let Some(src) = img.attr("abs:src").or_else(|| img.attr("src")) {
                manga.cover = Some(abs_url(&src));
            }
        }

        // On current InkStory pages the description is the paragraph after
        // the "Описание" heading. Fall back to the first suitable paragraph.
        let description = html
            .select_first("h2")
            .filter(|h| h.text().is_some_and(|t| t.trim() == "Описание"))
            .and_then(|h| h.parent())
            .and_then(|p| p.select_first("p"))
            .and_then(|p| p.text())
            .or_else(|| {
                html.select("p")
                    .into_iter()
                    .flatten()
                    .filter_map(|p| p.text())
                    .map(|t| t.trim().to_string())
                    .find(|t| !t.is_empty())
            });

        if let Some(description) = description {
            manga.description = Some(description);
        }

        manga.url = Some(url.clone());
    }

    if needs_chapters {
        let chapters_url = format!("{url}/chapters");
        let chapters_html = get(&chapters_url)?.get_html()?;
        let data = server_data(&chapters_html, "chapters", &manga.key, "")?;
        let chapters: Vec<Chapter> = serde_json::from_str(&data)?;

        manga.chapters = Some(chapters);
    }

    Ok(manga)
}

fn parse_pages(slug: &str, chapter_id: &str) -> Result<Vec<Page>> {
    // Reject stale v1 entries such as /comments before making any request.
    if !valid_chapter_id(chapter_id) {
        bail!("InkStory: устаревшая запись главы. Обновите список глав тайтла.");
    }
    let slug_encoded = aidoku::helpers::uri::encode_uri_component(slug);
    let url = format!("{BASE_URL}/content/{slug_encoded}/{chapter_id}");
    let html = get(&url)?.get_html()?;
    let data = server_data(&html, "pages", slug, chapter_id)?;
    let image_urls: Vec<String> = serde_json::from_str(&data)?;
    if image_urls.is_empty() {
        bail!("InkStory: глава не содержит доступных изображений.");
    }
    Ok(image_urls.into_iter().map(|src| Page {
        content: PageContent::url(src),
        ..Default::default()
    }).collect())
}

struct InkStory;

impl Source for InkStory {
    fn new() -> Self {
        Self
    }

    fn get_search_manga_list(
        &self,
        query: Option<String>,
        page: i32,
        _filters: Vec<FilterValue>,
    ) -> Result<MangaPageResult> {
        let page = page.max(1) - 1;
        let q = query.unwrap_or_default();
        let encoded = aidoku::helpers::uri::encode_uri_component(&q);

        let url = if encoded.is_empty() {
            format!("{BASE_URL}/content?page={page}&size={PER_PAGE}")
        } else {
            format!("{BASE_URL}/content?search={encoded}&page={page}&size={PER_PAGE}")
        };

        parse_catalog(&url)
    }

    fn get_manga_update(
        &self,
        manga: Manga,
        needs_details: bool,
        needs_chapters: bool,
    ) -> Result<Manga> {
        parse_detail(manga, needs_details, needs_chapters)
    }

    fn get_page_list(&self, manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
        parse_pages(&manga.key, &chapter.key)
    }
}

impl ListingProvider for InkStory {
    fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
        let sort = match listing.id.as_str() {
            "popular" => "viewsCount",
            "latest" => "createdAt",
            _ => { bail!("InkStory: неизвестный раздел"); }
        };
        let page = page.max(1) - 1;
        parse_catalog(&format!("{BASE_URL}/content?sort={sort}&orderBy=desc&page={page}&size={PER_PAGE}"))
    }
}

impl DeepLinkHandler for InkStory {
    fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
        if !url.starts_with(BASE_URL) {
            return Ok(None);
        }

        let Some(path) = site_path(&url) else { return Ok(None) };
        let parts: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();

        if parts.len() < 2 || parts[0] != "content" {
            return Ok(None);
        }

        Ok(Some(DeepLinkResult::Manga {
            key: parts[1].to_string(),
        }))
    }
}

register_source!(InkStory, ListingProvider, DeepLinkHandler);
