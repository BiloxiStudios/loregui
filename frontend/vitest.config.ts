import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// Vitest config for the frontend unit/component suite.
//
// New tests live in `*.spec.ts` / `*.spec.tsx` and run here (jsdom +
// @testing-library/react). The two legacy commercial suites
// (`src/commercial/*.test.ts`) keep using Node's built-in test runner via the
// `test:node` script — Vitest deliberately does NOT pick up `*.test.ts` so the
// two runners never collide. `npm test` runs both, in sequence.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    globals: true,
    include: ["src/**/*.spec.{ts,tsx}"],
    setupFiles: ["./src/test/setup.ts"],
    clearMocks: true,
    restoreMocks: true,
  },
});
