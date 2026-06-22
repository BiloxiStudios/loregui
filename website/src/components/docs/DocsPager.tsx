"use client";

import { usePathname } from "next/navigation";
import Link from "next/link";
import { DOCS_PAGES } from "@/lib/docs-nav";
import { ArrowRightIcon } from "@/components/icons";

/** Previous / next page links at the foot of each docs page. */
export function DocsPager() {
  const pathname = usePathname();
  const idx = DOCS_PAGES.findIndex((p) => p.href === pathname);
  if (idx === -1) return null;
  const prev = idx > 0 ? DOCS_PAGES[idx - 1] : null;
  const next = idx < DOCS_PAGES.length - 1 ? DOCS_PAGES[idx + 1] : null;

  return (
    <div className="mt-16 grid gap-4 border-t border-brand-muted/10 pt-8 sm:grid-cols-2">
      {prev ? (
        <Link
          href={prev.href}
          className="group rounded-lg border border-brand-muted/15 bg-brand-surface/40 px-4 py-3 transition-colors hover:border-brand-accent/40"
        >
          <span className="flex items-center gap-1.5 text-xs text-brand-muted">
            <ArrowRightIcon className="h-3.5 w-3.5 rotate-180" />
            Previous
          </span>
          <span className="mt-1 block font-medium text-brand-text-bright group-hover:text-brand-accent">
            {prev.title}
          </span>
        </Link>
      ) : (
        <span />
      )}
      {next ? (
        <Link
          href={next.href}
          className="group rounded-lg border border-brand-muted/15 bg-brand-surface/40 px-4 py-3 text-right transition-colors hover:border-brand-accent/40"
        >
          <span className="flex items-center justify-end gap-1.5 text-xs text-brand-muted">
            Next
            <ArrowRightIcon className="h-3.5 w-3.5" />
          </span>
          <span className="mt-1 block font-medium text-brand-text-bright group-hover:text-brand-accent">
            {next.title}
          </span>
        </Link>
      ) : (
        <span />
      )}
    </div>
  );
}
