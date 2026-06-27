import type { Metadata } from "next";
import Image from "next/image";
import { Header } from "@/components/Header";
import { Footer } from "@/components/Footer";
import { Container } from "@/components/ui/Container";
import { Button } from "@/components/ui/Button";
import { Badge } from "@/components/ui/Badge";
import { Card } from "@/components/ui/Card";
import { GradientText } from "@/components/ui/GradientText";
import { AppWindow } from "@/components/mockups/AppWindow";
import {
  LoreIcon,
  DatabaseIcon,
  ApiIcon,
  BoltIcon,
  CloudDownloadIcon,
  LockIcon,
  GitCompareIcon,
  ArrowRightIcon,
  CheckIcon,
} from "@/components/icons";

// StudioBrain.AI is a SEPARATE commercial product by Biloxi Studios that builds
// ON TOP of the open-source LoreGUI/lore stack. This page introduces it; it does
// not bundle or embed any StudioBrain code. CTAs link out to studiobrain.ai.
const STUDIOBRAIN_URL = "https://studiobrain.ai";
const STUDIOBRAIN_APP_URL = "https://app.studiobrain.ai";
// The "Connect your lore server" guide lives in the StudioBrain docs (SBAI-4289 / CP.8.6).
const CONNECT_GUIDE_URL = "https://docs.studiobrain.ai/guide/connect-lore-server";

export const metadata: Metadata = {
  title: "StudioBrain.AI — premium asset management on LoreGUI",
  description:
    "StudioBrain.AI is a commercial, schema-driven digital asset management platform that layers on top of LoreGUI. Keep your lore repository as the source of truth, and add entity/asset schemas, search, multi-tenant cloud, and a one-flow desktop install that bundles and activates LoreGUI for you.",
  alternates: { canonical: "/premium" },
  openGraph: {
    title: "StudioBrain.AI — premium asset management on LoreGUI",
    description:
      "The commercial DAM layer over open-source LoreGUI: schema-driven entities & assets, search, multi-tenant cloud, and a one-flow desktop install that bundles and hosts LoreGUI.",
    url: "/premium",
    type: "website",
  },
};

/**
 * A placeholder screenshot slot. Renders the shared desktop-window chrome around
 * a labeled placeholder image so the page reads correctly before the real
 * captures land. The capture pass (SBAI-4287 / CP.8.5, ADR-0004 C4 manifest)
 * overwrites each PNG in place at the SAME path — no code change needed here.
 */
function ScreenshotSlot({
  src,
  alt,
  windowTitle,
  priority,
}: {
  src: string;
  alt: string;
  windowTitle: string;
  priority?: boolean;
}) {
  return (
    <figure className="relative">
      <AppWindow title={windowTitle}>
        <Image
          src={src}
          alt={alt}
          width={1440}
          height={900}
          sizes="(min-width: 1024px) 50vw, 100vw"
          className="w-full"
          priority={priority}
        />
      </AppWindow>
      <figcaption className="sr-only">{alt}</figcaption>
    </figure>
  );
}

interface Feature {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  body: string;
}

const ADDS: Feature[] = [
  {
    icon: DatabaseIcon,
    title: "Schema-driven entities & assets",
    body: "Define characters, items, locations, quests — any entity type — with typed templates and validation. StudioBrain writes each one as Markdown into your lore repo, so the version-controlled files stay the single source of truth.",
  },
  {
    icon: BoltIcon,
    title: "Full-text & semantic search",
    body: "Index every entity, asset and document for instant keyword and meaning-based search across the whole project — backed by a content cache that always rebuilds from your lore revisions.",
  },
  {
    icon: CloudDownloadIcon,
    title: "Multi-tenant cloud",
    body: "Web and mobile clients for your whole team, with per-tenant isolation and cloud sync — while writes still land as revisions in your own lore repository, not a proprietary database.",
  },
  {
    icon: ApiIcon,
    title: "AI workshop & agents",
    body: "Generate, summarise and cross-reference entities with built-in AI, metered per tenant. The same schema that powers the forms also grounds the AI in your project's canon.",
  },
  {
    icon: GitCompareIcon,
    title: "Relationships & cross-references",
    body: "Link entities, follow references, and see what depends on what. The graph is derived from the same Markdown that LoreGUI versions, diffs and merges.",
  },
  {
    icon: LockIcon,
    title: "Your data, your repo",
    body: "Every edit becomes a lore revision in a repository you own. Cancel any time and keep a complete, human-readable, fully-versioned history — no lock-in.",
  },
];

