// Same plain-image fallback as the site's links-rLopdA5L.js when its service
// worker is unavailable. The x/s marker is only in the image filename.
function inkstoryImageUrl(url) {
    return url.replace(/\/([0-9a-f]{8}-[0-9a-f]{4}-)[xs]([0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}\.[^/?#]+)(?=[?#]|$)/i, '/$14$2');
}

// Read only the requested book/chapter from the server's reference table.
function inkstoryExtract(mode, slug, chapterId) {
    var rows = typeof $R !== 'undefined' && $R.tsr;
    if (!Array.isArray(rows)) throw new Error('Missing server data');
    var uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
    var book = rows.find(function (x) {
        return x && x.slug === slug && uuid.test(x.id) && x.type === 'COMIC';
    });
    if (!book) throw new Error('Requested book not found');
    if (mode === 'chapters') {
        var container = rows.find(function (x) {
            return x && Array.isArray(x.chapters) && x.chapters.some(function (c) {
                return c && c.bookId === book.id && uuid.test(c.id);
            });
        });
        if (!container) {
            if (book.chaptersCount === 0) return '[]';
            throw new Error('Chapter list not found');
        }
        var seen = Object.create(null);
        return JSON.stringify(container.chapters.filter(function (c) {
            if (!c || c.bookId !== book.id || !uuid.test(c.id) || seen[c.id]) return false;
            seen[c.id] = true;
            return true;
        }).map(function (c) {
            var date = Date.parse(c.createdAt);
            return {
                key: c.id,
                title: typeof c.name === 'string' && c.name.trim() ? c.name : null,
                chapter_number: Number.isFinite(c.number) ? c.number : null,
                volume_number: Number.isFinite(c.volume) ? c.volume : null,
                date_uploaded: Number.isFinite(date) ? Math.floor(date / 1000) : null,
                scanlators: typeof c.publisherNames === 'string' && c.publisherNames ? [c.publisherNames] : null,
                url: 'https://inkstory.net/content/' + encodeURIComponent(slug) + '/' + c.id,
                language: 'ru', thumbnail: null, locked: c.donut === true
            };
        }));
    }
    if (mode !== 'pages' || !uuid.test(chapterId)) throw new Error('Invalid chapter');
    var chapter = rows.find(function (x) {
        return x && x.id === chapterId && x.bookId === book.id && Array.isArray(x.pages);
    });
    if (!chapter || !chapter.pages.length) throw new Error('Chapter unavailable or no image pages');
    var pages = chapter.pages.slice().sort(function (a, b) { return a.index - b.index; });
    var indices = Object.create(null);
    var images = Object.create(null);
    return JSON.stringify(pages.map(function (p) {
        // Covers and publisher avatars must never be accepted as reader pages.
        if (!Number.isInteger(p.index) || p.index < 0 || indices[p.index] ||
            typeof p.image !== 'string' || !/^https?:\/\//.test(p.image) ||
            p.image.indexOf('/chapters/' + chapterId + '/') < 0 || images[p.image]) {
            throw new Error('Invalid or duplicate chapter page');
        }
        indices[p.index] = true;
        images[p.image] = true;
        return inkstoryImageUrl(p.image);
    }));
}
'ok';
