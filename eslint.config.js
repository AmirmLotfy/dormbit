// @ts-check
import tseslint from "typescript-eslint";
import { a11y, hooks, linterOptions, restrictedSyntax } from "./eslint.shared.js";

export default tseslint.config(
  { linterOptions },
  {
    ignores: [
      "**/dist/**",
      "**/target/**",
      "**/node_modules/**",
      "packages/schema/generated/**",
      "fixtures/**",
      ".scratch/**",
    ],
  },
  ...tseslint.configs.strictTypeChecked.map((c) => ({
    ...c,
    files: ["**/*.ts", "**/*.tsx"],
  })),
  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  { files: ["**/*.tsx"], ...a11y },
  { files: ["**/*.tsx"], ...hooks },
  { files: ["**/*.ts", "**/*.tsx"], ...restrictedSyntax },
);
