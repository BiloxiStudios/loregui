import { Container } from "@/components/ui/Container";
import { Button } from "@/components/ui/Button";
import { GradientText } from "@/components/ui/GradientText";
import { Badge } from "@/components/ui/Badge";
import { ArrowRightIcon, GithubIcon, WindowsIcon } from "@/components/icons";
import { AppWindow } from "@/components/mockups/AppWindow";
import { StatusMockup } from "@/components/mockups/StatusMockup";

const RELEASES_URL = "https://github.com/BiloxiStudios/loregui/releases/download/v0.1.0-alpha/LoreGUI_0.1.0_x64-setup.exe";
const GITHUB_URL = "https://github.com/BiloxiStudios/loregui";

export function Hero() {
  return (
    <section
      id="top"
      className="relative overflow-hidden pt-32 pb-20 sm:pt-40 sm:pb-32"
    >
      {/* Background glows */}
      <div className="pointer-events-none absolute inset-0" aria-hidden="true">
        <div className="absolute top-0 left-1/2 h-[600px] w-[800px] -translate-x-1/2 rounded-full bg-brand-accent/5 blur-3xl" />
        <div className="absolute top-40 right-0 h-[400px] w-[400px] rounded-full bg-brand-gold/5 blur-3xl" />
        <div
          className="absolute inset-0 opacity-[0.04]"
          style={{
            backgroundImage:
              "linear-gradient(to right, #8896b5 1px, transparent 1px), linear-gradient(to bottom, #8896b5 1px, transparent 1px)",
            backgroundSize: "48px 48px",
            maskImage:
              "radial-gradient(ellipse 60% 50% at 50% 0%, black, transparent)",
            WebkitMaskImage:
              "radial-gradient(ellipse 60% 50% at 50% 0%, black, transparent)",
          }}
        />
      </div>

      <Container className="relative">
        <div className="mx-auto max-w-4xl text-center">
          <Badge variant="accent" className="mb-6">
            Community project &middot; Built on Lore&rsquo;s native API
          </Badge>

          <h1 className="font-heading text-4xl font-bold tracking-tight sm:text-6xl lg:text-7xl">
            A beautiful desktop GUI for{" "}
            <GradientText>Lore.</GradientText>
          </h1>

          <p className="mx-auto mt-6 max-w-2xl text-lg text-brand-muted sm:text-xl">
            LoreGUI is a fast, cross-platform desktop client for{" "}
            <span className="text-brand-text">Lore</span> &mdash; Epic&rsquo;s
            next-generation version control for source code and huge binary
            assets. Stage, branch, merge, diff, and lock files without touching
            the command line.
          </p>

          <div className="mt-10 flex flex-col items-center justify-center gap-4 sm:flex-row">
            <Button variant="primary" size="lg" href={RELEASES_URL}>
              <WindowsIcon className="mr-2 h-5 w-5" />
              Download for Windows
            </Button>
            <Button
              variant="secondary"
              size="lg"
              href={GITHUB_URL}
              target="_blank"
              rel="noopener noreferrer"
            >
              <GithubIcon className="mr-2 h-5 w-5" />
              View on GitHub
            </Button>
          </div>

          <p className="mt-4 text-sm text-brand-muted">
            Free and open source. Windows, macOS &amp; Linux &middot; single
            installer, no daemon.
            <a
              href="#install"
              className="ml-1 inline-flex items-center font-medium text-brand-accent transition-colors hover:text-brand-gold"
            >
              winget &amp; scoop too
              <ArrowRightIcon className="ml-1 h-4 w-4" />
            </a>
          </p>
        </div>

        {/* Hero app mockup */}
        <div className="relative mx-auto mt-16 max-w-5xl">
          <AppWindow title="LoreGUI — astral-engine">
            <StatusMockup />
          </AppWindow>
          <div
            className="pointer-events-none absolute -inset-4 -z-10 rounded-xl bg-brand-accent/5 blur-2xl"
            aria-hidden="true"
          />
        </div>
      </Container>
    </section>
  );
}
