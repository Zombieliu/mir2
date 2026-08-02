"use client";

import { useEffect } from "react";

export default function SpectateEntryPage() {
  useEffect(() => {
    const target = new URL("/", window.location.origin);
    for (const [key, value] of new URLSearchParams(window.location.search)) {
      target.searchParams.set(key, value);
    }
    target.searchParams.set("spectate", "1");
    window.location.replace(target.toString());
  }, []);

  return (
    <main
      style={{
        minHeight: "100vh",
        display: "grid",
        placeItems: "center",
        background: "#050b16",
        color: "#dff7ff",
        fontFamily: "sans-serif",
      }}
    >
      正在进入 Dubhe 世界观战台…
    </main>
  );
}
