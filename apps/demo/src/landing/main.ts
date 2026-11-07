// Landing page entry. The text renders first; the gradient starts at once, and the 3D capsule
// (three.js, a separate chunk) starts after the page's own load event and a quiet moment.
// Without WebGL, or when the browser would draw WebGL in software, the SVG drawing stays.
import { startGlow } from "./glow";

const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const root = document.documentElement;
const hero = document.querySelector<HTMLElement>(".hero");
const stage = document.querySelector<HTMLElement>(".hero__stage");

if (hero) startGlow(hero, reduceMotion);

/**
 * True when WebGL exists and is hardware accelerated. Software renderers (SwiftShader, llvmpipe,
 * "Software Rasterizer") make a 3D hero slow and hot, and on a headless CI runner they can stall
 * the main thread, so those browsers keep the SVG capsule instead.
 */
function hasHardwareWebGL(): boolean {
  try {
    const canvas = document.createElement("canvas");
    const gl = canvas.getContext("webgl2") ?? canvas.getContext("webgl");
    if (!gl) return false;
    const info = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer = info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : "";
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return !/swiftshader|llvmpipe|software|softpipe/i.test(renderer);
  } catch {
    return false;
  }
}

function startRecorder(): void {
  if (!hero || !stage) return;
  if (!hasHardwareWebGL()) {
    root.dataset.hero = "fallback";
    return;
  }
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

function whenIdle(): void {
  // Safari has no requestIdleCallback, so fall back to a short timeout.
  const idle = (window as { requestIdleCallback?: Window["requestIdleCallback"] })
    .requestIdleCallback;
  if (idle) idle(startRecorder, { timeout: 1500 });
  else setTimeout(startRecorder, 200);
}

if (document.readyState === "complete") whenIdle();
else window.addEventListener("load", whenIdle, { once: true });
