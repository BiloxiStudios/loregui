/**
 * Single source of truth for the /docs knowledge-base navigation.
 *
 * Drives the docs sidebar, the in-page search index, the "On this page" lists,
 * and the sitemap. Each page is one MDX file under `src/app/docs/<slug>/page.mdx`
 * (or the index at `src/app/docs/page.mdx`). Keep `href`s in sync with the
 * folder layout — there is no filesystem crawl at build time.
 */

export interface DocsPage {
  title: string;
  href: string;
  /** Short blurb used by search results and section landing cards. */
  description: string;
}

export interface DocsSection {
  title: string;
  pages: DocsPage[];
}

export const DOCS_NAV: DocsSection[] = [
  {
    title: "Getting started",
    pages: [
      {
        title: "Introduction",
        href: "/docs",
        description:
          "What LoreGUI is, how the docs are organised, and where to go next.",
      },
      {
        title: "Install & build",
        href: "/docs/install",
        description:
          "Download a signed installer or build LoreGUI from source with cargo tauri build.",
      },
      {
        title: "Connect to a server",
        href: "/docs/connect",
        description:
          "Join an existing Lore server: point LoreGUI at its address and sign in.",
      },
      {
        title: "Host a server",
        href: "/docs/host",
        description:
          "Stand up a new repository — pick a storage backend and serve it locally.",
      },
    ],
  },
  {
    title: "Concepts",
    pages: [
      {
        title: "The Lore mental model",
        href: "/docs/lore-model",
        description:
          "How Lore differs from git and Perforce: revisions, staging, fragments, partitions, shared stores, and locks.",
      },
      {
        title: "git / p4 → Lore",
        href: "/docs/git-p4-to-lore",
        description:
          "A translation table mapping the git and Perforce commands you know to Lore operations.",
      },
    ],
  },
  {
    title: "Using the app",
    pages: [
      {
        title: "The command palette",
        href: "/docs/command-palette",
        description:
          "Press ⌘K / Ctrl K to fuzzy-search and run any operation in the app.",
      },
      {
        title: "Panels & domains",
        href: "/docs/panels",
        description:
          "A tour of every surface: Changes, Branches, History, Locks, Storage, Dependencies, Account, and Manage.",
      },
      {
        title: "Theming",
        href: "/docs/theming",
        description:
          "The semantic surface-token model and how to build, save, and share a theme.",
      },
    ],
  },
  {
    title: "Automation",
    pages: [
      {
        title: "MCP & agent skills",
        href: "/docs/mcp",
        description:
          "Drive Lore from AI agents via the lore-mcp server and the bundled agent skills.",
      },
      {
        title: "Operation reference",
        href: "/docs/op-reference",
        description:
          "Every operation LoreGUI exposes — domain, op, arguments — generated from the palette manifests.",
      },
    ],
  },
  {
    title: "Ecosystem",
    pages: [
      {
        title: "VS Code extension",
        href: "/docs/vscode",
        description:
          "Lore Source Control in VS Code's native SCM panel — stage, commit, diff, history, and file locks from the editor.",
      },
      {
        title: "Unreal Engine plugin",
        href: "/docs/unreal",
        description:
          "Content-browser lock overlays, checkout = lock, check-in = commit — Lore inside Unreal. Coming soon.",
      },
    ],
  },
];

/** Flat, ordered list of every docs page — used for prev/next and search. */
export const DOCS_PAGES: DocsPage[] = DOCS_NAV.flatMap((s) => s.pages);
