import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

/** Collects every request that leaves the site's own origin (judge-demo Requirement 15). */
function watchOutsideRequests(page: Page, base: string): string[] {
  const origin = new URL(base).origin;
  const outside: string[] = [];
  page.on("request", (r) => {
    const url = r.url();
    if (!url.startsWith(origin) && !url.startsWith("data:") && !url.startsWith("blob:"))
      outside.push(url);
  });
  return outside;
}

async function criticalViolations(page: Page): Promise<string[]> {
  const results = await new AxeBuilder({ page }).analyze();
  return results.violations.filter((v) => v.impact === "critical").map((v) => `${v.id}: ${v.help}`);
}

test.describe("landing page", () => {
  test("loads from its own origin with no critical axe violations", async ({ page, baseURL }) => {
    const outside = watchOutsideRequests(page, baseURL ?? "");
    await page.goto("./");
    await expect(page.getByRole("heading", { level: 1 })).toContainText("Pick up where");
    // The hero settles on the 3D capsule or, without WebGL, on the drawing (judge-demo 22.3, 22.4).
    await expect(page.locator("html")).toHaveAttribute("data-hero", /^(3d|fallback)$/, {
      timeout: 20_000,
    });
    expect(await criticalViolations(page)).toEqual([]);
    expect(outside).toEqual([]);
  });

  test("never scrolls sideways from 360 to 1440 px wide", async ({ page }) => {
    // One page load, then resize: layout is CSS-only, so media queries re-run on resize. Loading
    // the page five times made Firefox on shared CI runners exceed its time limit.
    await page.setViewportSize({ width: 360, height: 900 });
    await page.goto("./");
    for (const width of [360, 390, 768, 1024, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await page.waitForFunction(() => document.readyState === "complete");
      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - window.innerWidth,
      );
      expect(overflow, `horizontal overflow at ${String(width)} px`).toBeLessThanOrEqual(0);
    }
  });

  test("runs no animation under reduced motion", async ({ page }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.goto("./");
    await expect(page.locator("html")).toHaveAttribute("data-hero", /^(3d|fallback)$/, {
      timeout: 20_000,
    });
    const running = await page.evaluate(
      () => document.getAnimations().filter((a) => a.playState === "running").length,
    );
    expect(running).toBe(0);
  });
});

test("the hero only goes 3D on hardware-accelerated WebGL", async ({ page }) => {
  await page.goto("./");
  await expect(page.locator("html")).toHaveAttribute("data-hero", /^(3d|fallback)$/, {
    timeout: 20_000,
  });
  const { hero, software } = await page.evaluate(() => {
    const gl = document.createElement("canvas").getContext("webgl2");
    const ext = gl?.getExtension("WEBGL_debug_renderer_info");
    const renderer = ext && gl ? String(gl.getParameter(ext.UNMASKED_RENDERER_WEBGL)) : "";
    return {
      hero: document.documentElement.dataset.hero,
      software: !gl || /swiftshader|llvmpipe|software|softpipe/i.test(renderer),
    };
  });
  // Software or missing WebGL must keep the SVG capsule; hardware WebGL may use the 3D one.
  if (software) expect(hero).toBe("fallback");
});

test("demo page loads with no critical axe violations", async ({ page, baseURL }) => {
  const outside = watchOutsideRequests(page, baseURL ?? "");
  await page.goto("./demo/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  expect(await criticalViolations(page)).toEqual([]);
  expect(outside).toEqual([]);
});
