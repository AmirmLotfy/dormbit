// The hero's 3D capsule: a flight recorder built from primitives, lit by a generated studio
// environment, with no model files or textures to download. Loaded lazily by main.ts; if WebGL
// is missing, the constructor throws and the SVG drawing stays in place.
import {
  Box3,
  CanvasTexture,
  CylinderGeometry,
  DirectionalLight,
  Group,
  MathUtils,
  Mesh,
  MeshBasicMaterial,
  MeshPhysicalMaterial,
  MeshStandardMaterial,
  NeutralToneMapping,
  PerspectiveCamera,
  PlaneGeometry,
  PMREMGenerator,
  SRGBColorSpace,
  Scene,
  Sphere,
  Vector3,
  WebGLRenderer,
} from "three";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";

export interface RecorderMount {
  /** The element the canvas is added to (it already holds the SVG fallback). */
  stage: HTMLElement;
  /** The hero section; pointer movement anywhere over it tilts the capsule. */
  hero: HTMLElement;
  reduceMotion: boolean;
}

const SIGNAL = "#ff4f00";
const GRAPHITE = "#0f1211";
const BASE_PITCH = 0.1;

/** Builds the scene, renders the first frame, and starts the render loop. */
export async function mountRecorder({ stage, hero, reduceMotion }: RecorderMount): Promise<void> {
  const canvas = document.createElement("canvas");
  canvas.className = "hero__canvas";
  canvas.setAttribute("aria-hidden", "true");

  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
  renderer.setClearColor(0x000000, 0);
  renderer.outputColorSpace = SRGBColorSpace;
  renderer.toneMapping = NeutralToneMapping;
  // Keep the lit orange under the tone curve's highlight knee so it stays #FF4F00, not salmon.
  renderer.toneMappingExposure = 0.8;

  const scene = new Scene();
  const pmrem = new PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.6;
  pmrem.dispose();
  const key = new DirectionalLight("#ffffff", 1.25);
  key.position.set(4, 6, 5);
  const rim = new DirectionalLight("#bcd3cb", 0.8);
  rim.position.set(-5, 2.5, -4);
  scene.add(key, rim);

  await loadLabelFonts();
  const anisotropy = renderer.capabilities.getMaxAnisotropy();
  const model = buildCapsule(anisotropy);

  // Rotate about the capsule's visual center, not the body's.
  const box = new Box3().setFromObject(model);
  model.position.sub(box.getCenter(new Vector3()));
  const radius = box.getBoundingSphere(new Sphere()).radius;
  const pivot = new Group();
  pivot.add(model);
  const unit = new Group();
  unit.add(pivot);
  scene.add(unit);

  const shadowMat = new MeshBasicMaterial({
    map: shadowTexture(),
    transparent: true,
    depthWrite: false,
  });
  const shadow = new Mesh(new PlaneGeometry(1, 1), shadowMat);
  shadow.rotation.x = -Math.PI / 2;
  shadow.position.y = -(box.max.y - box.min.y) / 2 - 0.3;
  scene.add(shadow);

  const camera = new PerspectiveCamera(28, 1, 0.1, 100);
  const fit = (): void => {
    const w = Math.max(canvas.clientWidth, 1);
    const h = Math.max(canvas.clientHeight, 1);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    const v = MathUtils.degToRad(camera.fov) / 2;
    const half = Math.min(v, Math.atan(Math.tan(v) * camera.aspect));
    const dist = (radius * 1.06) / Math.sin(half);
    camera.position.set(0, dist * 0.17, dist);
    camera.lookAt(0, -0.08, 0);
    camera.updateProjectionMatrix();
  };

  // ---- interaction state ----------------------------------------------------------------
  const s = {
    yaw: -0.34,
    pitch: BASE_PITCH,
    spin: 0,
    tiltX: 0,
    tiltY: 0,
    aimX: 0,
    aimY: 0,
    scroll: 0,
  };
  let dragging = false;
  let lastX = 0;
  let lastY = 0;
  let lastT = 0;
  let velocity = 0;

  const step = (dt: number, t: number): boolean => {
    const k = 1 - Math.exp(-dt * 5);
    s.tiltX += (s.aimX - s.tiltX) * k;
    s.tiltY += (s.aimY - s.tiltY) * k;
    if (!dragging && !reduceMotion) {
      s.yaw += s.spin * dt;
      s.spin *= Math.exp(-dt * 1.6);
      s.pitch += (BASE_PITCH - s.pitch) * (1 - Math.exp(-dt * 2.2));
    }
    // A slow sway instead of a turntable, so the printed label stays readable.
    const sway = reduceMotion ? 0 : Math.sin(t * 0.32) * 0.38;
    pivot.rotation.set(s.pitch, s.yaw + sway, 0);
    unit.rotation.set(s.tiltX + s.scroll * 0.45, s.tiltY, 0);
    const bob = reduceMotion ? 0 : Math.sin(t * 1.15) * 0.045;
    unit.position.y = bob + s.scroll * 0.3;
    shadowMat.opacity = MathUtils.clamp(0.9 - bob * 3 - s.scroll * 0.6, 0, 1);
    shadow.scale.set(radius * 2.5 * (1 - bob), radius * 1.2 * (1 - bob), 1);
    const settling = Math.abs(s.aimX - s.tiltX) + Math.abs(s.aimY - s.tiltY) > 0.001;
    return !reduceMotion || dragging || settling;
  };

  // ---- render loop: runs only while the hero is on screen and the tab is visible ----------
  let visible = true;
  let running = false;
  let last = 0;
  const frame = (now: number): void => {
    const dt = Math.min((now - last) / 1000, 0.05);
    last = now;
    const more = step(dt, now / 1000);
    renderer.render(scene, camera);
    if (more && visible && !document.hidden) requestAnimationFrame(frame);
    else running = false;
  };
  const wake = (): void => {
    if (running || !visible || document.hidden) return;
    running = true;
    last = performance.now();
    requestAnimationFrame(frame);
  };

  hero.addEventListener(
    "pointermove",
    (e) => {
      if (e.pointerType !== "mouse" || dragging) return;
      const r = hero.getBoundingClientRect();
      s.aimY = ((e.clientX - r.left) / r.width - 0.5) * 0.5;
      s.aimX = ((e.clientY - r.top) / r.height - 0.5) * 0.3;
      wake();
    },
    { passive: true },
  );
  hero.addEventListener("pointerleave", () => {
    s.aimX = 0;
    s.aimY = 0;
    wake();
  });
  canvas.addEventListener("pointerdown", (e) => {
    dragging = true;
    canvas.setPointerCapture(e.pointerId);
    lastX = e.clientX;
    lastY = e.clientY;
    lastT = e.timeStamp;
    velocity = 0;
    s.spin = 0;
    wake();
  });
  canvas.addEventListener("pointermove", (e) => {
    if (!dragging) return;
    const dx = e.clientX - lastX;
    const dy = e.clientY - lastY;
    const dt = Math.max((e.timeStamp - lastT) / 1000, 1 / 240);
    s.yaw += dx * 0.011;
    s.pitch = MathUtils.clamp(s.pitch + dy * 0.006, -0.55, 0.65);
    velocity = (dx * 0.011) / dt;
    lastX = e.clientX;
    lastY = e.clientY;
    lastT = e.timeStamp;
    wake();
  });
  const release = (e: PointerEvent): void => {
    if (!dragging) return;
    dragging = false;
    const stale = e.timeStamp - lastT > 90;
    s.spin = reduceMotion || stale ? 0 : MathUtils.clamp(velocity, -7, 7);
    wake();
  };
  canvas.addEventListener("pointerup", release);
  canvas.addEventListener("pointercancel", release);
  if (!reduceMotion) {
    window.addEventListener(
      "scroll",
      () => {
        s.scroll = MathUtils.clamp(window.scrollY / Math.max(hero.offsetHeight, 1), 0, 1);
        wake();
      },
      { passive: true },
    );
  }
  canvas.addEventListener("webglcontextlost", (e) => {
    e.preventDefault();
    visible = false;
    stage.classList.remove("is-3d");
  });

  stage.append(canvas);
  fit();
  step(0, 0);
  renderer.render(scene, camera);

  new ResizeObserver(() => {
    fit();
    if (!running) renderer.render(scene, camera);
  }).observe(canvas);
  new IntersectionObserver((entries) => {
    visible = entries.some((e) => e.isIntersecting);
    wake();
  }).observe(stage);
  document.addEventListener("visibilitychange", wake);
  wake();
}

