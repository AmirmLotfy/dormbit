import { ESLint } from "eslint";
import tseslint from "typescript-eslint";
import { describe, expect, it } from "vitest";
import { a11y, hooks, linterOptions, restrictedSyntax } from "../../../eslint.shared.js";

// Lint canary: proves the security-relevant lint rules actually fire.
// If someone weakens eslint.shared.js, these tests fail.

const eslint = new ESLint({
  overrideConfigFile: true,
  overrideConfig: [
    { linterOptions },
    {
      files: ["**/*.tsx"],
      languageOptions: {
        parser: tseslint.parser,
        parserOptions: { ecmaFeatures: { jsx: true } },
      },
    },
    { files: ["**/*.tsx"], ...a11y },
    { files: ["**/*.tsx"], ...hooks },
    { files: ["**/*.tsx"], ...restrictedSyntax },
  ],
});

async function messagesFor(source: string) {
  const [result] = await eslint.lintText(source, { filePath: "canary.tsx" });
  return result?.messages ?? [];
}

describe("lint canary", () => {
  it("bans dangerouslySetInnerHTML (NAT-2)", async () => {
    const msgs = await messagesFor(
      `export const X = ({ h }: { h: string }) => <div dangerouslySetInnerHTML={{ __html: h }} />;`,
    );
    expect(msgs.some((m) => m.ruleId === "no-restricted-syntax")).toBe(true);
  });

  it("flags an <img> with no alt text (jsx-a11y)", async () => {
    const msgs = await messagesFor(`export const X = () => <img src="x.png" />;`);
    expect(msgs.some((m) => m.ruleId === "jsx-a11y/alt-text")).toBe(true);
  });

  it("does not let an inline directive switch rules off", async () => {
    const msgs = await messagesFor(
      [
        `// eslint-disable-next-line no-restricted-syntax`,
        `export const X = ({ h }: { h: string }) => <div dangerouslySetInnerHTML={{ __html: h }} />;`,
      ].join("\n"),
    );
    // The violation is still reported even though a disable comment is present.
    expect(msgs.some((m) => m.ruleId === "no-restricted-syntax")).toBe(true);
  });

  it("accepts clean code", async () => {
    const msgs = await messagesFor(`export const X = () => <img src="x.png" alt="A logo" />;`);
    expect(msgs.filter((m) => m.severity === 2)).toEqual([]);
  });
});
