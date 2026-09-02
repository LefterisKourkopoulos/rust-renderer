import type { RendererHandleClass } from "./wasm-types";

declare const init: (wasmUrl?: string) => Promise<unknown>;
export default init;
export declare const RendererHandle: RendererHandleClass;
