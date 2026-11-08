import { expect, test } from "@playwright/test";
import { start, step, throughApproval } from "./helpers";

// Privacy and motion checks for the judge demo (judge-demo Requirements 15.2-15.5, 17.5, 18.4).

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const seen: string[] = [];
    Object.defineProperty(window, "__cspViolations", { value: seen });
    document.addEventListener("securitypolicyviolation", (e) => {
      seen.push(`${e.violatedDirective} ${e.blockedURI}`);
    });
  });
});

async function cspViolations(page: import("@playwright/test").Page): Promise<string[]> {
  return page.evaluate(() => [
    ...((window as unknown as { __cspViolations?: string[] }).__cspViolations ?? []),
  ]);
}

test("the demo stores nothing in the browser and breaks no CSP rule", async ({ page }) => {
  await start(page);
  await throughApproval(page);
  const stored = await page.evaluate(async () => ({
    cookies: document.cookie,
    local: localStorage.length,
    session: sessionStorage.length,
    indexedDb: typeof indexedDB.databases === "function" ? (await indexedDB.databases()).length : 0,
  }));
  expect(stored).toEqual({ cookies: "", local: 0, session: 0, indexedDb: 0 });
  expect(await cspViolations(page)).toEqual([]);
});

test("a reload starts again at step 1", async ({ page }) => {
  await start(page);
  await throughApproval(page);
  await page.reload();
  await expect(page.getByRole("heading", { level: 1 })).toContainText("Find projects");
  await expect(page.getByRole("button", { name: "Grant and scan" })).toBeVisible();
});

test("under reduced motion nothing animates and progress appears at once", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await start(page);
  await throughApproval(page);
  expect(await page.evaluate(() => document.getAnimations().length)).toBe(0);
  await page.getByRole("button", { name: /^06 Make the capsule/ }).click();
  await expect(
    page.getByText("All", { exact: false }).filter({ hasText: "progress events shown" }),
  ).toBeAttached();
  await page.getByRole("button", { name: /^07 Verify the capsule/ }).click();
  await step(page, "Verify the capsule");
  expect(await page.evaluate(() => document.getAnimations().length)).toBe(0);
  expect(await cspViolations(page)).toEqual([]);
});
