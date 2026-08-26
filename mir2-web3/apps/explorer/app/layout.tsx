import type { Metadata, Viewport } from "next";
import type { ReactNode } from "react";
import "./globals.css";

export const metadata: Metadata = {
  metadataBase: new URL(process.env.NEXT_PUBLIC_SITE_URL ?? "http://localhost:3090"),
  title: "NUMERON ATLAS · 传奇重生天机阁",
  description: "探索传奇重生的角色、公会、装备、市场与世界事件。",
  openGraph: {
    type: "website",
    title: "NUMERON ATLAS · 传奇重生天机阁",
    description: "探索传奇重生的角色、公会、装备、市场与世界事件。",
    siteName: "NUMERON ATLAS",
    images: [{ url: "/og.png", width: 1200, height: 630, alt: "NUMERON ATLAS · Legend of Rebirth · 传奇重生天机阁" }],
  },
  twitter: {
    card: "summary_large_image",
    title: "NUMERON ATLAS · 传奇重生天机阁",
    description: "探索传奇重生的角色、公会、装备、市场与世界事件。",
    images: ["/og.png"],
  },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#080b0c",
  colorScheme: "dark",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
