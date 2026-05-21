import type { Metadata } from "next";
import { AssetCacheRegistrar } from "./components/asset-cache-registrar";
import "./globals.css";

export const metadata: Metadata = {
  title: "mir2-web3 client",
  description: "Next.js + Bevy WASM client for mir2-web3",
  icons: {
    icon: "/favicon.svg",
  },
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>
        <AssetCacheRegistrar />
        {children}
      </body>
    </html>
  );
}
