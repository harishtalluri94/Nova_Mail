/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  swcMinify: true,
  output: 'standalone',

  env: {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080',
    NEXT_PUBLIC_JMAP_URL: process.env.NEXT_PUBLIC_JMAP_URL || 'http://localhost:8081',
  },

  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: `${process.env.NEXT_PUBLIC_API_URL}/api/:path*`,
      },
      {
        source: '/jmap/:path*',
        destination: `${process.env.NEXT_PUBLIC_JMAP_URL}/jmap/:path*`,
      },
    ];
  },
};

module.exports = nextConfig;
