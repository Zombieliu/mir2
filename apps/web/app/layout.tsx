import type { Metadata, Viewport } from "next";
import { AssetCacheRegistrar } from "./components/asset-cache-registrar";
import { ChunkReloadGuard } from "./lib/chunk-reload-guard";
import { PwaGameShell } from "./components/pwa-game-shell";
import "./globals.css";
import "./pwa-game-shell.css";

export const metadata: Metadata = {
  title: "Legend of Mir 2",
  applicationName: "Legend of Mir 2",
  description: "Installable Legend of Mir 2 browser game client.",
  manifest: "/manifest.webmanifest",
  icons: {
    icon: "/favicon.png",
    apple: "/pwa/apple-touch-icon.png",
  },
  appleWebApp: {
    capable: true,
    statusBarStyle: "black-translucent",
    title: "Mir 2",
  },
  formatDetection: {
    telephone: false,
  },
  other: {
    google: "notranslate",
    "apple-mobile-web-app-capable": "yes",
  },
};

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  maximumScale: 1,
  userScalable: false,
  viewportFit: "cover",
  colorScheme: "dark",
  themeColor: "#1f140a",
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
        <PwaGameShell />
        {children}
      </body>
    </html>
  );
}
