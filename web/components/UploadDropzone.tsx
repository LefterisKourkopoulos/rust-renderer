"use client";

import { useRef, useState } from "react";
import { useRenderer } from "@/hooks/useRendererHandle";

const MAX_BYTES = 100 * 1024 * 1024;

export function UploadDropzone() {
  const { loadGlb, status } = useRenderer();
  const inputRef = useRef<HTMLInputElement>(null);
  const [dragging, setDragging] = useState(false);
  const [warning, setWarning] = useState<string | null>(null);

  async function handleFile(file: File) {
    if (!file.name.toLowerCase().endsWith(".glb")) {
      setWarning(`${file.name} is not a .glb file`);
      return;
    }
    if (file.size > MAX_BYTES) {
      setWarning(`${file.name} is ${(file.size / 1024 / 1024).toFixed(1)} MB; uploads over 100 MB may stall the browser tab`);
    } else {
      setWarning(null);
    }
    const bytes = new Uint8Array(await file.arrayBuffer());
    loadGlb(bytes, file.name);
  }

  return (
    <div
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        const file = e.dataTransfer.files[0];
        if (file) void handleFile(file);
      }}
      onClick={() => inputRef.current?.click()}
      style={{
        border: `2px dashed ${dragging ? "#6cf" : "#555"}`,
        borderRadius: 8,
        padding: 16,
        textAlign: "center",
        cursor: status === "ready" ? "pointer" : "not-allowed",
        opacity: status === "ready" ? 1 : 0.5,
      }}
    >
      <input
        ref={inputRef}
        type="file"
        accept=".glb"
        style={{ display: "none" }}
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) void handleFile(file);
          e.target.value = "";
        }}
      />
      <p style={{ margin: 0 }}>Drop a .glb file here, or click to browse</p>
      {warning && <p style={{ color: "#f99", margin: "8px 0 0" }}>{warning}</p>}
    </div>
  );
}
