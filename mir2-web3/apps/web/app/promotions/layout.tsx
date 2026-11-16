import type { Metadata, Viewport } from "next";
import { R16_FULL_CJK_FONT_CSS } from "../../lib/r16-cjk-fonts";

export const metadata: Metadata = {
  title: "Promotion program · Numeron",
  description: "Numeron promotion applications, invitations and settlement review.",
  robots: { index: false, follow: false },
};

export const viewport: Viewport = {
  width: "device-width", initialScale: 1, maximumScale: 5,
  userScalable: true, colorScheme: "dark", themeColor: "#11120f",
};

export default function PromotionLayout({ children }: { children: React.ReactNode }) {
  return <>
    <style>{R16_FULL_CJK_FONT_CSS}</style>
    {children}
  </>;
}
