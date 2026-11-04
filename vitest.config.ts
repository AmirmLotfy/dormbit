import { defineConfig } from "vitest/config";

// Minimal for now; foundation Task 12 (4.2) turns this into per-package projects with jsdom.
export default defineConfig({
  test: {
    include: [
      "packages/*/test/**/*.test.ts",
      "packages/*/test/**/*.test.tsx",
      "apps/*/test/**/*.test.ts",
      "apps/*/test/**/*.test.tsx",
    ],
  },
});
