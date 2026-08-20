// Maps the runtime specifier used by the dynamic `import("/wasm/rust_renderer.js")` in
// hooks/useRendererHandle.ts to a type (see tsconfig.json's `paths`). TypeScript won't accept an
// ambient `declare module "/wasm/rust_renderer.js"` for a path-like specifier ("Ambient module
// declaration cannot specify relative module name"), so this shim file stands in for it instead.
// The real file only exists on disk after scripts/build-wasm.sh runs.

import type { RendererHandleClass } from "./wasm-types";

declare const init: (wasmUrl?: string) => Promise<unknown>;
export default init;
export declare const RendererHandle: RendererHandleClass;
