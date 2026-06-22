import { Header } from "@/components/Header";
import { Footer } from "@/components/Footer";
import { Container } from "@/components/ui/Container";
import { DocsSidebar } from "@/components/docs/DocsSidebar";
import { DocsPager } from "@/components/docs/DocsPager";

/**
 * Shared chrome for the /docs knowledge base. Reuses the marketing site's
 * Header and Footer so the docs read as part of loregui.com, adds the docs
 * sidebar (with search) on the left and a prev/next pager under the content.
 *
 * Each docs page is an MDX file; its prose is styled by the global
 * `mdx-components.tsx` element mapping. This layout owns the page frame only.
 */
export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <>
      <Header />
      <main className="pt-24 pb-20 sm:pt-28">
        <Container>
          <div className="lg:grid lg:grid-cols-[16rem_minmax(0,1fr)] lg:gap-12">
            <aside className="hidden lg:block">
              <div className="sticky top-24 max-h-[calc(100vh-7rem)] overflow-y-auto pr-2">
                <DocsSidebar />
              </div>
            </aside>

            <article className="min-w-0">
              {/* Mobile sidebar / search lives inline above the content. */}
              <details className="mb-8 rounded-lg border border-brand-muted/15 bg-brand-surface/40 p-4 lg:hidden">
                <summary className="cursor-pointer text-sm font-semibold text-brand-text-bright">
                  Browse & search docs
                </summary>
                <div className="mt-4">
                  <DocsSidebar />
                </div>
              </details>

              <div className="max-w-3xl">
                {children}
                <DocsPager />
              </div>
            </article>
          </div>
        </Container>
      </main>
      <Footer />
    </>
  );
}
