import type { Metadata } from "next";
import { Badge } from "@/components/ui/Badge";
import {
  OP_REFERENCE,
  OP_COUNT,
  DOMAIN_COUNT,
} from "@/lib/op-reference.generated";

export const metadata: Metadata = {
  title: "Operation reference — LoreGUI docs",
  description:
    "Every operation LoreGUI exposes — domain, op, description, and arguments — generated automatically from the command-palette manifests, the single source of truth.",
  alternates: { canonical: "/docs/op-reference" },
};

function surfaceVariant(surface: string) {
  if (surface === "panel") return "accent" as const;
  if (surface === "menu") return "gold" as const;
  return "default" as const;
}

export default function OpReferencePage() {
  return (
    <div>
      <h1 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
        Operation reference
      </h1>
      <p className="mt-4 text-base leading-relaxed text-brand-muted">
        Every operation LoreGUI exposes, grouped by domain. This page is{" "}
        <strong className="font-semibold text-brand-text">generated</strong>{" "}
        from the command-palette manifests (
        <code className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-[0.85em] text-brand-accent">
          frontend/src/palette/manifest/&lt;domain&gt;/&lt;op&gt;.ts
        </code>
        ) — the single source of truth — so it tracks the real API automatically.
        Run any of these from the{" "}
        <a
          href="/docs/command-palette"
          className="font-medium text-brand-accent underline decoration-brand-accent/30 underline-offset-2 hover:text-brand-accent-hover"
        >
          command palette
        </a>
        .
      </p>

      <p className="mt-4 text-sm text-brand-muted">
        <strong className="font-semibold text-brand-text">{OP_COUNT}</strong>{" "}
        operations across{" "}
        <strong className="font-semibold text-brand-text">
          {DOMAIN_COUNT}
        </strong>{" "}
        domains.
      </p>

      {/* Domain jump list */}
      <nav
        aria-label="Domains"
        className="mt-6 flex flex-wrap gap-2 rounded-lg border border-brand-muted/15 bg-brand-surface/40 p-4"
      >
        {OP_REFERENCE.map((g) => (
          <a
            key={g.domain}
            href={`#${g.domain}`}
            className="rounded-md border border-brand-muted/20 px-2.5 py-1 font-mono text-xs text-brand-muted transition-colors hover:border-brand-accent/40 hover:text-brand-accent"
          >
            {g.domain}
            <span className="ml-1.5 text-brand-muted/60">{g.ops.length}</span>
          </a>
        ))}
      </nav>

      {OP_REFERENCE.map((group) => (
        <section
          key={group.domain}
          id={group.domain}
          className="mt-12 scroll-mt-24"
        >
          <h2 className="border-t border-brand-muted/10 pt-8 font-heading text-2xl font-bold tracking-tight text-brand-text-bright">
            <span className="font-mono text-brand-accent">{group.domain}</span>
          </h2>
          {group.blurb && (
            <p className="mt-2 text-base leading-relaxed text-brand-muted">
              {group.blurb}
            </p>
          )}

          <div className="mt-6 space-y-4">
            {group.ops.map((op) => (
              <div
                key={op.id}
                id={op.id}
                className="scroll-mt-24 rounded-lg border border-brand-muted/15 bg-brand-surface/40 p-5"
              >
                <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
                  <code className="font-mono text-sm font-semibold text-brand-text-bright">
                    {op.id}
                  </code>
                  <Badge variant={surfaceVariant(op.surface)}>
                    {op.surface}
                  </Badge>
                </div>
                {op.description && (
                  <p className="mt-2 text-sm leading-relaxed text-brand-muted">
                    {op.description}
                  </p>
                )}

                {op.args.length > 0 ? (
                  <div className="mt-4 overflow-x-auto rounded-md border border-brand-muted/15">
                    <table className="w-full border-collapse text-left text-sm">
                      <thead className="bg-brand-surface-light/60">
                        <tr>
                          <th className="border-b border-brand-muted/15 px-3 py-2 font-semibold text-brand-text-bright">
                            Argument
                          </th>
                          <th className="border-b border-brand-muted/15 px-3 py-2 font-semibold text-brand-text-bright">
                            Type
                          </th>
                          <th className="border-b border-brand-muted/15 px-3 py-2 font-semibold text-brand-text-bright">
                            Description
                          </th>
                        </tr>
                      </thead>
                      <tbody>
                        {op.args.map((arg) => (
                          <tr key={arg.name}>
                            <td className="border-b border-brand-muted/10 px-3 py-2 align-top">
                              <code className="font-mono text-[0.85em] text-brand-accent">
                                {arg.name}
                              </code>
                              {arg.required && (
                                <span
                                  className="ml-1 text-brand-gold"
                                  title="required"
                                >
                                  *
                                </span>
                              )}
                            </td>
                            <td className="border-b border-brand-muted/10 px-3 py-2 align-top">
                              <code className="font-mono text-[0.8em] text-brand-muted">
                                {arg.type}
                              </code>
                            </td>
                            <td className="border-b border-brand-muted/10 px-3 py-2 align-top text-brand-muted">
                              {arg.description || arg.label}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                ) : (
                  <p className="mt-3 text-xs text-brand-muted/70">
                    No arguments.
                  </p>
                )}
              </div>
            ))}
          </div>
        </section>
      ))}

      <p className="mt-12 text-xs text-brand-muted/70">
        <span className="text-brand-gold">*</span> denotes a required argument.
        Surface badges show where an op lives in the app:{" "}
        <span className="text-brand-accent">panel</span> (rich UI),{" "}
        <span className="text-brand-gold">menu</span> (row/context action), or{" "}
        palette (palette-only).
      </p>
    </div>
  );
}
