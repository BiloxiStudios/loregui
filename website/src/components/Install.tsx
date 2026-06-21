import { Container } from "@/components/ui/Container";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { WindowsIcon, AppleIcon, LinuxIcon } from "@/components/icons";

// The releases page — the rolling `nightly` build (current main) sits at the top,
// alongside any tagged stable releases. No stale version is pinned here.
const RELEASES_LATEST = "https://github.com/BiloxiStudios/loregui/releases";

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

        {/* Real, working download — signed installers on GitHub Releases. */}
        <div className="mx-auto mt-16 max-w-3xl">
          <Card highlight className="flex flex-col gap-6">
            <div className="text-center">
              <h3 className="font-heading text-lg font-semibold text-brand-text-bright">
                Get the latest release
              </h3>
              <p className="mt-1 text-sm text-brand-muted">
                Signed installers built by CI live on GitHub Releases — Windows
                <span className="text-brand-text-bright"> (.exe / .msi)</span> and
                Linux
                <span className="text-brand-text-bright"> (.deb / .AppImage)</span>
                .
              </p>
            </div>
            <div className="flex flex-wrap items-center justify-center gap-3">
              <Button variant="primary" size="md" href={RELEASES_LATEST}>
                <WindowsIcon className="mr-2 h-5 w-5" />
                Download for Windows
              </Button>
              <Button variant="secondary" size="md" href={RELEASES_LATEST}>
                <LinuxIcon className="mr-2 h-5 w-5" />
                Download for Linux
              </Button>
              <span className="inline-flex items-center gap-2 rounded-md border border-brand-muted/20 px-3 py-2 text-sm text-brand-muted">
                <AppleIcon className="h-5 w-5" />
                macOS — coming soon
              </span>
            </div>
            <p className="text-center text-xs text-brand-muted">
              Package managers (winget, Scoop, Homebrew) are on the roadmap.
              In-app auto-update is coming so the client keeps itself current.
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
