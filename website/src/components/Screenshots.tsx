import { Container } from "@/components/ui/Container";
import { Button } from "@/components/ui/Button";
import { ThemeSwapShot } from "@/components/ThemeSwapShot";
import { ArrowRightIcon } from "@/components/icons";

interface Shot {
  src: string;
  alt: string;
}

interface Caption {
  title: string;
  body: string;
}

/**
 * A captioned surface in the feature gallery. Each shows the DARK theme by
 * default and crossfades to a LIGHT shot on hover.
 *
 * - If `light` is the SAME surface re-themed → omit `captionHover`; the image
 *   swaps and the caption stays.
 * - If `light` is a DIFFERENT object/function → provide `captionHover` so the
 *   title + description flip to describe what's now on screen.
 */
const surfaces: {
  windowTitle: string;
  dark: Shot;
  light: Shot;
  hint?: string;
  caption: Caption;
  captionHover?: Caption;
  className?: string;
  sizes?: string;
}[] = [
  {
    windowTitle: "LoreGUI — Branches · Changes · History",
    className: "lg:col-span-2",
    sizes: "(min-width: 1024px) 1024px, 100vw",
    dark: {
      src: "/screenshots/main-view-dark.png",
      alt: "LoreGUI main view in the dark theme: branches on the left, staged and unstaged changes with a commit box in the center, and revision history on the right.",
    },
    light: {
      src: "/screenshots/main-view-light.png",
      alt: "The same LoreGUI main view rendered in the light theme — identical layout, re-themed surfaces.",
    },
    caption: {
      title: "Branches · Changes · History",
      body: "The whole repository in one window — pick a branch, stage and commit changes, and read the history without ever touching the command line.",
    },
  },
  {
    windowTitle: "LoreGUI — Branches & Revisions",
    dark: {
      src: "/screenshots/panel-branches-dark.png",
      alt: "LoreGUI branches panel in the dark theme showing the branch list and a guided merge flow.",
    },
    light: {
      src: "/screenshots/panel-history-light.png",
      alt: "LoreGUI history panel in the light theme listing revisions with diffs and a revert action.",
    },
    caption: {
      title: "Branches & merging",
      body: "Create, protect, reset and archive branches, then drive a guided three-way merge with conflict resolution built in.",
    },
    captionHover: {
      title: "Revisions & diff",
      body: "Walk every revision, compare any two side by side, and cherry-pick or revert a change with a single action.",
    },
  },
  {
    // Different function on hover: the storage-backend picker (dark) flips to
    // the real Windows "Host Server" wizard (light), so the caption flips too.
    windowTitle: "LoreGUI — Storage & self-hosting",
    hint: "See it on Windows",
    dark: {
      src: "/screenshots/panel-storage-dark.png",
      alt: "LoreGUI storage panel in the dark theme: choose a backend — local packfiles, Amazon S3, MinIO or Garage.",
    },
    light: {
      src: "/screenshots/windows/cropped/hosting.png",
      alt: "The LoreGUI desktop app running on Windows in the light theme, showing the Host Server step with a live lore:// connection URL to share with a team.",
    },
    caption: {
      title: "Storage backends",
      body: "Point a repository at local packfiles, an S3 bucket, MinIO or Garage — and confirm connectivity before you ever commit.",
    },
    captionHover: {
      title: "Host your own server",
      body: "Hover to jump into the real Windows app: start a Lore server over that same store and share a lore:// URL so your whole team can clone and push.",
    },
  },
  {
    windowTitle: "LoreGUI — Locks & Dependencies",
    dark: {
      src: "/screenshots/panel-locks-dark.png",
      alt: "LoreGUI locks panel in the dark theme, showing acquired locks on binary assets.",
    },
    light: {
      src: "/screenshots/panel-dependencies-light.png",
      alt: "LoreGUI dependencies panel in the light theme, listing linked repositories and their versions.",
    },
    caption: {
      title: "Lock management",
      body: "Acquire and release locks on binary assets to prevent conflicts, and see who's working on what across the whole team.",
    },
    captionHover: {
      title: "Dependency tracking",
      body: "Manage external binary dependencies and linked repositories, ensuring everyone has the right assets for the right revision.",
    },
  },
  {
    windowTitle: "LoreGUI — Command palette & search",
    dark: {
      src: "/screenshots/palette-dark.png",
      alt: "The LoreGUI command palette in its default dark state.",
    },
    light: {
      src: "/screenshots/palette-query-light.png",
      alt: "The LoreGUI command palette in the light theme, showing a fuzzy search in progress.",
    },
    caption: {
      title: "Command palette",
      body: "Press ⌘K to open the universal command palette — every operation in the app is just a few keystrokes away.",
    },
    captionHover: {
      title: "Fuzzy search",
      body: "Fuzzy-search through actions, branches, and files to jump exactly where you need to go, instantly.",
    },
  },
  {
    windowTitle: "LoreGUI — Theme editor",
    hint: "Re-theme it live",
    className: "lg:col-span-2",
    sizes: "(min-width: 1024px) 1024px, 100vw",
    dark: {
      src: "/screenshots/panel-theme-dark.png",
      alt: "LoreGUI theme editor in the dark theme, exposing semantic surface tokens for light and dark variants.",
    },
    light: {
      src: "/screenshots/panel-theme-light.png",
      alt: "The same LoreGUI theme editor re-themed to the light variant — the editor themes itself.",
    },
    caption: {
      title: "Theme editor",
      body: "Every surface is a semantic token. Build a theme, save it, and share it — the whole app re-themes instantly, light or dark. (Hover this one and watch the editor re-theme itself.)",
    },
  },
];

export function Screenshots() {
  return (
    <section id="screens" className="py-20 sm:py-32">
      <Container>
        <div className="mx-auto max-w-2xl text-center">
          <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
            Your whole repo, made legible
          </h2>
          <p className="mt-4 text-lg text-brand-muted">
            One window for status, history and branches — plus a command palette
            that runs any operation. Purpose-built for projects where the
            binaries are bigger than the code.
          </p>
          <p className="mt-4 inline-flex items-center gap-2 rounded-full border border-brand-muted/20 bg-brand-surface-light/60 px-3.5 py-1.5 text-sm text-brand-muted">
            <span aria-hidden="true">☀️</span>
            Hover (or tap) any window to flip it into the light theme — every
            pixel is a semantic token.
          </p>
        </div>

        {/* Captioned surface gallery */}
        <div className="mt-16 grid gap-8 lg:grid-cols-2">
          {surfaces.map((surface) => (
            <ThemeSwapShot
              key={surface.dark.src}
              windowTitle={surface.windowTitle}
              dark={surface.dark}
              light={surface.light}
              hint={surface.hint}
              caption={surface.caption}
              captionHover={surface.captionHover}
              className={surface.className}
              sizes={surface.sizes}
            />
          ))}
        </div>

        <div className="mt-12 flex flex-col items-center justify-center gap-4 sm:flex-row">
          <p className="text-sm text-brand-muted">
            Real screenshots of the LoreGUI desktop app.
          </p>
          <Button variant="secondary" size="sm" href="/guide">
            Read the user guide
            <ArrowRightIcon className="ml-2 h-4 w-4" />
          </Button>
        </div>
      </Container>
    </section>
  );
}
