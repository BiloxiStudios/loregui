import Image from "next/image";
import { Container } from "@/components/ui/Container";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { GradientText } from "@/components/ui/GradientText";
import { AppWindow } from "@/components/mockups/AppWindow";

type Shot = {
  src: string;
  alt: string;
  title: string;
  /** Intrinsic width/height — every harness capture is 1440×900,
   *  native captures are 1400×900. */
  width?: number;
  height?: number;
};

type GuideSectionData = {
  id: string;
  step: string;
  heading: string;
  body: string;
  shots: Shot[];
  /** Lay shots out side by side (e.g. light vs dark theme). */
  sideBySide?: boolean;
};

const sections: GuideSectionData[] = [
  {
    id: "getting-started",
    step: "01",
    heading: "Getting started",
    body: "Launch LoreGUI and you land on “Choose Your Setup Mode.” Pick whether you’re joining an existing server or hosting your own — everything else is guided from there. No config files, no CLI bootstrap.",
    shots: [
      {
        src: "/screenshots/native-onboarding.png",
        alt: "LoreGUI first launch: the Choose Your Setup Mode welcome screen in the native desktop binary.",
        title: "First launch",
        width: 1400,
        height: 900,
      },
      {
        src: "/screenshots/onboarding-mode-select.png",
        alt: "LoreGUI onboarding: choosing between connecting as a client or hosting a server.",
        title: "Client or host",
      },
    ],
  },
  {
    id: "connect",
    step: "02",
    heading: "Connect to a server",
    body: "Joining a team? Point LoreGUI at your server’s address and sign in. It binds the connection in-process and pulls only the files you open — so even a multi-terabyte project is ready in seconds.",
    shots: [
      {
        src: "/screenshots/onboarding-client-step1.png",
        alt: "LoreGUI onboarding step for connecting to an existing server.",
        title: "Connect as a client",
      },
    ],
  },
  {
    id: "host",
    step: "03",
    heading: "Host a server",
    body: "Standing up a new repository? Choose a storage backend — local disk, an S3 bucket, or a hosted endpoint — and LoreGUI provisions and serves it. On Windows it can register a service so checkouts stay synced and autorun on boot.",
    shots: [
      {
        src: "/screenshots/onboarding-host-step1.png",
        alt: "LoreGUI onboarding host step: choosing a storage backend for a new server.",
        title: "Pick a storage backend",
      },
    ],
  },
  {
    id: "command-palette",
    step: "04",
    heading: "The command palette",
    body: "Press ⌘K (Ctrl K on Windows and Linux) to open the palette and fuzzy-search every operation in the app — branch, merge, commit, lock, revert and more. It’s the fastest path to any action, and every endpoint LoreGUI exposes lives here.",
    shots: [
      {
        src: "/screenshots/palette-query-dark.png",
        alt: "LoreGUI command palette open with a fuzzy search for 'branch' listing matching operations.",
        title: "⌘K — run anything",
      },
    ],
  },
  {
    id: "changes-history",
    step: "05",
    heading: "Working with changes & history",
    body: "The main view puts branches, your staged and unstaged changes, and the revision history side by side. Stage files, write a commit message, and watch the new revision land in history. Open any revision to inspect its diff, cherry-pick it, or revert it.",
    shots: [
      {
        src: "/screenshots/main-view-dark.png",
        alt: "LoreGUI main view with branches, changes and history side by side.",
        title: "Branches · Changes · History",
      },
      {
        src: "/screenshots/panel-history-dark.png",
        alt: "LoreGUI history panel listing revisions with diff and revert actions.",
        title: "History panel",
      },
    ],
  },
  {
    id: "branches",
    step: "06",
    heading: "Branches & merging",
    body: "Create, protect, reset and archive branches from the branches panel. When two branches diverge, LoreGUI walks you through a guided three-way merge — resolve each conflict as mine, theirs, or a manual blend, then finish the merge in one place.",
    shots: [
      {
        src: "/screenshots/panel-branches-dark.png",
        alt: "LoreGUI branches panel showing the branch list and the guided merge flow.",
        title: "Branches panel",
      },
    ],
  },
  {
    id: "storage",
    step: "07",
    heading: "Storage backends",
    body: "The storage panel shows which backend a repository is bound to and whether it’s reachable. Content is chunked and hashed with BLAKE3, so identical data is stored exactly once — backends stay small and integrity is verifiable down to the chunk.",
    shots: [
      {
        src: "/screenshots/panel-storage-dark.png",
        alt: "LoreGUI storage panel showing the configured backend and connectivity status.",
        title: "Storage panel",
      },
    ],
  },
  {
    id: "locks",
    step: "08",
    heading: "Locks",
    body: "Binary assets can’t be merged. Claim an exclusive lock before you edit a texture, mesh or audio file, see who holds what in real time, and release it with one click when you’re done.",
    shots: [
      {
        src: "/screenshots/panel-locks-dark.png",
        alt: "LoreGUI locks panel listing held file locks and their owners.",
        title: "Locks panel",
      },
    ],
  },
  {
    id: "dependencies",
    step: "09",
    heading: "Dependencies",
    body: "Track the links between files and the assets they reference. The dependencies panel surfaces what depends on what, so you can change shared assets with confidence and remove links cleanly.",
    shots: [
      {
        src: "/screenshots/panel-dependencies-dark.png",
        alt: "LoreGUI dependencies panel showing links between files and referenced assets.",
        title: "Dependencies panel",
      },
    ],
  },
  {
    id: "manage",
    step: "10",
    heading: "Repository management",
    body: "Administer the repository itself from the manage panel — create and delete repositories, flush and garbage-collect storage, verify integrity, and set metadata. The maintenance operations that used to live behind CLI flags, made visible.",
    shots: [
      {
        src: "/screenshots/panel-manage-dark.png",
        alt: "LoreGUI repository manage panel with administration and maintenance actions.",
        title: "Manage panel",
      },
    ],
  },
  {
    id: "account",
    step: "11",
    heading: "Account",
    body: "Review your identity and the server you’re signed in to from the account panel. LoreGUI resolves your user info from the connection, so you always know who you are and where your commits land.",
    shots: [
      {
        src: "/screenshots/panel-account-dark.png",
        alt: "LoreGUI account panel showing the signed-in identity.",
        title: "Account panel",
      },
    ],
  },
  {
    id: "theming",
    step: "12",
    heading: "Theming",
    body: "Every surface in LoreGUI is a semantic token. The theme editor lets you build a palette, save it, and share it — the entire app re-themes instantly. Ship a dark theme for late nights and a light one for the studio, from the same controls.",
    shots: [
      {
        src: "/screenshots/panel-theme-light.png",
        alt: "LoreGUI theme editor in a light theme, exposing semantic surface tokens.",
        title: "Theme editor — light",
      },
      {
        src: "/screenshots/panel-theme-dark.png",
        alt: "LoreGUI theme editor in a dark theme, exposing semantic surface tokens.",
        title: "Theme editor — dark",
      },
    ],
    sideBySide: true,
  },
];

