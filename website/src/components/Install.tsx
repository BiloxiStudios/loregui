import { Container } from "@/components/ui/Container";
import { Card } from "@/components/ui/Card";
import { Button } from "@/components/ui/Button";
import { CodeBlock } from "@/components/CodeBlock";
import {
  WindowsIcon,
  AppleIcon,
  LinuxIcon,
  TerminalIcon,
} from "@/components/icons";

const RELEASES_URL = "https://github.com/BiloxiStudios/loregui/releases";

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
            Grab a signed installer, or use your platform&rsquo;s package
            manager. One binary, no daemon to configure.
          </p>
        </div>

        <div className="mx-auto mt-16 grid max-w-5xl gap-6 lg:grid-cols-3">
          {/* winget */}
          <Card className="flex flex-col">
            <div className="mb-3 flex items-center gap-2">
              <WindowsIcon className="h-5 w-5 text-brand-accent" />
              <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                Windows · winget
              </h3>
            </div>
            <p className="mb-4 text-sm text-brand-muted">
              The recommended path on Windows 10/11.
            </p>
            <CodeBlock lines={["$ winget install LoreGUI.LoreGUI"]} />
          </Card>

          {/* scoop */}
          <Card className="flex flex-col">
            <div className="mb-3 flex items-center gap-2">
              <TerminalIcon className="h-5 w-5 text-brand-accent" />
              <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                Windows · scoop
              </h3>
            </div>
            <p className="mb-4 text-sm text-brand-muted">
              Prefer scoop? Add the bucket and install.
            </p>
            <CodeBlock
              lines={[
                "$ scoop bucket add loregui https://github.com/loregui/scoop-bucket",
                "$ scoop install loregui",
              ]}
            />
          </Card>

          {/* macOS / Linux */}
          <Card className="flex flex-col">
            <div className="mb-3 flex items-center gap-2">
              <AppleIcon className="h-5 w-5 text-brand-accent" />
              <LinuxIcon className="h-5 w-5 text-brand-accent" />
              <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                macOS &amp; Linux
              </h3>
            </div>
            <p className="mb-4 text-sm text-brand-muted">
              Homebrew on macOS, or the AppImage on Linux.
            </p>
            <CodeBlock
              lines={[
                "$ brew install --cask loregui",
                "# or download the .AppImage below",
              ]}
            />
          </Card>
        </div>

        {/* Direct download */}
        <div className="mx-auto mt-8 max-w-5xl">
          <Card highlight className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <h3 className="font-heading text-base font-semibold text-brand-text-bright">
                Prefer a direct download?
              </h3>
              <p className="mt-1 text-sm text-brand-muted">
                Signed installers for every platform (.msi, .dmg, .AppImage,
                .deb) live on GitHub Releases.
              </p>
            </div>
            <div className="flex shrink-0 flex-wrap gap-3">
              <Button variant="primary" size="md" href={RELEASES_URL}>
                <WindowsIcon className="mr-2 h-5 w-5" />
                Windows
              </Button>
              <Button variant="secondary" size="md" href={RELEASES_URL}>
                <AppleIcon className="mr-2 h-5 w-5" />
                macOS
              </Button>
              <Button variant="secondary" size="md" href={RELEASES_URL}>
                <LinuxIcon className="mr-2 h-5 w-5" />
                Linux
              </Button>
            </div>
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
