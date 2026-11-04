import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// base matches the GitHub Pages project path (ADR-009).
export default defineConfig({
  plugins: [react()],
  base: "/dormbit/",
});
