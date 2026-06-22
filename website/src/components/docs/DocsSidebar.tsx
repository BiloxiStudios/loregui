"use client";

import { useMemo, useState } from "react";
import { usePathname } from "next/navigation";
import Link from "next/link";
import { DOCS_NAV, DOCS_PAGES } from "@/lib/docs-nav";

/**
 * The /docs knowledge-base sidebar: a grouped table of contents plus a simple
 * client-side fuzzy search over page titles and descriptions. No external search
 * service — the index is the static `DOCS_PAGES` list, so it ships in the bundle
 * and works offline.
 */
export function DocsSidebar({ onNavigate }: { onNavigate?: () => void }) {
  const pathname = usePathname();
  const [query, setQuery] = useState("");

  const results = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return null;
    return DOCS_PAGES.filter(
      (p) =>
        p.title.toLowerCase().includes(q) ||
        p.description.toLowerCase().includes(q) ||
        p.href.toLowerCase().includes(q),
    );
  }, [query]);

  return (
    <nav aria-label="Docs navigation" className="flex flex-col gap-6">
      <div>
        <label htmlFor="docs-search" className="sr-only">
          Search docs
        </label>
        <input
          id="docs-search"
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search docs…"
          className="w-full rounded-lg border border-brand-muted/20 bg-brand-deep/60 px-3 py-2 text-sm text-brand-text-bright placeholder:text-brand-muted/60 focus:border-brand-accent/50 focus:outline-none focus:ring-1 focus:ring-brand-accent/40"
        />
      </div>

      {results ? (
        <div>
          <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-brand-muted">
            {results.length} result{results.length === 1 ? "" : "s"}
          </p>
          {results.length === 0 ? (
            <p className="text-sm text-brand-muted">
              No pages match “{query}”.
            </p>
          ) : (
            <ul className="space-y-1" role="list">
              {results.map((p) => (
                <li key={p.href}>
                  <Link
                    href={p.href}
                    onClick={onNavigate}
                    className="block rounded-md px-2 py-1.5 text-sm text-brand-muted transition-colors hover:bg-brand-surface-light hover:text-brand-text-bright"
                  >
                    <span className="block font-medium text-brand-text">
                      {p.title}
                    </span>
                    <span className="block text-xs text-brand-muted">
                      {p.description}
                    </span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : (
        DOCS_NAV.map((section) => (
          <div key={section.title}>
            <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-brand-text-bright">
              {section.title}
            </h3>
            <ul className="space-y-0.5 border-l border-brand-muted/15" role="list">
              {section.pages.map((page) => {
                const active = pathname === page.href;
                return (
                  <li key={page.href}>
                    <Link
                      href={page.href}
                      onClick={onNavigate}
                      aria-current={active ? "page" : undefined}
                      className={`-ml-px block border-l-2 py-1.5 pl-4 text-sm transition-colors ${
                        active
                          ? "border-brand-accent font-medium text-brand-accent"
                          : "border-transparent text-brand-muted hover:border-brand-muted/40 hover:text-brand-text-bright"
                      }`}
                    >
                      {page.title}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))
      )}
    </nav>
  );
}