const ONE_FLOW = [
  {
    n: "1",
    title: "Install studiobrain-app",
    body: "Download and run the StudioBrain desktop installer. There's nothing else to set up by hand.",
  },
  {
    n: "2",
    title: "LoreGUI auto-installs & hosts",
    body: "StudioBrain bundles LoreGUI as a sidecar. On first run it stages a local lore server over your project folder and starts hosting it — no separate download.",
  },
  {
    n: "3",
    title: "Token-activates & connects",
    body: "The app advertises the hosted server through a secure relay, activates a write grant for your account, and connects StudioBrain to it automatically.",
  },
  {
    n: "4",
    title: "Start managing assets",
    body: "Create entities and assets in StudioBrain; each edit is written straight into your lore repository as a new revision you can branch, diff and merge in LoreGUI.",
  },
];

export default function PremiumPage() {
  return (
    <>
      <Header />
      <main>
        {/* ---------------------------------------------------------------- */}
        {/* Hero                                                             */}
        {/* ---------------------------------------------------------------- */}
        <section className="relative overflow-hidden pt-32 pb-20 sm:pt-40 sm:pb-28">
          <div
            className="pointer-events-none absolute inset-0"
            aria-hidden="true"
          >
            <div className="absolute top-0 left-1/2 h-[600px] w-[800px] -translate-x-1/2 rounded-full bg-vapor-purple/10 blur-3xl" />
            <div className="absolute top-40 right-0 h-[400px] w-[400px] rounded-full bg-vapor-pink/10 blur-3xl" />
          </div>

          <Container className="relative">
            <div className="mx-auto max-w-4xl text-center">
              <Badge variant="gold" className="mb-6">
                Commercial product by Biloxi Studios
              </Badge>
              <h1 className="font-heading text-4xl font-bold tracking-tight sm:text-6xl">
                Premium asset management,
                <br />
                built on <GradientText>LoreGUI.</GradientText>
              </h1>
              <p className="mx-auto mt-6 max-w-2xl text-lg text-brand-muted sm:text-xl">
                <span className="text-brand-text">StudioBrain.AI</span> is a
                schema-driven digital asset management platform for game teams.
                It turns the open-source <span className="text-brand-text">
                  LoreGUI
                </span>{" "}
                stack into a full DAM &mdash; entities, assets, search and an
                AI workshop &mdash; while your lore repository stays the source
                of truth for every change.
              </p>

              <div className="mt-10 flex flex-col items-center justify-center gap-4 sm:flex-row">
                <Button
                  variant="primary"
                  size="lg"
                  href={STUDIOBRAIN_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Explore StudioBrain.AI
                  <ArrowRightIcon className="ml-2 h-5 w-5" />
                </Button>
                <Button
                  variant="secondary"
                  size="lg"
                  href={CONNECT_GUIDE_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Read the connect guide
                </Button>
              </div>

              <p className="mt-4 text-sm text-brand-muted">
                LoreGUI stays free and open source (MIT). StudioBrain.AI is a
                paid, optional layer on top &mdash; you never need it to use
                LoreGUI.
              </p>
            </div>

            {/* Hero screenshot */}
            <div className="relative mx-auto mt-16 max-w-5xl">
              {/* SCREENSHOT (CP.8.5 / C4): the StudioBrain.AI app — entity & asset
                  browser over a game project. Replace public/screenshots/studiobrain-hero.png in place. */}
              <ScreenshotSlot
                windowTitle="StudioBrain.AI — astral-engine"
                src="/screenshots/studiobrain-hero.png"
                alt="The StudioBrain.AI desktop app showing a schema-driven entity and asset browser for a game project, backed by a lore repository."
                priority
              />
              <div
                className="pointer-events-none absolute -inset-4 -z-10 rounded-xl bg-vapor-purple/10 blur-2xl"
                aria-hidden="true"
              />
            </div>
          </Container>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* Positioning: LoreGUI vs StudioBrain.AI                           */}
        {/* ---------------------------------------------------------------- */}
        <section className="border-t border-brand-muted/10 py-20 sm:py-28">
          <Container>
            <div className="mx-auto max-w-2xl text-center">
              <Badge variant="accent" className="mb-6">
                Two layers, one stack
              </Badge>
              <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
                Where LoreGUI ends and StudioBrain begins
              </h2>
              <p className="mt-4 text-lg text-brand-muted">
                They are complementary, not competing: LoreGUI versions and
                hosts your project; StudioBrain.AI is the premium product layer
                your team works in.
              </p>
            </div>

            <div className="mt-14 grid gap-6 lg:grid-cols-2">
              <Card className="flex flex-col gap-5">
                <div className="flex items-center gap-3">
                  <div className="inline-flex rounded-lg bg-brand-accent/10 p-3">
                    <LoreIcon className="h-6 w-6 text-brand-accent" />
                  </div>
                  <div>
                    <h3 className="font-heading text-lg font-semibold text-brand-text-bright">
                      LoreGUI
                    </h3>
                    <p className="text-xs text-brand-muted">
                      Open source &middot; MIT &middot; free forever
                    </p>
                  </div>
                </div>
                <p className="text-sm leading-relaxed text-brand-muted">
                  The desktop client and host for Epic&rsquo;s{" "}
                  <span className="text-brand-text">lore</span> version control.
                  Branch, commit, merge, diff, lock binaries, and serve a repo
                  to your team over a <code className="rounded bg-brand-deep/70 px-1 py-0.5 font-mono text-xs text-brand-text">lore://</code> URL.
                </p>
                <ul className="grid gap-y-2" role="list">
                  {[
                    "Versions code and huge binary assets",
                    "Hosts your own lore server",
                    "Visual branch, merge, diff & locks",
                    "Runs anywhere — Windows, macOS, Linux",
                  ].map((f) => (
                    <li key={f} className="flex items-start gap-2">
                      <CheckIcon className="mt-0.5 h-4 w-4 shrink-0 text-vapor-green" />
                      <span className="text-sm text-brand-text">{f}</span>
                    </li>
                  ))}
                </ul>
              </Card>

              <Card highlight className="flex flex-col gap-5">
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3">
                    <div className="inline-flex rounded-lg bg-brand-gold/10 p-3">
                      <DatabaseIcon className="h-6 w-6 text-brand-gold" />
                    </div>
                    <div>
                      <h3 className="font-heading text-lg font-semibold text-brand-text-bright">
                        StudioBrain.AI
                      </h3>
                      <p className="text-xs text-brand-muted">
                        Commercial &middot; SaaS + desktop &middot; paid plans
                      </p>
                    </div>
                  </div>
                  <Badge variant="gold">Premium</Badge>
                </div>
                <p className="text-sm leading-relaxed text-brand-muted">
                  The schema-driven DAM for game development. It writes every
                  entity and asset into <span className="text-brand-text">your</span>{" "}
                  lore repository, then adds typed schemas, search, multi-tenant
                  cloud, mobile clients and an AI workshop on top.
                </p>
                <ul className="grid gap-y-2" role="list">
                  {[
                    "Schema-driven entities & assets",
                    "Full-text + semantic search",
                    "Multi-tenant cloud, web & mobile",
                    "AI workshop, metered per tenant",
                  ].map((f) => (
                    <li key={f} className="flex items-start gap-2">
                      <CheckIcon className="mt-0.5 h-4 w-4 shrink-0 text-vapor-green" />
                      <span className="text-sm text-brand-text">{f}</span>
                    </li>
                  ))}
                </ul>
              </Card>
            </div>
          </Container>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* What it adds                                                     */}
        {/* ---------------------------------------------------------------- */}
        <section className="border-t border-brand-muted/10 py-20 sm:py-28">
          <Container>
            <div className="mx-auto max-w-2xl text-center">
              <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
                What StudioBrain.AI adds on top
              </h2>
              <p className="mt-4 text-lg text-brand-muted">
                Everything below is layered over your lore repository &mdash;
                the version-controlled Markdown stays the source of truth, so
                nothing here locks your data away.
              </p>
            </div>

            <div className="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
              {ADDS.map((f) => (
                <Card key={f.title} hover className="flex flex-col gap-4">
                  <div className="inline-flex w-fit rounded-lg bg-brand-accent/10 p-3">
                    <f.icon className="h-6 w-6 text-brand-accent" />
                  </div>
                  <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                    {f.title}
                  </h3>
                  <p className="text-sm leading-relaxed text-brand-muted">
                    {f.body}
                  </p>
                </Card>
              ))}
            </div>
          </Container>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* The desktop one-flow install                                     */}
        {/* ---------------------------------------------------------------- */}
        <section className="relative overflow-hidden border-t border-brand-muted/10 py-20 sm:py-28">
          <div
            className="pointer-events-none absolute inset-0"
            aria-hidden="true"
          >
            <div className="absolute left-1/3 top-1/4 h-[360px] w-[520px] -translate-x-1/2 rounded-full bg-vapor-blue/[0.08] blur-3xl" />
          </div>
          <Container className="relative">
            <div className="mx-auto max-w-2xl text-center">
              <Badge variant="accent" className="mb-6">
                One installer, zero setup
              </Badge>
              <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
                The desktop one-flow install
              </h2>
              <p className="mt-4 text-lg text-brand-muted">
                Install StudioBrain on the desktop and it brings LoreGUI with
                it &mdash; hosting, activating and connecting your lore server in
                a single flow.
              </p>
            </div>

            <div className="mt-14 grid items-start gap-10 lg:grid-cols-2">
              <ol className="space-y-6">
                {ONE_FLOW.map((step) => (
                  <li key={step.n} className="flex gap-4">
                    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-gradient-to-r from-vapor-pink via-vapor-purple to-vapor-blue font-heading text-sm font-bold text-white">
                      {step.n}
                    </span>
                    <div>
                      <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                        {step.title}
                      </h3>
                      <p className="mt-1 text-sm leading-relaxed text-brand-muted">
                        {step.body}
                      </p>
                    </div>
                  </li>
                ))}
              </ol>

              <div className="space-y-6">
                {/* SCREENSHOT (CP.8.5 / C4 lore-cloud-onboarding-option): picking the
                    StudioBrain Lore provider during onboarding. */}
                <ScreenshotSlot
                  windowTitle="StudioBrain — choose your backend"
                  src="/screenshots/studiobrain-lore-cloud-onboarding-option.png"
                  alt="The StudioBrain onboarding wizard with the 'StudioBrain Lore' provider selected as the project source of truth."
                />
                {/* SCREENSHOT (CP.8.5 / C4 lore-connected-state): the connected state
                    after the desktop one-flow install. */}
                <ScreenshotSlot
                  windowTitle="StudioBrain — connected"
                  src="/screenshots/studiobrain-lore-connected-state.png"
                  alt="StudioBrain showing a connected lore server: health connected, repo and branch, and the latest synced revision."
                />
              </div>
            </div>
          </Container>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* Bring your own lore server                                       */}
        {/* ---------------------------------------------------------------- */}
        <section className="border-t border-brand-muted/10 py-20 sm:py-28">
          <Container>
            <div className="mx-auto max-w-2xl text-center">
              <Badge variant="accent" className="mb-6">
                Already self-hosting?
              </Badge>
              <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
                Bring your own lore server
              </h2>
              <p className="mt-4 text-lg text-brand-muted">
                If you already host a lore repository &mdash; with LoreGUI, in
                your own infrastructure, or anywhere reachable &mdash; point
                StudioBrain.AI at it instead of letting the desktop app host one
                for you.
              </p>
            </div>

            <div className="mt-14 grid items-start gap-10 lg:grid-cols-2">
              <div className="space-y-6">
                <div className="rounded-xl border border-brand-muted/20 bg-brand-surface p-6">
                  <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                    1. Configure the connection
                  </h3>
                  <p className="mt-2 text-sm leading-relaxed text-brand-muted">
                    In StudioBrain, open{" "}
                    <span className="text-brand-text">
                      Settings &rarr; Storage &rarr; Lore
                    </span>{" "}
                    and enter your relay URL{" "}
                    <code className="rounded bg-brand-deep/70 px-1 py-0.5 font-mono text-xs text-brand-text">
                      lore://host:port/repo
                    </code>
                    , the repository name and a branch. Hit{" "}
                    <span className="text-brand-text">Test connection</span> to
                    confirm it&rsquo;s reachable before you save.
                  </p>
                </div>
                <div className="rounded-xl border border-brand-muted/20 bg-brand-surface p-6">
                  <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                    2. Connect StudioBrain
                  </h3>
                  <p className="mt-2 text-sm leading-relaxed text-brand-muted">
                    Click <span className="text-brand-text">Connect StudioBrain</span>{" "}
                    and grant read + write access in the secure consent dialog.
                    Consent and identity are handled entirely by the StudioBrain
                    accounts service &mdash; the app only ever receives an opaque
                    grant reference, never a raw token.
                  </p>
                </div>
                <div className="rounded-xl border border-brand-muted/20 bg-brand-surface p-6">
                  <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                    3. Edit &rarr; new revision
                  </h3>
                  <p className="mt-2 text-sm leading-relaxed text-brand-muted">
                    From then on, every entity or asset you edit in StudioBrain
                    is written into your repository as a new lore revision. Open
                    LoreGUI to branch, diff, merge or revert it like any other
                    change.
                  </p>
                </div>
              </div>

              <div className="space-y-6">
                {/* SCREENSHOT (CP.8.5 / C4 lore-tenant-settings-config): the Settings →
                    Storage → Lore config form + Test connection. */}
                <ScreenshotSlot
                  windowTitle="StudioBrain — Settings · Storage · Lore"
                  src="/screenshots/studiobrain-lore-tenant-settings-config.png"
                  alt="StudioBrain Settings showing the Lore storage form: relay URL, repository and branch fields with a Test connection button."
                />
                {/* SCREENSHOT (CP.8.5 / C4 lore-write-roundtrip): an edit appearing as a
                    new lore revision in history. */}
                <ScreenshotSlot
                  windowTitle="StudioBrain — edit becomes a revision"
                  src="/screenshots/studiobrain-lore-write-roundtrip.png"
                  alt="An entity edit in StudioBrain reflected as a new revision in the lore history, with the Markdown commit as the source of truth."
                />
              </div>
            </div>
          </Container>
        </section>

        {/* ---------------------------------------------------------------- */}
        {/* CTA                                                              */}
        {/* ---------------------------------------------------------------- */}
        <section className="border-t border-brand-muted/10 py-20 sm:py-28">
          <Container>
            <div className="mx-auto max-w-3xl rounded-2xl border border-brand-gold/20 bg-brand-surface/60 p-10 text-center">
              <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
                Ready to add the premium layer?
              </h2>
              <p className="mx-auto mt-4 max-w-xl text-lg text-brand-muted">
                Keep building with free, open-source LoreGUI &mdash; and turn on
                StudioBrain.AI when your team needs schemas, search, cloud and
                AI over the same repository.
              </p>
              <div className="mt-8 flex flex-col items-center justify-center gap-4 sm:flex-row">
                <Button
                  variant="primary"
                  size="lg"
                  href={STUDIOBRAIN_APP_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Get started on StudioBrain.AI
                  <ArrowRightIcon className="ml-2 h-5 w-5" />
                </Button>
                <Button
                  variant="secondary"
                  size="lg"
                  href={CONNECT_GUIDE_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  Connect your lore server
                </Button>
              </div>
              <p className="mt-6 text-xs leading-relaxed text-brand-muted">
                StudioBrain.AI is a commercial product of Biloxi Studios, Inc.
                LoreGUI is an independent, MIT-licensed community project and is
                not affiliated with, sponsored by, or endorsed by Epic Games,
                Inc. &ldquo;Lore&rdquo; is a trademark of Epic Games, Inc.
              </p>
            </div>
          </Container>
        </section>
      </main>
      <Footer />
    </>
  );
}
