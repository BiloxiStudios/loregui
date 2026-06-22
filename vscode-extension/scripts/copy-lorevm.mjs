import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

// copy-lorevm.mjs — pulls the lorevm binary into the extension's bin/ folder.
// This is used during the `vscode:prepublish` step (via `npm run package`)
// to bundle the binary for marketplace delivery.

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.join(__dirname, '..', '..');
const extRoot = path.join(__dirname, '..');

const BIN_NAME = process.platform === 'win32' ? 'lorevm.exe' : 'lorevm';
const binDir = path.join(extRoot, 'bin');

if (!fs.existsSync(binDir)) {
  fs.mkdirSync(binDir, { recursive: true });
}

// 1. If LOREVM_BIN is set, use it (CI override).
if (process.env.LOREVM_BIN && fs.existsSync(process.env.LOREVM_BIN)) {
  const dest = path.join(binDir, BIN_NAME);
  console.log(`Copying override ${process.env.LOREVM_BIN} to ${dest}...`);
  fs.copyFileSync(process.env.LOREVM_BIN, dest);
  process.exit(0);
}

// 2. Otherwise search target/{release,debug} (local dev).
const profiles = ['release', 'debug'];
let copied = false;

for (const profile of profiles) {
  const src = path.join(root, 'target', profile, BIN_NAME);
  if (fs.existsSync(src)) {
    const dest = path.join(binDir, BIN_NAME);
    console.log(`Copying ${src} to ${dest}...`);
    fs.copyFileSync(src, dest);
    copied = true;
    break;
  }
}

if (!copied) {
  console.error(`Error: ${BIN_NAME} not found. Build it first: cargo build -p lorevm-cli`);
  process.exit(1);
}
