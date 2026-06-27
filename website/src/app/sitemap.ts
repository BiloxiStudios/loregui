import type { MetadataRoute } from "next";
import { DOCS_PAGES } from "@/lib/docs-nav";

const SITE_URL = "https://loregui.com";

export default function sitemap(): MetadataRoute.Sitemap {
  const docs: MetadataRoute.Sitemap = DOCS_PAGES.map((p) => ({
    url: `${SITE_URL}${p.href}`,
    lastModified: new Date(),
    changeFrequency: "monthly",
    // The docs landing page ranks a little higher than the leaf pages.
    priority: p.href === "/docs" ? 0.8 : 0.6,
  }));

  return [
    {
      url: SITE_URL,
      lastModified: new Date(),
      changeFrequency: "weekly",
      priority: 1,
    },
    {
      url: `${SITE_URL}/guide`,
      lastModified: new Date(),
      changeFrequency: "monthly",
      priority: 0.8,
    },
    {
      url: `${SITE_URL}/premium`,
      lastModified: new Date(),
      changeFrequency: "monthly",
      priority: 0.7,
    },
    {
      url: `${SITE_URL}/docs/vscode`,
      lastModified: new Date(),
      changeFrequency: "monthly",
      priority: 0.7,
    },
    {
      url: `${SITE_URL}/docs/unreal`,
      lastModified: new Date(),
      changeFrequency: "monthly",
      priority: 0.6,
    },
    ...docs,
  ];
}
