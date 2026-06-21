"use client";

import { useState } from "react";
import { Container } from "@/components/ui/Container";
import { Button } from "@/components/ui/Button";
import { LoreIcon, MenuIcon, XIcon } from "@/components/icons";

const GITHUB_URL = "https://github.com/BiloxiStudios/loregui";
const RELEASES_URL = "https://github.com/BiloxiStudios/loregui/releases";

const navLinks = [
  { label: "Features", href: "/#features" },
  { label: "API", href: "/#api" },
  { label: "Screens", href: "/#screens" },
  { label: "Guide", href: "/guide" },
  { label: "Install", href: "/#install" },
];

export function Header() {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  return (
    <header className="fixed top-0 z-50 w-full border-b border-brand-muted/10 bg-brand-deep/80 backdrop-blur-lg">
      <Container>
        <div className="flex h-16 items-center justify-between">
          <a
            href="/#top"
            className="flex items-center gap-2"
            aria-label="LoreGUI Home"
          >
            <LoreIcon className="h-8 w-8 text-brand-accent" />
            <span className="font-heading text-xl font-bold text-brand-text-bright">
              Lore<span className="text-brand-accent">GUI</span>
            </span>
          </a>

          <nav
            className="hidden items-center gap-8 md:flex"
            aria-label="Main navigation"
          >
            {navLinks.map((link) => (
              <a
                key={link.href}
                href={link.href}
                className="text-sm font-medium text-brand-muted transition-colors hover:text-brand-text-bright"
              >
                {link.label}
              </a>
            ))}
          </nav>

          <div className="hidden items-center gap-3 md:flex">
            <Button
              variant="ghost"
              size="sm"
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
            >
              GitHub
            </Button>
            <Button variant="primary" size="sm" href={RELEASES_URL}>
              Download
            </Button>
          </div>

          <button
            type="button"
            className="text-brand-muted md:hidden"
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            aria-label={mobileMenuOpen ? "Close menu" : "Open menu"}
            aria-expanded={mobileMenuOpen}
          >
            {mobileMenuOpen ? (
              <XIcon className="h-6 w-6" />
            ) : (
              <MenuIcon className="h-6 w-6" />
            )}
          </button>
        </div>

        {mobileMenuOpen && (
          <div className="border-t border-brand-muted/10 pb-4 md:hidden">
            <nav
              className="flex flex-col gap-1 pt-4"
              aria-label="Mobile navigation"
            >
              {navLinks.map((link) => (
                <a
                  key={link.href}
                  href={link.href}
                  className="rounded-lg px-4 py-2 text-sm font-medium text-brand-muted transition-colors hover:bg-brand-surface-light hover:text-brand-text-bright"
                  onClick={() => setMobileMenuOpen(false)}
                >
                  {link.label}
                </a>
              ))}
              <div className="mt-4 flex flex-col gap-2 px-4">
                <Button
                  variant="secondary"
                  size="sm"
                  href={GITHUB_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  GitHub
                </Button>
                <Button variant="primary" size="sm" href={RELEASES_URL}>
                  Download
                </Button>
              </div>
            </nav>
          </div>
        )}
      </Container>
    </header>
  );
}
