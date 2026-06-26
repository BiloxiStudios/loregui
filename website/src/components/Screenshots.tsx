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
 * A captioned surface in the feature gallery. Each shows one surface by default
 * and crossfades to a second shot on hover.
 *
 * - FUNCTION FLIP: `light` is a DIFFERENT (but related) feature → provide
 *   `captionHover` so the title + description flip to describe what's now on
 *   screen. We pair the second feature's LIGHT shot so the flip doubles as a
 *   live "fully themeable" demo. (e.g. storage → hosting, locks → dependencies.)
 * - THEME SWAP: `light` is the SAME surface re-themed → omit `captionHover`;
 *   the image swaps light/dark and the caption stays put.
 *
 * MOST cards are function flips; two stay pure theme swaps (the full-width main
 * view that opens the gallery and the theme editor that closes it) so the
 * "every pixel is a semantic token" story still reads.
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
    // THEME SWAP — opens the gallery: the whole app, re-themed, same layout.
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
      body: "The whole repository in one window — pick a branch, stage and commit changes, and read the history without ever touching the command line. Hover to re-theme it light.",
    },
  },
  {
    // FUNCTION FLIP — branch management (dark) flips to that branch's revision
    // history (light): commit on a branch, then read and revert its history.
    windowTitle: "LoreGUI — Branches → history",
    hint: "Then: its history",
    dark: {
      src: "/screenshots/panel-branches-dark.png",
      alt: "LoreGUI branches panel in the dark theme: the branch list with create, switch, protect and archive actions, plus a guided merge flow.",
    },
    light: {
      src: "/screenshots/panel-history-light.png",
      alt: "LoreGUI history panel in the light theme, listing revisions with diff and revert actions.",
    },
    caption: {
      title: "Branches & merging",
      body: "Create, protect, archive and reset branches — then drive a guided three-way merge with conflict resolution built in.",
    },
    captionHover: {
      title: "Revisions & diff",
      body: "Hover to follow that branch into its history: walk every revision, compare any two side by side, and revert a change in a single click.",
    },
  },
  {
    // FUNCTION FLIP — the storage-backend picker (dark) flips to the real
    // Windows "Host Server" wizard (light): configure where the repo lives,
    // then serve it.
    windowTitle: "LoreGUI — Storage → self-hosting",
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
    // FUNCTION FLIP — file locking (dark) flips to dependency tracking (light):
    // two halves of safely editing big binaries on a team.
    windowTitle: "LoreGUI — Locks → dependencies",
    hint: "See dependencies",
    dark: {
      src: "/screenshots/panel-locks-dark.png",
      alt: "LoreGUI locks panel in the dark theme: held locks with owner and path filters, file status checks, and an acquire-locks form.",
    },
    light: {
      src: "/screenshots/panel-dependencies-light.png",
      alt: "LoreGUI dependencies panel in the light theme: view the files a path depends on, follow transitive edges, or reverse the query to list dependents.",
    },
    caption: {
      title: "File locks",
      body: "Claim an exclusive lock on the binaries you're about to edit, so two people never overwrite the same asset — see who holds what, and when.",
    },
    captionHover: {
      title: "Dependency tracking",
      body: "Hover to map how assets reference each other: know exactly what a mesh or texture depends on — and what would break — before you change it.",
    },
  },
  {
    // FUNCTION FLIP — the full command list (dark) flips to a live fuzzy search
    // (light): open the palette, then type to run any of 100+ operations.
    windowTitle: "LoreGUI — ⌘K → run anything",
    hint: "Type to search",
    dark: {
      src: "/screenshots/palette-dark.png",
      alt: "LoreGUI command palette in the dark theme, freshly opened and listing every operation in the app — over a hundred commands.",
    },
    light: {
      src: "/screenshots/palette-query-light.png",
      alt: "The LoreGUI command palette in the light theme, narrowed by a fuzzy search for 'branch' to just the matching operations.",
    },
    caption: {
      title: "One palette, every command",
      body: "Press ⌘K anywhere to open a single palette that lists every operation in the app — over a hundred of them, no menu-hunting.",
    },
    captionHover: {
      title: "Fuzzy-find & run",
      body: "Hover to start typing: the list narrows instantly — branch, lock, push, merge — and Enter runs it. Power-user speed, nothing to memorise.",
    },
  },
  {
    // THEME SWAP — closes the gallery: the editor that themes the whole app,
    // shown re-theming itself. Full width to bookend the main-view opener.
    windowTitle: "LoreGUI — Theme editor",
    className: "lg:col-span-2",
    sizes: "(min-width: 1024px) 1024px, 100vw",
    hint: "Re-theme it live",
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
            <span aria-hidden="true">✨</span>
            Hover (or tap) any window — most reveal a related feature, and the
            rest flip to the light theme, since every pixel is a semantic token.
          </p>
        </div>

        {/* Hero shot: the command palette — dark by default, light on hover. */}
        <div className="relative mx-auto mt-16 max-w-5xl">
          <ThemeSwapShot
            windowTitle="LoreGUI — ⌘K command palette"
            priority
            sizes="(min-width: 1024px) 1024px, 100vw"
            dark={{
              src: "/screenshots/palette-query-dark.png",
              alt: "LoreGUI command palette in the dark theme, open with a fuzzy search for 'branch', listing matching operations.",
            }}
            light={{
              src: "/screenshots/palette-query-light.png",
              alt: "The same LoreGUI command palette and fuzzy search rendered in the light theme.",
            }}
          >
            <p className="mt-4 text-center text-sm text-brand-muted">
              Press{" "}
              <kbd className="rounded border border-brand-muted/30 bg-brand-surface-light px-1.5 py-0.5 font-mono text-xs text-brand-text">
                ⌘K
              </kbd>{" "}
              to fuzzy-search and run any operation in the app.
            </p>
          </ThemeSwapShot>
          <div
            className="pointer-events-none absolute -inset-4 -z-10 rounded-xl bg-vapor-pink/10 blur-2xl"
            aria-hidden="true"
          />
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
