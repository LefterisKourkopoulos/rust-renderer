"use client";

import { useRenderer } from "@/hooks/useRendererHandle";
import { SKYBOX_PRESETS, fetchBytes } from "@/lib/presets";

export function SkyboxPicker() {
  const { setSkybox, status } = useRenderer();

  return (
    <div>
      <label style={{ display: "block", marginBottom: 4 }}>Skybox</label>
      <select
        disabled={status !== "ready"}
        defaultValue=""
        onChange={async (e) => {
          const preset = SKYBOX_PRESETS.find((p) => p.url === e.target.value);
          if (!preset) return;
          const bytes = await fetchBytes(preset.url);
          setSkybox(bytes);
        }}
        style={{ width: "100%" }}
      >
        <option value="" disabled>
          Choose a skybox...
        </option>
        {SKYBOX_PRESETS.map((preset) => (
          <option key={preset.url} value={preset.url}>
            {preset.name}
          </option>
        ))}
      </select>
    </div>
  );
}