function GuideShot({ shot }: { shot: Shot }) {
  return (
    <AppWindow title={`LoreGUI — ${shot.title}`}>
      <Image
        src={shot.src}
        alt={shot.alt}
        width={shot.width ?? 1440}
        height={shot.height ?? 900}
        className="w-full"
      />
    </AppWindow>
  );
}

function GuideSection({ section }: { section: GuideSectionData }) {
  const multi = section.shots.length > 1;
  return (
    <section
      id={section.id}
      className="scroll-mt-24 border-t border-brand-muted/10 py-12 first:border-t-0 sm:py-16"
    >
      <div className="flex items-baseline gap-3">
        <span className="font-mono text-sm text-brand-accent">
          {section.step}
        </span>
        <h2 className="font-heading text-2xl font-bold tracking-tight text-brand-text-bright sm:text-3xl">
          {section.heading}
        </h2>
      </div>
      <p className="mt-3 max-w-3xl text-base leading-relaxed text-brand-muted">
        {section.body}
      </p>

      <div
        className={`mt-8 grid gap-6 ${
          multi ? "lg:grid-cols-2" : "max-w-4xl"
        }`}
      >
        {section.shots.map((shot) => (
          <GuideShot key={shot.src} shot={shot} />
        ))}
      </div>
    </section>
  );
}

export function Guide() {
  return (
    <div className="pt-32 pb-20 sm:pt-40">
      {/* Header */}
      <Container>
        <div className="mx-auto max-w-3xl text-center">
          <Badge variant="accent" className="mb-6">
            User guide
          </Badge>
          <h1 className="font-heading text-4xl font-bold tracking-tight sm:text-5xl">
            Get the most out of <GradientText>LoreGUI.</GradientText>
          </h1>
          <p className="mx-auto mt-6 max-w-2xl text-lg text-brand-muted">
            From first launch to theming, here’s how every surface of LoreGUI
            works — illustrated with real screenshots of the desktop app.
          </p>
        </div>

        {/* Contents */}
        <Card className="mx-auto mt-12 max-w-3xl">
          <h2 className="font-heading text-sm font-semibold tracking-wide text-brand-text-bright uppercase">
            On this page
          </h2>
          <ol className="mt-4 grid gap-x-6 gap-y-2 sm:grid-cols-2">
            {sections.map((s) => (
              <li key={s.id}>
                <a
                  href={`#${s.id}`}
                  className="inline-flex items-baseline gap-2 text-sm text-brand-muted transition-colors hover:text-brand-text-bright"
                >
                  <span className="font-mono text-xs text-brand-accent">
                    {s.step}
                  </span>
                  {s.heading}
                </a>
              </li>
            ))}
          </ol>
        </Card>
      </Container>

      {/* Sections */}
      <Container className="mt-8">
        <div className="mx-auto max-w-5xl">
          {sections.map((section) => (
            <GuideSection key={section.id} section={section} />
          ))}
        </div>
      </Container>
    </div>
  );
}
