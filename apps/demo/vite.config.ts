import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

// The site loads only its own files. Injected at build time; dev mode needs inline HMR scripts.
const CSP = [
  "default-src 'self'",
  // 'wasm-unsafe-eval' lets the judge demo compile its own WebAssembly core; scripts still
  // load only from this site.
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self'",
  "img-src 'self' data: blob:",
  "font-src 'self'",
  "connect-src 'self'",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

function contentSecurityPolicy(): Plugin {
  return {
    name: "dormbit-csp",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler: (html) =>
        html.replace(
          "<head>",
          `<head>\n    <meta http-equiv="Content-Security-Policy" content="${CSP}" />`,
        ),
    },
  };
}

// Footer label: "Preview build <short SHA>" when ci sets VITE_BUILD_SHA (judge-demo 20.3).
function buildLabel(): Plugin {
  let label = "Local build";
  return {
    name: "dormbit-build-label",
    configResolved(config) {
      const sha: unknown = config.env.VITE_BUILD_SHA;
      if (typeof sha === "string" && /^[0-9a-f]{7,40}$/.test(sha))
        label = `Preview build ${sha.slice(0, 7)}`;
    },
    transformIndexHtml: (html) => html.replaceAll("%BUILD_LABEL%", label),
  };
}

// base matches the GitHub Pages project path (ADR-009). Two pages: the landing page at
// /dormbit/ and the judge demo at /dormbit/demo/ (hash routes inside it).
export default defineConfig({
  plugins: [react(), contentSecurityPolicy(), buildLabel()],
  base: "/dormbit/",
  build: {
    rolldownOptions: {
      input: { main: "index.html", demo: "demo/index.html" },
    },
  },
});
