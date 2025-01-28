{
    if (typeof __turbopack_load__ === 'function') {
        const orig = __turbopack_load__;
        __turbopack_load__ = function __turbopack_load__(...args) {
            return $$trackDynamicImport__(orig(...args));
        };
    }
}import { trackDynamicImport as $$trackDynamicImport__ } from 'private-next-rsc-track-dynamic-import';
export default async function Page() {
    await __turbopack_load__('some-chunk');
    return null;
}
