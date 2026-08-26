/** @type {import('next').NextConfig} */
const explorerOrigin = (process.env.EXPLORER_ORIGIN ?? "http://127.0.0.1:3090").replace(/\/$/, "");

const nextConfig = {
  reactStrictMode: true,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  images: {
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
        source: "/zh-CN/explore",
        destination: `${explorerOrigin}/explore/zh-CN`,
      },
      {
        source: "/explore/:path*",
        destination: `${explorerOrigin}/explore/:path*`,
      },
    ];
  },
};

export default nextConfig;
