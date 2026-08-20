// Hand-written shape of the wasm-bindgen `--target web` bundle produced by
// scripts/build-wasm.sh into public/wasm/rust_renderer.js. That file (and its real
// rust_renderer.d.ts) only exists after the wasm build runs, and public/ assets aren't part of
// the TypeScript module graph anyway, so this describes the contract by hand instead.

export interface RendererHandle {
  loadGlb(bytes: Uint8Array, fileName: string): void;
  setSkybox(bytes: Uint8Array): void;
  setTimeOfDay(hour: number): void;
}

export interface RendererHandleClass {
  init(canvasId: string): Promise<RendererHandle>;
}

export interface RustRendererModule {
  default: (wasmUrl?: string) => Promise<unknown>;
  RendererHandle: RendererHandleClass;
}
