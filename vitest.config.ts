import { defineConfig } from "vitest/config";

// `pnpm test` runs every TypeScript test in apps/ and packages/ (foundation Requirement 16.3).
// Logic packages run in Node; UI packages and apps run in jsdom for Testing Library.
export default defineConfig({
  test: {
    projects: [
      {
        test: {
          name: "node",
          environment: "node",
          include: ["packages/schema/test/**/*.test.ts", "packages/client/test/**/*.test.ts"],
        },
      },
      {
        test: {
          name: "dom",
          environment: "jsdom",
          include: ["packages/ui/test/**/*.test.{ts,tsx}", "apps/*/test/**/*.test.{ts,tsx}"],
        },
      },
    ],
  },
});
