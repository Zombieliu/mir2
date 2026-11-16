/** @type {import('next').NextConfig} */
const explorerOrigin = (process.env.EXPLORER_ORIGIN ?? "https://numeron-explorer.vercel.app").replace(/\/$/, "");

const nextConfig = {
  reactStrictMode: true,
  assetPrefix: "/marketing-assets",
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  images: {
    unoptimized: true,
    remotePatterns: [
      {
        protocol: "https",
        hostname: "mir2.obelisk.build",
        pathname: "/bootstrap/**",
      },
    ],
  },
  async redirects() {
    return [
      { source: "/guide", destination: "/zh-CN/guide", permanent: false },
      { source: "/developers", destination: "/zh-CN/developers", permanent: false },
      { source: "/game", destination: "https://mir2.obelisk.build/game", permanent: false },
      { source: "/game/:path*", destination: "https://mir2.obelisk.build/game/:path*", permanent: false },
      {
        source: "/promotions",
        has: [{ type: "query", key: "lang", value: "(?<promotionLocale>zh-CN|en|ja|ko|zh-TW|es|pt-BR|de|fr|ru|pl|tr|th|vi|id|ar)" }],
        destination: "/:promotionLocale/promotions",
        permanent: false,
      },
      { source: "/promotions", destination: "/zh-CN/promotions", permanent: false },
      { source: "/promotions/:path*", destination: "https://mir2.obelisk.build/promotions/:path*", permanent: false },
      {
        source: "/",
        destination: "/zh-CN",
        permanent: false,
      },
    ];
  },
  async rewrites() {
    return [
      {
        source: "/:locale(zh-CN|en|ja|ko|zh-TW|es|pt-BR|de|fr|ru|pl|tr|th|vi|id|ar)/explore",
        destination: `${explorerOrigin}/explore/:locale`,
      },
      {
        source: "/:locale(zh-CN|en|ja|ko|zh-TW|es|pt-BR|de|fr|ru|pl|tr|th|vi|id|ar)/archive/:path*",
        destination: `${explorerOrigin}/explore/:locale/archive/:path*`,
      },
      {
        source: "/explore/:path*",
        destination: `${explorerOrigin}/explore/:path*`,
      },
    ];
  },
};

export default nextConfig;
