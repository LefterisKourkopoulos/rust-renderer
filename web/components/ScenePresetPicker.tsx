"use client";

import { useRenderer } from "@/hooks/useRendererHandle";
import { MODEL_PRESETS, fetchBytes } from "@/lib/presets";

export function ScenePresetPicker() {
  const { loadGlb, status } = useRenderer();

  return (
    <div>
      <label style={{ display: "block", marginBottom: 4 }}>Preset scenes</label>
      <select
        disabled={status !== "ready"}
        defaultValue=""
        onChange={async (e) => {
          const preset = MODEL_PRESETS.find((p) => p.url === e.target.value);
          if (!preset) return;
          const bytes = await fetchBytes(preset.url);
          loadGlb(bytes, preset.name);
        }}
        style={{ width: "100%" }}
      >
        <option value="" disabled>
          Choose a preset...
        </option>
        {MODEL_PRESETS.map((preset) => (
          <option key={preset.url} value={preset.url}>
            {preset.name}
          </option>
        ))}
      </select>
    </div>
  );
}
