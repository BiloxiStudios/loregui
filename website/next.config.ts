import type { NextConfig } from "next";
import createMDX from "@next/mdx";

const nextConfig: NextConfig = {
  output: "standalone",
  // Allow `.mdx` (and `.md`) files to be treated as pages/components so the
  // /docs knowledge base can be authored in MDX alongside the existing TSX app.
  pageExtensions: ["ts", "tsx", "js", "jsx", "md", "mdx"],
  images: {
    formats: ["image/avif", "image/webp"],
  },
  async headers() {
    return [
      {
        source: "/(.*)",
        headers: [
          {
            key: "X-Frame-Options",
            value: "DENY",
          },
          {
            key: "X-Content-Type-Options",
            value: "nosniff",
          },
          {
            key: "Referrer-Policy",
            value: "strict-origin-when-cross-origin",
          },
        ],
      },
    ];
  },
};

const withMDX = createMDX({
  // Markdown/MDX options. Kept minimal — themed element mapping lives in
  // `mdx-components.tsx` (the App-Router MDX convention).
});

export default withMDX(nextConfig);
