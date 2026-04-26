import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Mir2 Aether Ops",
  description: "Production operations console for the Mir2 MMORPG foundation"
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>{children}</body>
    </html>
  );
}
