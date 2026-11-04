// Landing page entry. The text renders first; the gradient starts at once, and the 3D capsule
// (three.js, a separate chunk) loads when the browser is idle. Without WebGL the SVG stays.
import { startGlow } from "./glow";

const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const root = document.documentElement;
const hero = document.querySelector<HTMLElement>(".hero");
const stage = document.querySelector<HTMLElement>(".hero__stage");

if (hero) startGlow(hero, reduceMotion);

function startRecorder(): void {
  if (!hero || !stage) return;
  import("./recorder")
    .then(({ mountRecorder }) => mountRecorder({ stage, hero, reduceMotion }))
    .then(() => {
      stage.classList.add("is-3d");
      root.dataset.hero = "3d";
    })
    .catch(() => {
      root.dataset.hero = "fallback";
    });
}

// Safari has no requestIdleCallback, so fall back to a short timeout.
const idle = (window as { requestIdleCallback?: Window["requestIdleCallback"] })
  .requestIdleCallback;
if (idle) idle(startRecorder, { timeout: 1500 });
else setTimeout(startRecorder, 200);
