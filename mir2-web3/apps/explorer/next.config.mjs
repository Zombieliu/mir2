/** @type {import('next').NextConfig} */
const nextConfig = {
  basePath: "/explore",
  poweredByHeader: false,
  reactStrictMode: true,
  allowedDevOrigins: ["127.0.0.1", "localhost"],
  async redirects() {
    return [{ source: "/", destination: "/zh-CN", permanent: false }];
  },
};

export default nextConfig;
