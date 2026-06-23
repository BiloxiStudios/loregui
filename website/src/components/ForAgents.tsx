import { Container } from "@/components/ui/Container";
import { Card } from "@/components/ui/Card";
import { Badge } from "@/components/ui/Badge";
import { CodeBlock } from "@/components/CodeBlock";
import {
  TerminalIcon,
  GitCompareIcon,
  BoltIcon,
  CheckIcon,
} from "@/components/icons";

// The exact shape an agent drops into its MCP config — matches lore-mcp/README.md.
const mcpRegistration = [
  '"lore": {',
  '  "command": "/path/to/loregui/lore-mcp/venv/bin/python",',
  '  "args": ["/path/to/loregui/lore-mcp/server.py"],',
  '  "env": {',
  '    "LORE_REPO": "/path/to/repo",',
  '    "LORE_OFFLINE": "1"',
  "  }",
  "}",
];

// One-paste prompt an agent can run to self-install the full stack end-to-end.
// The runbook lives at AGENT-SETUP.md in the repo root.
const agentInstallPrompt =
  "Set up LoreGUI and the lore MCP for me — read and follow " +
  "https://raw.githubusercontent.com/BiloxiStudios/loregui/main/AGENT-SETUP.md " +
  "end-to-end, then verify it works.";

const points = [
  {
    icon: TerminalIcon,
    title: "One tool per lore op",
    description:
      "The MCP server exposes each lore operation as its own tool — commit, branch, stage, lock and more — so an agent calls them natively instead of guessing at a CLI.",
  },
  {
    icon: GitCompareIcon,
    title: "Schemas from the same manifests",
    description:
      "Every tool's name, description and input schema is generated from the GUI's command-palette manifests — the agent and the app stay in lock-step from one source of truth.",
  },
  {
    icon: BoltIcon,
    title: "Read tools are repo intelligence",
    description:
      "status, history, diff, file-history and locks — the git/p4-equivalent read & metrics surface — give an agent a structured, real-time picture of the repository before it acts.",
  },
];

export function ForAgents() {
  return (
    <section
      id="agents"
      className="relative overflow-hidden border-t border-brand-muted/10 py-20 sm:py-32"
    >
      <div className="pointer-events-none absolute inset-0" aria-hidden="true">
        <div className="absolute right-1/4 top-0 h-[300px] w-[500px] translate-x-1/2 rounded-full bg-brand-accent/5 blur-3xl" />
      </div>

      <Container className="relative">
        <div className="mx-auto max-w-3xl text-center">
          <Badge variant="accent" className="mb-6">
            <TerminalIcon className="mr-1.5 h-4 w-4" />
            Drive LoreGUI from AI agents (MCP)
          </Badge>
          <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
            Built for agents, too
          </h2>
          <p className="mt-4 text-lg leading-relaxed text-brand-muted">
            LoreGUI isn&rsquo;t just a desktop app — it&rsquo;s a toolkit. The
            same in-process lore binding that powers the command palette and
            panels also ships as an{" "}
            <span className="font-semibold text-brand-text">MCP server</span>,
            so agents like Claude Code drive Epic&rsquo;s lore VCS the same way a
            person uses the GUI.
          </p>
          <p className="mt-4 text-lg leading-relaxed text-brand-muted">
            Point an agent at the server and it gets native lore access — status,
            history, diff, branches, file-history and locks — plus the mutations
            to commit, branch, stage and lock. It lives in the repo at{" "}
            <code className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-sm text-brand-text">
              lore-mcp/
            </code>{" "}
            and pairs with the desktop app.
          </p>
        </div>

        <div className="mx-auto mt-14 grid items-start gap-10 lg:grid-cols-2">
          <div>
            <h3 className="mb-3 font-heading text-sm font-semibold uppercase tracking-wide text-brand-muted">
              Register it in one block
            </h3>
            <CodeBlock lines={mcpRegistration} />
            <p className="mt-4 flex items-start gap-2.5 text-sm leading-relaxed text-brand-muted">
              <CheckIcon className="mt-0.5 h-4 w-4 shrink-0 text-vapor-green" />
              <span>
                Two agent skills —{" "}
                <code className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-xs text-brand-text">
                  loregui
                </code>{" "}
                and{" "}
                <code className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-xs text-brand-text">
                  lore
                </code>{" "}
                — let any agent self-onboard: install, configure and then drive
                the server without hand-holding.
              </span>
            </p>
          </div>

          <ul className="grid gap-4" role="list">
            {points.map((p) => (
              <li key={p.title}>
                <Card className="flex gap-4">
                  <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border border-brand-accent/30 bg-brand-accent/10">
                    <p.icon className="h-5 w-5 text-brand-accent" />
                  </div>
                  <div>
                    <h4 className="font-heading text-base font-semibold text-brand-text-bright">
                      {p.title}
                    </h4>
                    <p className="mt-1 text-sm leading-relaxed text-brand-muted">
                      {p.description}
                    </p>
                  </div>
                </Card>
              </li>
            ))}
          </ul>
        </div>

        {/* Agent self-install block — paste this prompt into any agent to bootstrap
            the full stack end-to-end from the AGENT-SETUP.md runbook. */}
        <div className="mx-auto mt-16 max-w-3xl">
          <div className="rounded-xl border border-brand-accent/20 bg-brand-deep/60 p-6">
            <div className="mb-3 flex items-center gap-2">
              <TerminalIcon className="h-4 w-4 text-brand-accent" />
              <h3 className="font-heading text-sm font-semibold uppercase tracking-wide text-brand-muted">
                Let your agent install it
              </h3>
            </div>
            <p className="mb-4 text-sm leading-relaxed text-brand-muted">
              Paste this prompt into Claude Code (or any agent that can fetch
              URLs) and it will clone the repo, build{" "}
              <code className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-xs text-brand-text">
                lorevm
              </code>
              , install the MCP server, register it, and verify the chain
              end-to-end — no manual steps.
            </p>
            <CodeBlock lines={[agentInstallPrompt]} />
            <p className="mt-3 text-xs leading-relaxed text-brand-muted/70">
              The runbook is{" "}
              <a
                href="https://github.com/BiloxiStudios/loregui/blob/main/AGENT-SETUP.md"
                target="_blank"
                rel="noopener noreferrer"
                className="underline underline-offset-2 hover:text-brand-muted"
              >
                AGENT-SETUP.md
              </a>{" "}
              in the repo root — readable by humans and agents alike.
            </p>
          </div>
        </div>
      </Container>
    </section>
  );
}
