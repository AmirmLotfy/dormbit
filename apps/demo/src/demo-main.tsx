import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./demo/demo.css";
import { Shell } from "./demo/Shell";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root");
createRoot(root).render(
  <StrictMode>
    <Shell />
  </StrictMode>,
);
