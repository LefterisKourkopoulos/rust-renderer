"use client";

import { createContext, useContext, useEffect, useRef, useState } from "react";
import type { RendererHandle, RustRendererModule } from "@/lib/wasm-types";

export type RendererStatus = "idle" | "loading" | "ready" | "error";

export interface RendererContextValue {
  status: RendererStatus;
  error: string | null;
  loadGlb(bytes: Uint8Array, fileName: string): void;
  setSkybox(bytes: Uint8Array): void;
  setTimeOfDay(hour: number): void;
}

export const RendererContext = createContext<RendererContextValue | null>(null);

export function useRenderer(): RendererContextValue {
  const value = useContext(RendererContext);
  if (!value) {
    throw new Error("useRenderer must be used within a RendererProvider");
  }
  return value;
}

// Module-level cache so React Strict Mode's dev-time double effect invocation (and any other
// re-mount) reuses the same in-flight/completed init instead of spinning up a second wasm
// module, event loop, and GPU device.
let initPromise: Promise<RendererHandle> | null = null;

function initRenderer(canvasId: string): Promise<RendererHandle> {
  if (!initPromise) {
    initPromise = (async () => {
      const mod = (await import(
        /* webpackIgnore: true */ "/wasm/rust_renderer.js"
      )) as RustRendererModule;
      await mod.default();
      return mod.RendererHandle.init(canvasId);
    })();
  }
  return initPromise;
}

export function useRendererInit(canvasId: string): RendererContextValue {
  const [status, setStatus] = useState<RendererStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const handleRef = useRef<RendererHandle | null>(null);

  useEffect(() => {
    if (!("gpu" in navigator)) {
      setStatus("error");
      setError("This browser does not support WebGPU. Try a recent Chrome or Edge.");
      return;
    }

    setStatus("loading");
    initRenderer(canvasId)
      .then((handle) => {
        handleRef.current = handle;
        setStatus("ready");
      })
      .catch((e) => {
        setStatus("error");
        setError(String(e));
      });
  }, [canvasId]);

  return {
    status,
    error,
    loadGlb(bytes, fileName) {
      handleRef.current?.loadGlb(bytes, fileName);
    },
    setSkybox(bytes) {
      handleRef.current?.setSkybox(bytes);
    },
    setTimeOfDay(hour) {
      handleRef.current?.setTimeOfDay(hour);
    },
  };
}
