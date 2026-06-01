import type { Metadata } from "next";
import { AssetCacheRegistrar } from "./components/asset-cache-registrar";
import { ChunkReloadGuard } from "./lib/chunk-reload-guard";
import "./globals.css";

export const metadata: Metadata = {
  title: "mir2-web3 client",
  description: "Next.js + Bevy WASM client for mir2-web3",
  icons: {
    icon: "/favicon.svg",
  },
  other: {
    google: "notranslate",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en" translate="no" className="notranslate" suppressHydrationWarning>
      <body translate="no" className="notranslate" suppressHydrationWarning>
        <ChunkReloadGuard />
        <AssetCacheRegistrar />
        {children}
      </body>
    </html>
  );
}
