import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "mir2-web3 client",
  description: "Next.js + Bevy WASM client for mir2-web3",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
