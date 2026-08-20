"use client";

import { useRenderer } from "@/hooks/useRendererHandle";
import { CANVAS_ID } from "@/components/RendererProvider";

export function CanvasHost() {
  const { status, error } = useRenderer();

  return (
    <div style={{ position: "relative", flex: 1, minHeight: 0 }}>
      <canvas id={CANVAS_ID} style={{ width: "100%", height: "100%", display: "block" }} />
      {status !== "ready" && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            background: "rgba(0,0,0,0.6)",
            pointerEvents: "none",
          }}
        >
          {status === "error" ? (error ?? "Something went wrong") : "Loading renderer..."}
        </div>
      )}
    </div>
  );
}
