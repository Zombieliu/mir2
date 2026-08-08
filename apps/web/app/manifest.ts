import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    id: "/",
    name: "Legend of Mir 2",
    short_name: "Mir 2",
    description: "Play Legend of Mir 2 from an installable browser game client.",
    start_url: "/",
    scope: "/",
    display: "fullscreen",
    orientation: "landscape",
    background_color: "#000000",
    theme_color: "#1f140a",
    categories: ["games", "entertainment"],
    icons: [
      {
        src: "/pwa/icon-192.png",
        sizes: "192x192",
        type: "image/png",
        purpose: "any",
      },
      {
        src: "/pwa/icon-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "any",
      },
      {
        src: "/pwa/icon-maskable-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
  };
}
