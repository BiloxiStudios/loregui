import type { Metadata } from "next";
import { Analytics } from '@vercel/analytics/next';
import "./globals.css";

const SITE_URL = "https://loregui.com";

export const metadata: Metadata = {
  title: "LoreGUI — A beautiful desktop GUI for Lore version control",
  description:
    "LoreGUI is a fast, cross-platform desktop client for Lore, Epic's next-generation version control for code and huge binary assets. Visual branch, merge, diff, and file locking — no command line required.",
  keywords: [
    "Lore",
    "version control",
    "Epic Games",
    "game development",
    "binary assets",
    "BLAKE3",
    "file locking",
    "desktop GUI",
    "VCS client",
  ],
  authors: [{ name: "Biloxi Studios" }],
  openGraph: {
    title: "LoreGUI — A beautiful desktop GUI for Lore",
    description:
      "A fast, cross-platform desktop client for Lore — Epic's next-generation version control for code and huge binary assets.",
    url: SITE_URL,
    siteName: "LoreGUI",
    type: "website",
    locale: "en_US",
    images: [
      {
        url: "/og-image.svg",
        width: 1200,
        height: 630,
        alt: "LoreGUI — a desktop GUI for Lore version control",
      },
    ],
  },
  twitter: {
    card: "summary_large_image",
    title: "LoreGUI — A beautiful desktop GUI for Lore",
    description:
      "A fast, cross-platform desktop client for Lore — version control for code and huge binary assets.",
    images: ["/og-image.svg"],
  },
  metadataBase: new URL(SITE_URL),
  robots: {
    index: true,
    follow: true,
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <head>
        <link rel="preconnect" href="https://fonts.googleapis.com" />
        <link
          rel="preconnect"
          href="https://fonts.gstatic.com"
          crossOrigin="anonymous"
        />
        <link
          href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&family=Space+Grotesk:wght@400;500;600;700&display=swap"
          rel="stylesheet"
        />
      </head>
      <body className="antialiased">
        {/* BiloxiStudios retrowave perspective grid */}
        <div
          className="pointer-events-none fixed inset-0 -z-20 opacity-20"
          aria-hidden="true"
          style={{
            backgroundImage: `
              linear-gradient(transparent 0%, #B967FF 2%, transparent 5%),
              linear-gradient(90deg, transparent 0%, #01CDFE 2%, transparent 5%)
            `,
            backgroundSize: "40px 40px",
            transform:
              "perspective(500px) rotateX(60deg) translateY(-100px) translateZ(-200px)",
            transformOrigin: "top center",
            height: "200vh",
          }}
        />
        <div className="pointer-events-none fixed inset-0 -z-10 bg-black/50" />
        {children}
        <Analytics />
      </body>
    </html>
  );
}
