import { Container } from "@/components/ui/Container";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { CodeBlock } from "@/components/CodeBlock";
import {
  LoreIcon,
  VSCodeIcon,
  UnrealIcon,
  TerminalIcon,
  ArrowRightIcon,
  CheckIcon,
} from "@/components/icons";

const VSCODE_MARKETPLACE_URL =
  "https://marketplace.visualstudio.com/items?itemName=BiloxiStudios.loregui-lore";
const VSCODE_INSTALL_CMD = [
  "code --install-extension BiloxiStudios.loregui-lore",
];

type SurfaceStatus = "live" | "coming";

interface SurfaceCta {
  label: string;
  href: string;
  external?: boolean;
}

interface Surface {
  id: string;
  icon: React.ComponentType<{ className?: string }>;
  label: string;
  status: SurfaceStatus;
  headline: string;
  description: string;
  cta: SurfaceCta | null;
  detail: string[] | null;
  features: string[];
}

const surfaces: Surface[] = [
  {
    id: "desktop",
    icon: LoreIcon,
    label: "Desktop app",
    status: "live",
    headline: "LoreGUI Desktop",
    description:
      "The full GUI: branches, commits, merge, diff, file locking, theming — everything in one cross-platform window. Binds the lore engine in-process; no daemon, no CLI shelling.",
    cta: { label: "Download", href: "/#install" },
    detail: null,
    features: [
      "Visual branch, merge & diff",
      "File locking for binaries",
      "Command palette (⌘K)",
      "Fully themeable",
      "Windows, macOS & Linux",
    ],
  },
  {
    id: "vscode",
    icon: VSCodeIcon,
    label: "VS Code extension",
    status: "live",
    headline: "Lore Source Control",
    description:
      "Lore source control inside VS Code's native SCM panel. Stage, commit, view history, compare diffs, and manage file locks without leaving your editor. Built for Verse/UEFN teams and asset-heavy projects — powered by the same lorevm engine as the desktop app.",
    cta: {
      label: "Install from Marketplace",
      href: VSCODE_MARKETPLACE_URL,
      external: true,
    },
    detail: VSCODE_INSTALL_CMD,
    features: [
      "Native SCM panel integration",
      "Stage, commit & diff inline",
      "Revision history in-editor",
      "File-lock awareness & status",
      "One-line install",
    ],
  },
  {
    id: "unreal",
    icon: UnrealIcon,
    label: "Unreal Engine plugin",
    status: "coming",
    headline: "StudioBrain for Unreal",
    description:
      "Lore source control inside Unreal Engine's content browser. Lock/status overlays on assets, checkout = lock, check-in = commit, sync — all driving lorevm-ffi for the hot path. Entity-aware versioning is on the roadmap.",
    cta: null,
    detail: null,
    features: [
      "Content-browser lock overlays",
      "Checkout = lock, check-in = commit",
      "Sync from Unreal toolbar",
      "Drives lorevm-ffi (C ABI)",
      "Entity-aware versioning (roadmap)",
    ],
  },
  {
    id: "mcp",
    icon: TerminalIcon,
    label: "MCP / agents",
    status: "live",
    headline: "lore-mcp",
    description:
      "Drive Lore from AI agents (Claude Code and others) via the MCP server included in the repo. One tool per Lore operation, schemas generated from the same palette manifests as the desktop app — so the agent and the app stay in lock-step.",
    cta: { label: "Read the docs", href: "/docs/mcp" },
    detail: null,
    features: [
      "One MCP tool per Lore op",
      "Schemas from palette manifests",
      "Status, history, diff, locks",
      "Commit, branch, stage, lock",
      "Agent self-onboarding skills",
    ],
  },
];

