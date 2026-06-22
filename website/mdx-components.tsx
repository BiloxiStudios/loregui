import type { MDXComponents } from "mdx/types";
import Image, { type ImageProps } from "next/image";
import type { ComponentPropsWithoutRef } from "react";

/**
 * Themed MDX element mapping for the /docs knowledge base.
 *
 * `@next/mdx` (App Router) reads this file to style raw markdown elements with
 * the LoreGUI / BiloxiStudios retrowave tokens (brand-*), so docs prose matches
 * the rest of the site without per-page wrapper components. Headings get
 * scroll-anchor ids via rehype is not configured here — instead the docs layout
 * provides the page chrome and these handle inline typography.
 */

function slugify(children: React.ReactNode): string | undefined {
  if (typeof children === "string") {
    return children
      .toLowerCase()
      .replace(/[^\w\s-]/g, "")
      .trim()
      .replace(/\s+/g, "-");
  }
  return undefined;
}

export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    h1: ({ children, ...props }: ComponentPropsWithoutRef<"h1">) => (
      <h1
        className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl"
        {...props}
      >
        {children}
      </h1>
    ),
    h2: ({ children, ...props }: ComponentPropsWithoutRef<"h2">) => (
      <h2
        id={slugify(children)}
        className="mt-12 scroll-mt-24 border-t border-brand-muted/10 pt-8 font-heading text-2xl font-bold tracking-tight text-brand-text-bright"
        {...props}
      >
        {children}
      </h2>
    ),
    h3: ({ children, ...props }: ComponentPropsWithoutRef<"h3">) => (
      <h3
        id={slugify(children)}
        className="mt-8 scroll-mt-24 font-heading text-xl font-semibold text-brand-text-bright"
        {...props}
      >
        {children}
      </h3>
    ),
    h4: ({ children, ...props }: ComponentPropsWithoutRef<"h4">) => (
      <h4
        className="mt-6 font-heading text-lg font-semibold text-brand-text"
        {...props}
      >
        {children}
      </h4>
    ),
    p: ({ children, ...props }: ComponentPropsWithoutRef<"p">) => (
      <p
        className="mt-4 text-base leading-relaxed text-brand-muted"
        {...props}
      >
        {children}
      </p>
    ),
    a: ({ children, href, ...props }: ComponentPropsWithoutRef<"a">) => {
      const external = href?.startsWith("http");
      return (
        <a
          href={href}
          className="font-medium text-brand-accent underline decoration-brand-accent/30 underline-offset-2 transition-colors hover:text-brand-accent-hover hover:decoration-brand-accent"
          {...(external
            ? { target: "_blank", rel: "noopener noreferrer" }
            : {})}
          {...props}
        >
          {children}
        </a>
      );
    },
    ul: ({ children, ...props }: ComponentPropsWithoutRef<"ul">) => (
      <ul
        className="mt-4 list-disc space-y-2 pl-6 text-base leading-relaxed text-brand-muted marker:text-brand-accent/60"
        {...props}
      >
        {children}
      </ul>
    ),
    ol: ({ children, ...props }: ComponentPropsWithoutRef<"ol">) => (
      <ol
        className="mt-4 list-decimal space-y-2 pl-6 text-base leading-relaxed text-brand-muted marker:text-brand-accent/60"
        {...props}
      >
        {children}
      </ol>
    ),
    li: ({ children, ...props }: ComponentPropsWithoutRef<"li">) => (
      <li className="pl-1" {...props}>
        {children}
      </li>
    ),
    strong: ({ children, ...props }: ComponentPropsWithoutRef<"strong">) => (
      <strong className="font-semibold text-brand-text" {...props}>
        {children}
      </strong>
    ),
    em: ({ children, ...props }: ComponentPropsWithoutRef<"em">) => (
      <em className="text-brand-text italic" {...props}>
        {children}
      </em>
    ),
    blockquote: ({
      children,
      ...props
    }: ComponentPropsWithoutRef<"blockquote">) => (
      <blockquote
        className="mt-6 border-l-2 border-brand-accent/50 bg-brand-surface/40 py-2 pl-4 text-base italic text-brand-muted"
        {...props}
      >
        {children}
      </blockquote>
    ),
    code: ({ children, ...props }: ComponentPropsWithoutRef<"code">) => (
      <code
        className="rounded bg-brand-deep/70 px-1.5 py-0.5 font-mono text-[0.85em] text-brand-accent"
        {...props}
      >
        {children}
      </code>
    ),
    pre: ({ children, ...props }: ComponentPropsWithoutRef<"pre">) => (
      <pre
        className="mt-6 overflow-x-auto rounded-lg border border-brand-muted/15 bg-brand-deep/70 p-4 font-mono text-[13px] leading-relaxed text-brand-text [&_code]:bg-transparent [&_code]:p-0 [&_code]:text-brand-text"
        {...props}
      >
        {children}
      </pre>
    ),
    hr: (props: ComponentPropsWithoutRef<"hr">) => (
      <hr className="my-10 border-brand-muted/10" {...props} />
    ),
    table: ({ children, ...props }: ComponentPropsWithoutRef<"table">) => (
      <div className="mt-6 overflow-x-auto rounded-lg border border-brand-muted/15">
        <table className="w-full border-collapse text-left text-sm" {...props}>
          {children}
        </table>
      </div>
    ),
    thead: ({ children, ...props }: ComponentPropsWithoutRef<"thead">) => (
      <thead className="bg-brand-surface-light/60" {...props}>
        {children}
      </thead>
    ),
    th: ({ children, ...props }: ComponentPropsWithoutRef<"th">) => (
      <th
        className="border-b border-brand-muted/15 px-4 py-2.5 font-semibold text-brand-text-bright"
        {...props}
      >
        {children}
      </th>
    ),
    td: ({ children, ...props }: ComponentPropsWithoutRef<"td">) => (
      <td
        className="border-b border-brand-muted/10 px-4 py-2.5 align-top text-brand-muted [&_code]:whitespace-nowrap"
        {...props}
      >
        {children}
      </td>
    ),
    img: (props: ImageProps) => (
      <Image
        sizes="100vw"
        width={1440}
        height={900}
        className="mt-6 w-full rounded-lg border border-brand-muted/20"
        {...props}
      />
    ),
    ...components,
  };
}
