import type { Metadata, Viewport } from "next";

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
  return children;
}