export function Ecosystem() {
  return (
    <section
      id="ecosystem"
      className="relative overflow-hidden border-t border-brand-muted/10 py-20 sm:py-32"
    >
      <div className="pointer-events-none absolute inset-0" aria-hidden="true">
        <div className="absolute left-1/3 top-0 h-[400px] w-[600px] -translate-x-1/2 rounded-full bg-vapor-purple/[0.08] blur-3xl" />
        <div className="absolute right-0 top-1/2 h-[300px] w-[400px] -translate-y-1/2 rounded-full bg-vapor-blue/[0.08] blur-3xl" />
      </div>

      <Container className="relative">
        <div className="mx-auto max-w-2xl text-center">
          <Badge variant="accent" className="mb-6">
            One toolkit, four surfaces
          </Badge>
          <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
            Lore anywhere you work
          </h2>
          <p className="mt-4 text-lg text-brand-muted">
            Whether you&rsquo;re in a desktop GUI, a code editor, Unreal
            Engine, or an AI agent &mdash; the same{" "}
            <span className="font-semibold text-brand-text">lorevm</span> engine
            drives every surface, and every op is consistent across all of them.
          </p>
        </div>

        {/* Surface pill strip */}
        <div className="mx-auto mt-10 flex max-w-2xl flex-wrap items-center justify-center gap-3">
          {surfaces.map((s) => (
            <span
              key={s.id}
              className="inline-flex items-center gap-1.5 rounded-full border border-brand-muted/20 bg-brand-surface/60 px-4 py-1.5 text-sm font-medium text-brand-muted"
            >
              <s.icon className="h-4 w-4 shrink-0" />
              {s.label}
              {s.status === "coming" && (
                <span className="ml-1 rounded-full bg-brand-gold/15 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-brand-gold">
                  soon
                </span>
              )}
            </span>
          ))}
        </div>

        {/* Surface cards */}
        <div className="mt-16 grid gap-6 sm:grid-cols-2">
          {surfaces.map((surface) => (
            <Card
              key={surface.id}
              hover={surface.status !== "coming"}
              highlight={surface.id === "desktop"}
              className="flex flex-col gap-5"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-center gap-3">
                  <div className="inline-flex rounded-lg bg-brand-accent/10 p-3">
                    <surface.icon className="h-6 w-6 text-brand-accent" />
                  </div>
                  <div>
                    <h3 className="font-heading text-lg font-semibold text-brand-text-bright">
                      {surface.headline}
                    </h3>
                    <p className="text-xs text-brand-muted">{surface.label}</p>
                  </div>
                </div>
                {surface.status === "coming" ? (
                  <span className="shrink-0 rounded-full border border-brand-gold/30 bg-brand-gold/10 px-2.5 py-1 text-xs font-semibold text-brand-gold">
                    Coming soon
                  </span>
                ) : (
                  <span className="shrink-0 rounded-full border border-vapor-green/30 bg-vapor-green/10 px-2.5 py-1 text-xs font-semibold text-vapor-green">
                    Live
                  </span>
                )}
              </div>

              <p className="text-sm leading-relaxed text-brand-muted">
                {surface.description}
              </p>

              <ul className="grid grid-cols-1 gap-y-2" role="list">
                {surface.features.map((f) => (
                  <li key={f} className="flex items-start gap-2">
                    <CheckIcon className="mt-0.5 h-4 w-4 shrink-0 text-vapor-green" />
                    <span className="text-sm text-brand-text">{f}</span>
                  </li>
                ))}
              </ul>

              {surface.detail && (
                <div className="mt-1">
                  <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-brand-muted">
                    One-liner install
                  </p>
                  <CodeBlock lines={surface.detail} />
                </div>
              )}

              {surface.cta && (
                <div className="mt-auto pt-2">
                  <Button
                    variant="secondary"
                    size="sm"
                    href={surface.cta.href}
                    {...(surface.cta.external
                      ? { target: "_blank", rel: "noopener noreferrer" }
                      : {})}
                  >
                    {surface.cta.label}
                    <ArrowRightIcon className="ml-2 h-4 w-4" />
                  </Button>
                </div>
              )}

              {surface.status === "coming" && (
                <p className="mt-auto pt-2 text-xs text-brand-muted">
                  No download yet &mdash; follow the{" "}
                  <a
                    href="https://github.com/BiloxiStudios/loregui"
                    className="text-brand-accent hover:text-brand-accent-hover"
                    target="_blank"
                    rel="noopener noreferrer"
                  >
                    GitHub repo
                  </a>{" "}
                  for updates.
                </p>
              )}
            </Card>
          ))}
        </div>

        {/* VS Code install highlight */}
        <div className="mx-auto mt-12 max-w-2xl rounded-2xl border border-brand-accent/20 bg-brand-surface/60 p-6">
          <div className="flex items-center gap-3">
            <VSCodeIcon className="h-6 w-6 text-brand-accent" />
            <h3 className="font-heading text-base font-semibold text-brand-text-bright">
              Install the VS Code extension now
            </h3>
          </div>
          <p className="mt-3 text-sm leading-relaxed text-brand-muted">
            Published as{" "}
            <code className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-xs text-brand-text">
              BiloxiStudios.loregui-lore
            </code>{" "}
            on the VS Code Marketplace. Run the one-liner or search &ldquo;Lore
            Source Control&rdquo; in the Extensions panel.
          </p>
          <div className="mt-4">
            <CodeBlock lines={VSCODE_INSTALL_CMD} />
          </div>
          <div className="mt-4 flex flex-wrap gap-3">
            <Button
              variant="primary"
              size="sm"
              href={VSCODE_MARKETPLACE_URL}
              target="_blank"
              rel="noopener noreferrer"
            >
              <VSCodeIcon className="mr-2 h-4 w-4" />
              Open in Marketplace
            </Button>
            <Button variant="secondary" size="sm" href="/docs/vscode">
              Read the docs
              <ArrowRightIcon className="ml-2 h-4 w-4" />
            </Button>
          </div>
        </div>
      </Container>
    </section>
  );
}
