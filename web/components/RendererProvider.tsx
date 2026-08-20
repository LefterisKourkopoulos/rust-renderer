"use client";

import { RendererContext, useRendererInit } from "@/hooks/useRendererHandle";

export const CANVAS_ID = "rust-renderer-canvas";

export function RendererProvider({ children }: { children: React.ReactNode }) {
  const value = useRendererInit(CANVAS_ID);

  return <RendererContext.Provider value={value}>{children}</RendererContext.Provider>;
}
