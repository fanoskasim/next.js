{
    if (typeof __webpack_load__ === 'function') {
        const orig = __webpack_load__;
        __webpack_load__ = function __webpack_load__(...args) {
            return $$trackDynamicImport__(orig(...args));
        };
    }
}import { trackDynamicImport as $$trackDynamicImport__ } from 'private-next-rsc-track-dynamic-import';
export default async function Page() {
    await __webpack_load__('some-chunk');
    return null;
}
