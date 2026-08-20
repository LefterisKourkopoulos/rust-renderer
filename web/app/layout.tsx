import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Rust Renderer",
  description: "Upload glTF/GLB scenes and control them in a WebGPU renderer",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body style={{ margin: 0, background: "#111", color: "#eee", fontFamily: "sans-serif" }}>
        {children}
      </body>
    </html>
  );
}
