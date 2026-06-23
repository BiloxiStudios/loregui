// runTest.ts — entry point for the @vscode/test-electron harness.
//
// Downloads a pinned VS Code build, launches it headless, and points it at the
// compiled extension (extensionDevelopmentPath) + the compiled mocha suite
// (extensionTestsPath). The suite (suite/index.ts) builds a REAL scratch lore
// repo with the `lorevm` CLI and opens it as the test workspace.
//
// Run with `xvfb-run -a npm test` on Linux CI — VS Code needs a display server.

import * as path from 'path';
import * as os from 'os';
import * as fs from 'fs';
import { runTests } from '@vscode/test-electron';

/**
 * Load LORE_TEST_WORKSPACE / LOREVM_BIN from the test-workspace.env file the
 * seed step (pretest) wrote, so `npm test` works without shell env juggling.
 * Existing process.env values win (CI can still override).
 */
function loadSeedEnv(): void {
  const envPath = path.resolve(__dirname, '../../', 'test-workspace.env');
  if (!fs.existsSync(envPath)) {
    return;
  }
  for (const line of fs.readFileSync(envPath, 'utf8').split('\n')) {
    const m = /^([A-Z0-9_]+)=(.*)$/.exec(line.trim());
    if (m && !process.env[m[1]]) {
      process.env[m[1]] = m[2];
    }
  }
}

async function main(): Promise<void> {
  try {
    loadSeedEnv();
    // The folder containing package.json (the extension manifest).
    const extensionDevelopmentPath = path.resolve(__dirname, '../../');
    // The compiled mocha bootstrap (out/test/suite/index.js).
    const extensionTestsPath = path.resolve(__dirname, './suite/index');

    // Give VS Code an isolated, throwaway user-data dir so a developer's real
    // settings/extensions never leak into the run (and vice-versa).
    const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'lore-vscode-user-'));

    // A scratch workspace folder is created INSIDE the suite (it needs the
    // lorevm binary to seed a real .lore repo first), so we don't pass a
    // launchArgs workspace here — the suite opens it via vscode.openFolder is
    // not reliable for the first-window case, so instead the suite seeds the
    // repo into a dir whose path it reads from LORE_TEST_WORKSPACE and we pass
    // that as the launch folder. seedWorkspace.ts (run below) creates it.
    const workspace = process.env.LORE_TEST_WORKSPACE;
    if (!workspace) {
      throw new Error(
        'LORE_TEST_WORKSPACE not set — run `npm run pretest` (seedWorkspace) first.',
      );
    }

    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs: [
        workspace,
        '--disable-extensions', // only OUR extension under dev loads
        '--disable-workspace-trust',
        `--user-data-dir=${userDataDir}`,
        '--disable-gpu',
      ],
      extensionTestsEnv: {
        // Forward the resolved lorevm path so the extension finds the fresh
        // binary the seed step built/located.
        LOREVM_BIN: process.env.LOREVM_BIN ?? '',
        LORE_TEST_WORKSPACE: workspace,
      },
    });
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error('Failed to run tests:', err);
    process.exit(1);
  }
}

void main();
