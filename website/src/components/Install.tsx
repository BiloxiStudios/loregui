import { Container } from "@/components/ui/Container";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { WindowsIcon, AppleIcon, LinuxIcon } from "@/components/icons";
import { DOWNLOADS, RELEASES_PAGE } from "@/lib/downloads";

export function Install() {
  return (
    <section
      id="install"
      className="border-t border-brand-muted/10 bg-brand-deep-light/40 py-20 sm:py-32"
    >
      <Container>
        <div className="mx-auto max-w-2xl text-center">
          <h2 className="font-heading text-3xl font-bold tracking-tight text-brand-text-bright sm:text-4xl">
            Install in seconds
          </h2>
          <p className="mt-4 text-lg text-brand-muted">
            Download a signed installer for your platform. One binary, no daemon
            to configure.
          </p>
        </div>

        {/* Real, working downloads — installers from the CI-maintained
            `nightly` release. Each link is a direct asset URL. */}
        <div className="mx-auto mt-16 max-w-3xl">
          <Card highlight className="flex flex-col gap-6">
            <div className="text-center">
              <h3 className="font-heading text-lg font-semibold text-brand-text-bright">
                Get the latest build
              </h3>
              <p className="mt-1 text-sm text-brand-muted">
                CI builds the current <code className="text-brand-text-bright">main</code> on
                every push and publishes installers to GitHub Releases (the
                rolling <span className="text-brand-text-bright">nightly</span> build).
              </p>
            </div>

            <div className="grid gap-4 sm:grid-cols-3">
              {/* Windows */}
              <div className="flex flex-col items-center gap-2 rounded-lg border border-brand-muted/15 p-4 text-center">
                <WindowsIcon className="h-6 w-6 text-brand-text-bright" />
                <span className="text-sm font-semibold text-brand-text-bright">
                  Windows
                </span>
                <Button
                  variant="primary"
                  size="sm"
                  href={DOWNLOADS.windowsExe}
                  className="w-full"
                >
                  Installer (.exe)
                </Button>
                <a
                  href={DOWNLOADS.windowsMsi}
                  className="text-xs font-medium text-brand-accent transition-colors hover:text-brand-gold"
                >
                  or .msi
                </a>
              </div>

              {/* Linux */}
              <div className="flex flex-col items-center gap-2 rounded-lg border border-brand-muted/15 p-4 text-center">
                <LinuxIcon className="h-6 w-6 text-brand-text-bright" />
                <span className="text-sm font-semibold text-brand-text-bright">
                  Linux
                </span>
                <Button
                  variant="secondary"
                  size="sm"
                  href={DOWNLOADS.linuxAppImage}
                  className="w-full"
                >
                  AppImage
                </Button>
                <span className="text-xs text-brand-muted">
                  or{" "}
                  <a
                    href={DOWNLOADS.linuxDeb}
                    className="font-medium text-brand-accent transition-colors hover:text-brand-gold"
                  >
                    .deb
                  </a>{" "}
                  /{" "}
                  <a
                    href={DOWNLOADS.linuxRpm}
                    className="font-medium text-brand-accent transition-colors hover:text-brand-gold"
                  >
                    .rpm
                  </a>
                </span>
              </div>

              {/* macOS */}
              <div className="flex flex-col items-center gap-2 rounded-lg border border-brand-muted/15 p-4 text-center">
                <AppleIcon className="h-6 w-6 text-brand-text-bright" />
                <span className="text-sm font-semibold text-brand-text-bright">
                  macOS
                </span>
                <Button
                  variant="secondary"
                  size="sm"
                  href={DOWNLOADS.macosDmg}
                  className="w-full"
                >
                  Apple Silicon (.dmg)
                </Button>
                <span className="text-xs text-brand-muted">
                  unsigned build — may need Gatekeeper override
                </span>
              </div>
            </div>

            <p className="text-center text-xs text-brand-muted">
              Need a different format or an older build?{" "}
              <a
                href={RELEASES_PAGE}
                target="_blank"
                rel="noopener noreferrer"
                className="font-medium text-brand-accent transition-colors hover:text-brand-gold"
              >
                Browse all GitHub Releases
              </a>
              . Package managers (winget, Scoop, Homebrew) and in-app
              auto-update are on the roadmap.
            </p>
          </Card>
        </div>

        <p className="mx-auto mt-8 max-w-3xl text-center text-sm text-brand-muted">
          LoreGUI ships as a standalone desktop app — but it&rsquo;s also
          designed to embed into larger tooling: drop the same UI into your
          studio&rsquo;s launcher or pipeline dashboard and drive Lore from
          there.
        </p>
      </Container>
    </section>
  );
}