/** Body, reflective tape, beacon can, label, and screws, in model units (1 ≈ 10 cm). */
function buildCapsule(anisotropy: number): Group {
  const W = 2.7;
  const H = 1.62;
  const D = 1.5;
  const paint = new MeshPhysicalMaterial({
    color: SIGNAL,
    roughness: 0.42,
    clearcoat: 0.45,
    clearcoatRoughness: 0.3,
  });
  const tape = new MeshStandardMaterial({ color: "#eef0ec", roughness: 0.3, metalness: 0.2 });
  const screwMat = new MeshStandardMaterial({ color: "#3a1500", roughness: 0.45, metalness: 0.6 });

  const g = new Group();
  g.add(new Mesh(new RoundedBoxGeometry(W, H, D, 6, 0.1), paint));
  for (const y of [H / 2 - 0.2, -H / 2 + 0.2]) {
    const band = new Mesh(new RoundedBoxGeometry(W + 0.014, 0.15, D + 0.014, 4, 0.06), tape);
    band.position.y = y;
    g.add(band);
  }

  const can = new Mesh(new CylinderGeometry(0.44, 0.44, 0.6, 64), paint);
  can.position.set(0, H / 2 + 0.28, -0.08);
  const ring = new Mesh(new CylinderGeometry(0.447, 0.447, 0.12, 64, 1, true), tape);
  ring.position.set(0, H / 2 + 0.36, -0.08);
  const cap = new Mesh(new CylinderGeometry(0.38, 0.44, 0.07, 64), paint);
  cap.position.set(0, H / 2 + 0.615, -0.08);
  g.add(can, ring, cap);

  const LW = W - 0.42;
  const LH = 0.84;
  const labelMat = new MeshPhysicalMaterial({
    map: labelTexture(LW / LH, anisotropy),
    roughness: 0.42,
    clearcoat: 0.45,
    clearcoatRoughness: 0.3,
  });
  const label = new Mesh(new PlaneGeometry(LW, LH), labelMat);
  label.position.set(0, 0, D / 2 + 0.004);
  g.add(label);

  const screw = new CylinderGeometry(0.045, 0.045, 0.03, 24);
  for (const sx of [-1, 1]) {
    for (const sy of [-1, 1]) {
      const m = new Mesh(screw, screwMat);
      m.rotation.x = Math.PI / 2;
      m.position.set(sx * (W / 2 - 0.15), sy * 0.48, D / 2 + 0.012);
      g.add(m);
    }
  }
  return g;
}

