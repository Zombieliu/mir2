import type { ReactNode } from "react";
import "../globals.css";

export default function RedirectRootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="zh-CN">
      <body>{children}</body>
    </html>
  );
}
