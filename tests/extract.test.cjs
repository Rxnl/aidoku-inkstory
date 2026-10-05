const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const path = require('node:path');
const bootstrap = fs.readFileSync(path.join(__dirname, '../src/bootstrap.js'), 'utf8');
const extract = fs.readFileSync(path.join(__dirname, '../src/extract.js'), 'utf8');
const bookId = '0ef488a7-9cb1-4f7b-80f8-caabf5062afb';
const chapterId = 'c060c81c-a2b0-47c0-abd8-6fd21c664e44';
const image = 'https://static.inuko.me/chapters/' + chapterId + '/771ea621-6d9b-x7db-b088-b0bcf0f47511.jpeg';
const book = {id: bookId, slug: 'toukyou-kushu', type: 'COMIC', chaptersCount: 1};
const chapter = {id: chapterId, bookId, name: 'трагедия', number: 1, volume: 1};
function setup(rows) {
    const ctx = vm.createContext({});
    vm.runInContext(bootstrap, ctx);
    vm.runInContext('var $R = {tsr: ' + JSON.stringify(rows) + '};', ctx);
    vm.runInContext(extract, ctx);
    return {
        run: (mode, id = chapterId) => JSON.parse(vm.runInContext('inkstoryExtract.apply(null,' + JSON.stringify([mode, book.slug, id]) + ')', ctx)),
        image: url => vm.runInContext('inkstoryImageUrl(' + JSON.stringify(url) + ')', ctx)
    };
}
test('real chapters retain numbers and exclude navigation, other books and duplicates', () => {
    const c = setup([book, {chapters: [chapter, chapter, {...chapter, id:'comments'}, {...chapter, bookId:'other'}]}]);
    const entries = c.run('chapters');
    assert.equal(entries.length, 1); assert.equal(entries[0].chapter_number, 1);
    assert.equal(entries[0].volume_number, 1); assert.equal(entries[0].key, chapterId);
});
test('navigation alone cannot become chapters', () => {
    assert.throws(() => setup([book, {chapters:[{id:'comments', bookId}]}]).run('chapters'));
});
test('known empty books have no chapters', () => {
    assert.deepEqual(setup([{...book, chaptersCount:0}]).run('chapters'), []);
});
test('pages are ordered by index and encrypted image names become ordinary images', () => {
    const second = image.replace('771ea621', '771ea622');
    const c = setup([book, {...chapter, pages:[{index:1,image:second}, {index:0,image}]}]);
    assert.deepEqual(c.run('pages'), [image.replace('-x7db-', '-47db-'), second.replace('-x7db-', '-47db-')]);
});
test('s image marker and query string are handled without changing chapter id', () => {
    const c = setup([]), url = image.replace('-x7db-', '-s7db-') + '?width=1000';
    assert.equal(c.image(url), image.replace('-x7db-', '-47db-') + '?width=1000');
    assert.equal(c.image(image.replace('-x7db-', '-47db-')), image.replace('-x7db-', '-47db-'));
});
test('covers and publisher pictures cannot become pages', () => {
    assert.throws(() => setup([book, {...chapter, pages:[{index:0,image:'https://static.inuko.me/book/x/poster/a.jpg'}]}]).run('pages'));
});
test('a different chapter or book cannot supply pages', () => {
    const c = setup([book, {...chapter, pages:[{index:0,image}]}]);
    assert.throws(() => c.run('pages', '00000000-0000-0000-0000-000000000000'));
    assert.throws(() => setup([book, {...chapter, bookId:'other', pages:[{index:0,image}]}]).run('pages'));
});
test('stale navigation chapter keys are rejected', () => {
    for (const id of ['comments','reviews','discussions','media']) assert.throws(() => setup([book]).run('pages',id));
});
test('missing and duplicate pages are rejected', () => {
    assert.throws(() => setup([book, {...chapter, pages:[]}]).run('pages'));
    assert.throws(() => setup([book, {...chapter, pages:[{index:0,image},{index:0,image}]}]).run('pages'));
});

test('listing names retain correct Russian text independent of file encoding', () => {
    const metadata = JSON.parse(fs.readFileSync(path.join(__dirname, '../res/source.json'), 'utf8'));
    assert.equal(metadata.listings.find(x => x.id === 'popular').name, '\u041f\u043e\u043f\u0443\u043b\u044f\u0440\u043d\u043e\u0435');
    assert.equal(metadata.listings.find(x => x.id === 'latest').name, '\u041d\u043e\u0432\u0438\u043d\u043a\u0438');
});
