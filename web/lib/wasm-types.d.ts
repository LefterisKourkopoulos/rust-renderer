
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