async function loadLabelFonts(): Promise<void> {
  try {
    await Promise.all([
      document.fonts.load('900 120px "Archivo"'),
      document.fonts.load('600 48px "Atkinson Mono"'),
    ]);
  } catch {
    // The label falls back to system fonts; the capsule still renders.
  }
}

/** The printed front plate: orange like the paint, so it reads as part of the casing. */
function labelTexture(aspect: number, anisotropy: number): CanvasTexture {
  const cw = 2048;
  const ch = Math.round(cw / aspect);
  const c = document.createElement("canvas");
  c.width = cw;
  c.height = ch;
  const g = c.getContext("2d");
  if (!g) throw new Error("2D canvas unavailable");
  const pad = 64;
  g.fillStyle = SIGNAL;
  g.fillRect(0, 0, cw, ch);
  g.fillStyle = GRAPHITE;
  g.textBaseline = "alphabetic";

  const title = "DORMBIT CAPSULE";
  g.fontStretch = "ultra-expanded";
  g.font = '900 160px "Archivo", sans-serif';
  const size = Math.floor((160 * (cw - pad * 2)) / g.measureText(title).width);
  g.font = `900 ${String(size)}px "Archivo", sans-serif`;
  g.fillText(title, pad, ch * 0.3);

  g.fontStretch = "normal";
  g.letterSpacing = "8px";
  g.font = '700 44px "Atkinson Mono", monospace';
  g.fillText("RECOVER WITH DORMBIT · DO NOT DISCARD", pad + 4, ch * 0.44);
  g.fillRect(pad, ch * 0.5, cw - pad * 2, 6);

  g.letterSpacing = "2px";
  g.font = '600 62px "Atkinson Mono", monospace';
  g.fillText("dockline-invoices.dormbit", pad + 4, ch * 0.68);
  g.font = '500 58px "Atkinson Mono", monospace';
  g.fillText("sha256  e0a0 323b … c6e3 74", pad + 4, ch * 0.83);

  const tex = new CanvasTexture(c);
  tex.colorSpace = SRGBColorSpace;
  tex.anisotropy = anisotropy;
  return tex;
}

/** A soft contact shadow, drawn once on a small canvas. */
function shadowTexture(): CanvasTexture {
  const c = document.createElement("canvas");
  c.width = 256;
  c.height = 256;
  const g = c.getContext("2d");
  if (!g) throw new Error("2D canvas unavailable");
  const grad = g.createRadialGradient(128, 128, 0, 128, 128, 128);
  grad.addColorStop(0, "rgba(0,0,0,0.62)");
  grad.addColorStop(0.45, "rgba(0,0,0,0.3)");
  grad.addColorStop(1, "rgba(0,0,0,0)");
  g.fillStyle = grad;
  g.fillRect(0, 0, 256, 256);
  return new CanvasTexture(c);
}
