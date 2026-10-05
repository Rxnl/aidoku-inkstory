// TanStack's server data needs these browser globals, but no DOM or network.
var self = globalThis;
var document = { currentScript: { remove: function () {} } };
var ReadableStream = function (options) {
    if (options.start) {
        options.start({ enqueue: function () {}, error: function () {}, close: function () {} });
    }
};
'ok';
