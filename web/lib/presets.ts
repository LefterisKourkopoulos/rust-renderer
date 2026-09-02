export interface ModelPreset {
  name: string;
  url: string;
}

export interface SkyboxPreset {
  name: string;
  url: string;
}

// Presets are fetched client-side and passed as bytes into the same loadGlb/setSkybox calls a
// user upload would use, so "preset" and "uploaded" are the same Rust-side operation.
export const MODEL_PRESETS: ModelPreset[] = [
  { name: "Cube diorama", url: "/models/cube_diorama.glb" },
  { name: "Cube", url: "/models/cube.glb" },
];

export const SKYBOX_PRESETS: SkyboxPreset[] = [
  { name: "Pure sky", url: "/skyboxes/pure-sky-hdri.jpg" },
  { name: "Dusk", url: "/skyboxes/dusk-hdri.jpeg" },
];

export async function fetchBytes(url: string): Promise<Uint8Array> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}
