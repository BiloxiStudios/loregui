import { Container } from "@/components/ui/Container";
import { LoreIcon, GithubIcon } from "@/components/icons";

const GITHUB_URL = "https://github.com/BiloxiStudios/loregui";
const STUDIO_URL = "https://biloxistudios.com";

const footerLinks: Record<string, { label: string; href: string; external?: boolean }[]> = {
  Product: [
    { label: "Features", href: "/#features" },
    { label: "Full API", href: "/#api" },
    { label: "Screenshots", href: "/#screens" },
    { label: "User guide", href: "/guide" },
    { label: "Documentation", href: "/docs" },
    { label: "Install", href: "/#install" },
  ],
  Project: [
    { label: "Lore on GitHub", href: GITHUB_URL, external: true },
    { label: "Releases", href: "https://github.com/BiloxiStudios/loregui/releases", external: true },
    { label: "Report an issue", href: "https://github.com/BiloxiStudios/loregui/issues", external: true },
  ],
  Studio: [
    { label: "Biloxi Studios", href: STUDIO_URL, external: true },
  ],
};

export function Footer() {
  const currentYear = new Date().getFullYear();

  return (
    <footer className="border-t border-brand-muted/10 bg-brand-deep py-12">
      <Container>
        <div className="grid gap-8 sm:grid-cols-2 lg:grid-cols-4">
          <div>
            <a
              href="/#top"
              className="flex items-center gap-2"
              aria-label="LoreGUI Home"
            >
              <LoreIcon className="h-7 w-7 text-brand-accent" />
              <span className="font-heading text-lg font-bold text-brand-text-bright">
                Lore<span className="text-brand-accent">GUI</span>
              </span>
            </a>
            <p className="mt-3 text-sm text-brand-muted">
              A community-built desktop GUI for Lore. Crafted by{" "}
              <a
                href={STUDIO_URL}
                className="text-brand-text hover:text-brand-text-bright"
                target="_blank"
                rel="noopener noreferrer"
              >
                Biloxi Studios
              </a>
              .
            </p>
            <div className="mt-4 flex gap-3">
              <a
                href={GITHUB_URL}
                className="text-brand-muted transition-colors hover:text-brand-text-bright"
                aria-label="GitHub"
                target="_blank"
                rel="noopener noreferrer"
              >
                <GithubIcon className="h-5 w-5" />
              </a>
            </div>
          </div>

          {Object.entries(footerLinks).map(([category, links]) => (
            <div key={category}>
              <h3 className="text-sm font-semibold text-brand-text-bright">
                {category}
              </h3>
              <ul className="mt-3 space-y-2" role="list">
                {links.map((link) => (
                  <li key={link.label}>
                    <a
                      href={link.href}
                      className="text-sm text-brand-muted transition-colors hover:text-brand-text-bright"
                      {...(link.external
                        ? { target: "_blank", rel: "noopener noreferrer" }
                        : {})}
                    >
                      {link.label}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        <div className="mt-12 space-y-3 border-t border-brand-muted/10 pt-8">
          <p className="text-center text-xs leading-relaxed text-brand-muted">
            LoreGUI is an independent community project. It is{" "}
            <span className="text-brand-text">
              not affiliated with, sponsored by, or endorsed by Epic Games, Inc.
            </span>{" "}
            &ldquo;Lore&rdquo; is a trademark of Epic Games, Inc. LoreGUI is
            released under the MIT License.
          </p>
          <p className="text-center text-xs text-brand-muted">
            &copy; {currentYear} LoreGUI contributors. Built by Biloxi Studios.
          </p>
        </div>
      </Container>
    </footer>
  );
}
