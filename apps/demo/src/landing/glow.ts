// Pointer-reactive hero gradient. Three soft light pools drift slowly and lean toward the
// pointer. Only `transform` changes, so the browser composites it without repainting.

const FOLLOW = [0.24, -0.12, 0.09];
const DRIFT = [0.025, 0.035, 0.04];

export function startGlow(hero: HTMLElement, reduceMotion: boolean): void {
  if (reduceMotion) return;
  const blobs = [...hero.querySelectorAll<HTMLElement>(".glow")].map((el, i) => ({
    el,
    follow: FOLLOW[i] ?? 0,
    drift: DRIFT[i] ?? 0.03,
    phase: i * 2.1,
  }));
  if (blobs.length === 0) return;

  let tx = 0.5;
  let ty = 0.5;
  let x = 0.5;
  let y = 0.5;
  let w = hero.clientWidth;
  let h = hero.clientHeight;
  let visible = true;
  let raf = 0;

  const frame = (now: number): void => {
    raf = 0;
    x += (tx - x) * 0.05;
    y += (ty - y) * 0.05;
    const t = now / 1000;
    for (const b of blobs) {
      const dx = (x - 0.5) * w * b.follow + Math.sin(t * 0.21 + b.phase) * w * b.drift;
      const dy = (y - 0.5) * h * b.follow + Math.cos(t * 0.17 + b.phase) * h * b.drift;
      b.el.style.transform = `translate3d(${dx.toFixed(1)}px, ${dy.toFixed(1)}px, 0)`;
    }
    if (visible && !document.hidden) raf = requestAnimationFrame(frame);
  };
  const wake = (): void => {
    if (raf === 0 && visible && !document.hidden) raf = requestAnimationFrame(frame);
  };

  hero.addEventListener(
    "pointermove",
    (e) => {
      const r = hero.getBoundingClientRect();
      tx = (e.clientX - r.left) / r.width;
      ty = (e.clientY - r.top) / r.height;
    },
    { passive: true },
  );
  hero.addEventListener("pointerleave", () => {
    tx = 0.5;
    ty = 0.5;
  });
  new ResizeObserver(() => {
    w = hero.clientWidth;
    h = hero.clientHeight;
  }).observe(hero);
  new IntersectionObserver((entries) => {
    visible = entries.some((e) => e.isIntersecting);
    wake();
  }).observe(hero);
  document.addEventListener("visibilitychange", wake);
  wake();
}
