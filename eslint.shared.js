// @ts-check
// ESLint pieces shared by eslint.config.js and the lint canary test, so the
// canary exercises the exact rules the repo enforces.
import jsxA11y from "eslint-plugin-jsx-a11y";
import reactHooks from "eslint-plugin-react-hooks";

/** Inline directives are ignored and reported, so nobody can switch a rule off in place. */
export const linterOptions = {
  noInlineConfig: true,
  reportUnusedDisableDirectives: "error",
};

/** Raw HTML is banned everywhere in the UI (NAT-2). */
export const restrictedSyntax = {
  rules: {
    "no-restricted-syntax": [
      "error",
      {
        selector: "JSXAttribute[name.name='dangerouslySetInnerHTML']",
        message: "Raw HTML is banned (NAT-2). Use SafeMarkdown or a text node.",
      },
      {
        selector: "Property[key.name='dangerouslySetInnerHTML']",
        message: "Raw HTML is banned (NAT-2). Use SafeMarkdown or a text node.",
      },
    ],
  },
};

export const a11y = jsxA11y.flatConfigs.recommended;

export const hooks = {
  plugins: { "react-hooks": reactHooks },
  rules: {
    "react-hooks/rules-of-hooks": "error",
    "react-hooks/exhaustive-deps": "error",
  },
};
