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
        source: "/marketing-assets/_next/static/:path*",
        destination: "/_next/static/:path*",
      },
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
