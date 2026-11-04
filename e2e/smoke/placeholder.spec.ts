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
    for (const width of [360, 390, 768, 1024, 1440]) {
      await page.setViewportSize({ width, height: 900 });
      await page.goto("./");
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

test("demo page loads with no critical axe violations", async ({ page, baseURL }) => {
  const outside = watchOutsideRequests(page, baseURL ?? "");
  await page.goto("./demo/");
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  expect(await criticalViolations(page)).toEqual([]);
  expect(outside).toEqual([]);
});
