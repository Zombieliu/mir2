import type { Metadata } from "next";
import { getAdminI18n } from "../lib/i18n";
import "./globals.css";

export const metadata: Metadata = {
  title: "Mir2 Aether Ops",
  description: "Production operations console for the Mir2 MMORPG foundation"
};

export default async function RootLayout({ children }: { children: React.ReactNode }) {
  const { locale } = await getAdminI18n();

  return (
    <html lang={locale}>
      <body>{children}</body>
    </html>
  );
}
