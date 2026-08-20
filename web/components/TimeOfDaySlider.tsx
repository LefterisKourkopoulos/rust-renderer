"use client";

import { useRef, useState } from "react";
import { useRenderer } from "@/hooks/useRendererHandle";

const THROTTLE_MS = 32;

function formatHour(hour: number): string {
  const h = Math.floor(hour);
  const m = Math.round((hour - h) * 60);
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

export function TimeOfDaySlider() {
  const { setTimeOfDay, status } = useRenderer();
  const [hour, setHour] = useState(12);
  const lastSent = useRef(0);

  return (
    <div>
      <label style={{ display: "block", marginBottom: 4 }}>Time of day: {formatHour(hour)}</label>
      <input
        type="range"
        min={0}
        max={24}
        step={0.1}
        value={hour}
        disabled={status !== "ready"}
        onChange={(e) => {
          const value = Number(e.target.value);
          setHour(value);

          const now = performance.now();
          if (now - lastSent.current >= THROTTLE_MS) {
            lastSent.current = now;
            setTimeOfDay(value);
          }
        }}
        onMouseUp={(e) => setTimeOfDay(Number((e.target as HTMLInputElement).value))}
        style={{ width: "100%" }}
      />
    </div>
  );
}
