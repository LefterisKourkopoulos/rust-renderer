import { RendererProvider } from "@/components/RendererProvider";
import { CanvasHost } from "@/components/CanvasHost";
import { UploadDropzone } from "@/components/UploadDropzone";
import { ScenePresetPicker } from "@/components/ScenePresetPicker";
import { SkyboxPicker } from "@/components/SkyboxPicker";
import { TimeOfDaySlider } from "@/components/TimeOfDaySlider";

export default function Home() {
  return (
    <RendererProvider>
      <main style={{ display: "flex", height: "100vh" }}>
        <CanvasHost />
        <aside
          style={{
            width: 320,
            padding: 16,
            display: "flex",
            flexDirection: "column",
            gap: 16,
            background: "#181818",
            overflowY: "auto",
          }}
        >
          <h1 style={{ fontSize: 18, margin: 0 }}>Rust Renderer</h1>
          <UploadDropzone />
          <ScenePresetPicker />
          <SkyboxPicker />
          <TimeOfDaySlider />
        </aside>
      </main>
    </RendererProvider>
  );
}
