// Vitest global setup: jest-dom matchers + jsdom polyfills the app relies on.
import "@testing-library/jest-dom/vitest";

// jsdom does not implement matchMedia; the theme system calls it (system mode).
// Provide a minimal, override-able stub so ThemeProvider / applyTheme run.
if (typeof window !== "undefined" && !window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}
