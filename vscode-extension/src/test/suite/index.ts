// suite/index.ts — mocha bootstrap run INSIDE the VS Code extension host.
//
// @vscode/test-electron calls the exported `run()` after VS Code has launched
// and the test workspace (the seeded .lore repo) is open. We collect every
// compiled *.test.js under this directory and run them through mocha's
// programmatic API, resolving/rejecting the returned promise on the result so
// the harness exit code reflects pass/fail.

import * as path from 'path';
import { glob } from 'glob';
import Mocha from 'mocha';

export function run(): Promise<void> {
  const mocha = new Mocha({
    ui: 'tdd',
    color: true,
    // The first activation + lorevm shell-outs can be slow on cold CI runners.
    timeout: 60_000,
    slow: 5_000,
  });

  const testsRoot = path.resolve(__dirname, '.');

  return new Promise((resolve, reject) => {
    glob('**/*.test.js', { cwd: testsRoot })
      .then((files: string[]) => {
        for (const f of files) {
          mocha.addFile(path.resolve(testsRoot, f));
        }
        try {
          mocha.run((failures: number) => {
            if (failures > 0) {
              reject(new Error(`${failures} test(s) failed.`));
            } else {
              resolve();
            }
          });
        } catch (err) {
          reject(err as Error);
        }
      })
      .catch((err: unknown) => reject(err as Error));
  });
}
