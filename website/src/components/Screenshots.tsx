import Image from "next/image";
import { Container } from "@/components/ui/Container";
import { Button } from "@/components/ui/Button";
import { AppWindow } from "@/components/mockups/AppWindow";
import { ArrowRightIcon } from "@/components/icons";

/** A captioned surface in the feature gallery. All shots are 1440×900. */
const surfaces: {
  src: string;
  alt: string;
  title: string;
  caption: string;
}[] = [
  {
    src: "/screenshots/main-view-dark.png",
    alt: "LoreGUI main view: branches on the left, staged and unstaged changes with a commit box in the center, and revision history on the right.",
    title: "Branches · Changes · History",
    caption:
      "The whole repository in one window — pick a branch, stage and commit changes, and read the history without ever touching the command line.",
  },
  {
    src: "/screenshots/panel-branches-dark.png",
    alt: "LoreGUI branches panel showing the branch list and a guided merge flow.",
    title: "Branches & merging",
    caption:
      "Create, protect, reset and archive branches, then drive a guided three-way merge with conflict resolution built in.",
  },
  {
    src: "/screenshots/panel-history-dark.png",
    alt: "LoreGUI history panel listing revisions with diffs and a revert action.",
    title: "Revisions & diff",
    caption:
      "Walk every revision, compare any two side by side, and cherry-pick or revert a change with a single action.",
  },
  {
    src: "/screenshots/panel-storage-dark.png",
    alt: "LoreGUI storage panel showing the configured backend and connectivity status.",
    title: "Storage backends",
    caption:
      "See which backend a repository is bound to and confirm connectivity at a glance — local disk, an S3 bucket, or a hosted server.",
  },
  {
    src: "/screenshots/panel-theme-dark.png",
    alt: "LoreGUI theme editor exposing semantic surface tokens for light and dark themes.",
    title: "Theme editor",
    caption:
      "Every surface is a semantic token. Build a theme, save it, and share it — the whole app re-themes instantly, light or dark.",
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
        </div>

        {/* Hero shot: the command palette */}
        <div className="relative mx-auto mt-16 max-w-5xl">
          <AppWindow title="LoreGUI — ⌘K command palette">
            <Image
              src="/screenshots/palette-query-dark.png"
              alt="LoreGUI command palette open with a fuzzy search for 'branch', listing matching operations."
              width={1440}
              height={900}
              className="w-full"
              priority
            />
          </AppWindow>
          <div
            className="pointer-events-none absolute -inset-4 -z-10 rounded-xl bg-vapor-pink/10 blur-2xl"
            aria-hidden="true"
          />
          <p className="mt-4 text-center text-sm text-brand-muted">
            Press{" "}
            <kbd className="rounded border border-brand-muted/30 bg-brand-surface-light px-1.5 py-0.5 font-mono text-xs text-brand-text">
              ⌘K
            </kbd>{" "}
            to fuzzy-search and run any operation in the app.
          </p>
        </div>

        {/* Captioned surface gallery */}
        <div className="mt-16 grid gap-8 lg:grid-cols-2">
          {surfaces.map((surface, i) => (
            <figure
              key={surface.src}
              className={i === 0 ? "lg:col-span-2" : ""}
            >
              <AppWindow title={`LoreGUI — ${surface.title}`}>
                <Image
                  src={surface.src}
                  alt={surface.alt}
                  width={1440}
                  height={900}
                  className="w-full"
                />
              </AppWindow>
              <figcaption className="mt-4">
                <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                  {surface.title}
                </h3>
                <p className="mt-1 text-sm leading-relaxed text-brand-muted">
                  {surface.caption}
                </p>
              </figcaption>
            </figure>
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
