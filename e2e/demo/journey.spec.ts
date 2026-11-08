import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { expect, test, type Page } from "@playwright/test";
import { axeClean, next, noSideScroll, start, step, throughBrief } from "./helpers";

// The judge demo, end to end in a real browser with the production CSP: the real core
// compiled to WebAssembly runs every step on the synthetic project.

test("the journey through the brief runs on the real core with no outside requests", async ({
  page,
}) => {
  const outside = await start(page);
  const csp = await page
    .locator('meta[http-equiv="Content-Security-Policy"]')
    .getAttribute("content");
  expect(csp).toContain("script-src 'self' 'wasm-unsafe-eval'");
  await axeClean(page);

  await throughBrief(page);

  await axeClean(page);
  expect(outside).toEqual([]);
});

test("damage to a stored file is caught and named, and Reset starts over", async ({ page }) => {
  await start(page);
  await throughBrief(page);
  await next(page);
  await page.getByRole("button", { name: /Damage a stored file/ }).click();
  await expect(page.getByText(/changed from 0x/)).toBeVisible();
  await next(page);
  await step(page, "Verify the capsule again");
  await expect(page.getByRole("status").filter({ hasText: "Failed" })).toBeVisible();
  await expect(page.getByText("src/pdf/renderInvoice.ts").last()).toBeVisible();
  await axeClean(page);
  await step(page, "Reset");
  await expect(page.getByRole("button", { name: "Grant and scan" })).toBeVisible();
});

async function damage(page: Page, control: RegExp) {
  await throughBrief(page);
  await next(page);
  await page.getByRole("button", { name: control }).click();
  await expect(page.getByText(/changed from 0x/)).toBeVisible();
  await next(page);
}

test("damage outside file data is caught by the verifier", async ({ page }) => {
  await start(page);
  await damage(page, /Damage capsule bytes/);
  await step(page, "Verify the capsule again");
  await expect(page.getByRole("status").filter({ hasText: "Failed" })).toBeVisible();
});

test("damage after verification stops the restore and creates no folder", async ({ page }) => {
  await start(page);
  await damage(page, /Damage after verification/);
  await step(page, "Run the restore");
  await expect(page.getByText("No dockline-after-damage folder was created.")).toBeVisible();
  await expect(page.getByRole("status").filter({ hasText: /^✓/ })).toHaveCount(0);
  await axeClean(page);
});

test("the downloaded capsule hashes to the SHA-256 shown at capture", async ({ page }) => {
  await start(page);
  await throughBrief(page);
  const [dl] = await Promise.all([
    page.waitForEvent("download"),
    step(page, "Download the capsule"),
  ]);
  expect(dl.suggestedFilename()).toMatch(/\.dormbit$/);
  const got = createHash("sha256")
    .update(await readFile(await dl.path()))
    .digest("hex");
  await page.getByRole("button", { name: /^06 Make the capsule/ }).click();
  await expect(page.locator('dt:text-is("SHA-256") + dd')).toHaveText(got);
});

test("the downloaded brief carries the four outcomes", async ({ page }) => {
  await start(page);
  await throughBrief(page);
  const [dl] = await Promise.all([
    page.waitForEvent("download"),
    step(page, "Download the brief (.md)"),
  ]);
  expect(dl.suggestedFilename()).toBe("RESURRECTION-BRIEF.md");
  const md = await readFile(await dl.path(), "utf8");
  for (const label of [
    "Files restored and verified.",
    "Context reconstructed (inferred from evidence).",
    "AI session resumed: not performed.",
    "Builds/runs: unknown (never executed).",
  ])
    expect(md).toContain(label);
});

test("the deployed site serves the commit ci tested", async ({ page }) => {
  const sha = process.env.DEMO_SHA ?? "";
  test.skip(sha === "", "runs only against a deployed build");
  await page.goto("demo/");
  await expect(page.getByText(`Preview build ${sha.slice(0, 7)}`)).toBeVisible();
  await page.goto("./");
  await expect(page.getByText(`Preview build ${sha.slice(0, 7)}`)).toBeVisible();
});

test.describe("at 360 px", () => {
  test.use({ viewport: { width: 360, height: 740 } });

  test("every step fits a narrow phone without sideways scrolling", async ({ page }, info) => {
    await start(page);
    await throughBrief(page);
    await noSideScroll(page);
    await page.screenshot({ path: info.outputPath("brief-360.png"), fullPage: true });
  });
});
