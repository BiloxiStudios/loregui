"use client";

import { useState } from "react";
import Image from "next/image";
import { AppWindow } from "@/components/mockups/AppWindow";

interface Shot {
  src: string;
  alt: string;
}

interface Caption {
  title: string;
  body: string;
}

interface ThemeSwapShotProps {
  /** Title shown in the faux desktop-window chrome. */
  windowTitle: string;
  /** Default (dark-theme) screenshot — establishes the aspect box. */
  dark: Shot;
  /** Light-theme screenshot that crossfades in on hover / focus / tap. */
  light: Shot;
  /** Short affordance label, e.g. "Light theme" or "See it on Windows". */
  hint?: string;
  /** Intrinsic size of the dark shot (defaults to the 1440×900 gallery size). */
  width?: number;
  height?: number;
  /** next/image responsive `sizes` hint. */
  sizes?: string;
  priority?: boolean;
  /** Figure className — e.g. a column span. */
  className?: string;
  /** Default caption. Omit for an image-only shot (e.g. the hero). */
  caption?: Caption;
  /**
   * Flipped caption — provide ONLY when the light shot is a *different*
   * surface/function than the dark one. When present the caption text
   * crossfades along with the image; when absent the caption stays put.
   */
  captionHover?: Caption;
  /** Extra content rendered under the frame (e.g. the hero's ⌘K hint). */
  children?: React.ReactNode;
}

/**
 * A captioned screenshot that shows the DARK theme by default and crossfades to
 * the LIGHT theme on hover — a live demo that the app is fully themeable.
 *
 * - Same-surface pair (just re-themed): pass `caption` only → image swaps, caption stays.
 * - Different surface/function: also pass `captionHover` → caption text flips too.
 *
 * Accessibility: both images carry alt text; the reveal is CSS-driven via
 * `group-hover` / `group-focus-within`, honours `prefers-reduced-motion`
 * (instant swap, no fade), and exposes a real toggle button so touch and
 * keyboard users can flip the theme without a pointer hover.
 */
export function ThemeSwapShot({
  windowTitle,
  dark,
  light,
  hint = "Light theme",
  width = 1440,
  height = 900,
  sizes = "(min-width: 1024px) 50vw, 100vw",
  priority,
  className = "",
  caption,
  captionHover,
  children,
}: ThemeSwapShotProps) {
  const [revealed, setRevealed] = useState(false);
  const Wrapper = caption ? "figure" : "div";

  return (
    <Wrapper
      className={`group ${revealed ? "is-revealed" : ""} ${className}`.trim()}
    >
      <div className="relative">
        <AppWindow title={windowTitle}>
          <div className="relative">
            {/* Default: dark theme. Sets the box height the light shot fills. */}
            <Image
              src={dark.src}
              alt={dark.alt}
              width={width}
              height={height}
              sizes={sizes}
              className="w-full"
              priority={priority}
            />
            {/* On hover/focus/tap: light theme crossfades over the same box. */}
            <Image
              src={light.src}
              alt={light.alt}
              fill
              sizes={sizes}
              aria-hidden={!revealed}
              className="object-cover object-center opacity-0 transition-opacity duration-500 ease-out group-hover:opacity-100 group-focus-within:opacity-100 group-[.is-revealed]:opacity-100 motion-reduce:transition-none"
            />
          </div>
        </AppWindow>

        {/* Affordance + touch/keyboard toggle. */}
        <button
          type="button"
          onClick={() => setRevealed((v) => !v)}
          aria-pressed={revealed}
          aria-label={
            revealed
              ? "Show the dark theme screenshot"
              : "Show the light theme screenshot"
          }
          className="absolute top-3 right-3 z-10 inline-flex items-center gap-1.5 rounded-full border border-brand-muted/30 bg-brand-deep-light/80 px-2.5 py-1 font-mono text-[11px] leading-none text-brand-muted shadow-sm backdrop-blur-sm transition-colors group-hover:border-brand-accent/40 group-hover:text-brand-text hover:border-brand-accent/50 hover:text-brand-text-bright focus-visible:ring-2 focus-visible:ring-brand-accent/60 focus-visible:outline-none"
        >
          <span aria-hidden="true">{revealed ? "🌙" : "☀️"}</span>
          <span>{revealed ? "Dark theme" : hint}</span>
        </button>
      </div>

      {caption && (
        <figcaption className="mt-4 grid">
          <div
            className={`col-start-1 row-start-1 ${
              captionHover
                ? "transition-opacity duration-500 ease-out group-hover:opacity-0 group-focus-within:opacity-0 group-[.is-revealed]:opacity-0 motion-reduce:transition-none"
                : ""
            }`}
          >
            <h3 className="font-heading text-base font-semibold text-brand-text-bright">
              {caption.title}
            </h3>
            <p className="mt-1 text-sm leading-relaxed text-brand-muted">
              {caption.body}
            </p>
          </div>
          {captionHover && (
            <div className="col-start-1 row-start-1 opacity-0 transition-opacity duration-500 ease-out group-hover:opacity-100 group-focus-within:opacity-100 group-[.is-revealed]:opacity-100 motion-reduce:transition-none">
              <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                {captionHover.title}
              </h3>
              <p className="mt-1 text-sm leading-relaxed text-brand-muted">
                {captionHover.body}
              </p>
            </div>
          )}
        </figcaption>
      )}

      {children}
    </Wrapper>
  );
}
