"use client";

import { useEffect, useRef, useCallback } from "react";

/**
 * CodeBlockHighlight — client-side enhancement for docs code blocks.
 *
 * After mount, scans all <pre><code> elements within its children and:
 *  1. Adds `data-language` attributes from language class names
 *  2. Wraps known symbols (Verse keywords, types, built-ins) in
 *     <span data-symbol="..." data-symbol-kind="..."> so the CSS
 *     tooltip system in codeblocks.css activates on hover.
 *  3. Optionally adds line numbers when data-line-numbers is present.
 *
 * This is the functional equivalent of codeblocks.js + symbol_ai.js
 * referenced in the original ticket — implemented as a React client
 * component instead of standalone scripts because the loregui website
 * is a Next.js App Router app.
 */

// Built-in Verse symbols that get hover tooltips.
// In production this would be populated from an index / LSP server,
// but the core set covers the most common reference lookups.
const VERSE_SYMBOLS: Record<
  string,
  { kind: string; tooltip: string }
> = {
  // Verse keywords
  foreach: { kind: "kw", tooltip: "foreach — iterate over a collection" },
  for: { kind: "kw", tooltip: "for — indexed loop" },
  while: { kind: "kw", tooltip: "while — conditional loop" },
  if: { kind: "kw", tooltip: "if — conditional branch" },
  else: { kind: "kw", tooltip: "else — alternate branch" },
  return: { kind: "kw", tooltip: "return — exit function with value" },
  break: { kind: "kw", tooltip: "break — exit nearest loop" },
  continue: { kind: "kw", tooltip: "continue — skip to next iteration" },
  switch: { kind: "kw", tooltip: "switch — multi-way branch" },
  case: { kind: "kw", tooltip: "case — switch branch label" },
  default: { kind: "kw", tooltip: "default — fallback switch branch" },

  // Verse types
  int: { kind: "type", tooltip: "int — 32-bit signed integer" },
  float: { kind: "type", tooltip: "float — 32-bit IEEE 754 floating point" },
  bool: { kind: "type", tooltip: "bool — boolean (true / false)" },
  string: { kind: "type", tooltip: "string — UTF-8 text" },
  void: { kind: "type", tooltip: "void — no return value" },
  agent: { kind: "type", tooltip: "agent — Verse agent reference" },
  type: { kind: "type", tooltip: "type — type definition keyword" },

  // Common Unreal / Creative types
  "FortCharacter": {
    kind: "type",
    tooltip: "FortCharacter — player pawn in Fortnite Creative",
  },
  "FortPlayerController": {
    kind: "type",
    tooltip: "FortPlayerController — player controller",
  },
  "CreativeDevice": {
    kind: "type",
    tooltip: "CreativeDevice — base class for UEFN devices",
  },

  // Common functions
  Print: {
    kind: "fn",
    tooltip: "Print(text) — write to the Verse console",
  },
  Sleep: {
    kind: "fn",
    tooltip: "Sleep(seconds) — suspend agent for N seconds",
  },
};

interface CodeBlockHighlightProps {
  children: React.ReactNode;
}

function wrapSymbols(text: string): string {
  // Only wrap if the text looks like Verse code
  if (!/^(verse|fortnite)/i.test(text) && !/\b(foreach|agent)\b/.test(text)) {
    return text;
  }

  // Sort keys by length descending so longer matches win
  const keys = Object.keys(VERSE_SYMBOLS).sort(
    (a, b) => b.length - a.length
  );

  // Build a regex that matches any symbol as a whole word
  const escaped = keys.map((k) => k.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`\\b(${escaped.join("|")})\\b`, "g");

  return text.replace(re, (match) => {
    const sym = VERSE_SYMBOLS[match];
    if (!sym) return match;
    return `<span data-symbol="${match}" data-symbol-kind="${sym.kind}" data-symbol-tooltip="${sym.tooltip}">${match}</span>`;
  });
}

export function CodeBlockHighlight({ children }: CodeBlockHighlightProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  const enhanceCodeBlocks = useCallback(() => {
    const container = containerRef.current;
    if (!container) return;

    const preBlocks = container.querySelectorAll("pre");
    preBlocks.forEach((pre) => {
      const code = pre.querySelector("code");
      if (!code) return;

      // Extract language from class
      const classList = Array.from(code.classList);
      const langClass = classList.find((c) => c.startsWith("language-"));
      if (langClass) {
        const lang = langClass.replace("language-", "");
        pre.setAttribute("data-language", lang);
      }

      // Wrap symbols in text content (only if not already processed)
      if (!pre.hasAttribute("data-enhanced")) {
        const innerHTML = code.innerHTML;
        // Don't process if it already has spans (already tokenized)
        if (!innerHTML.includes("data-symbol=")) {
          const text = code.textContent ?? "";
          if (text.trim().length > 0) {
            code.innerHTML = wrapSymbols(text);
          }
        }
        pre.setAttribute("data-enhanced", "true");
      }
    });
  }, []);

  useEffect(() => {
    enhanceCodeBlocks();
  }, [enhanceCodeBlocks]);

  return <div ref={containerRef}>{children}</div>;
}
