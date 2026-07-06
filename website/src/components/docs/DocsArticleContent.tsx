"use client";

import { CodeBlockHighlight } from "@/components/CodeBlockHighlight";
import { DocsPager } from "@/components/docs/DocsPager";

/**
 * Client-side wrapper for docs article content.
 * Wraps MDX prose in CodeBlockHighlight so all <pre><code> blocks
 * get syntax highlighting token classes and symbol hover tooltips
 * (codeblocks.css + the symbol AI layer).
 *
 * This is the functional equivalent of loading codeblocks.js and
 * symbol_ai.js on every docs page — implemented as a React client
 * component for the Next.js App Router.
 */
export function DocsArticleContent({ children }: { children: React.ReactNode }) {
  return (
    <CodeBlockHighlight>
      {children}
      <DocsPager />
    </CodeBlockHighlight>
  );
}
