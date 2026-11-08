import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// The judge demo, end to end in a real browser with the production CSP: the real core
// compiled to WebAssembly runs every step on the synthetic project.

const BANNER = "Synthetic Demo — No Local Files Accessed";

async function start(page: Page) {
  // The site's own origin: the local preview, or the deployed site when DEMO_URL is set.
  const origin = new URL(test.info().project.use.baseURL ?? "http://127.0.0.1/").origin;
  const outside: string[] = [];
  page.on("request", (r) => {
    const url = r.url();
    if (!url.startsWith(origin) && !url.startsWith("data:") && !url.startsWith("blob:"))
      outside.push(url);
  });
  await page.goto("demo/");
  await expect(page.getByText(BANNER)).toBeVisible();
  await expect(page.getByRole("button", { name: "Grant and scan" })).toBeVisible();
  return outside;
}

async function axeClean(page: Page) {
  const axe = await new AxeBuilder({ page }).analyze();
  expect(
    axe.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => `${n.target.join(" ")} :: ${n.failureSummary ?? ""}`),
    })),
  ).toEqual([]);
}

async function step(page: Page, button: string) {
  // Exact: the step rail has buttons named after the steps too.
  await page.getByRole("button", { name: button, exact: true }).click();
}

// No step may scroll sideways at any viewport, including 360 px phones.
async function noSideScroll(page: Page) {
  const over = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(over).toBeLessThanOrEqual(0);
}

async function next(page: Page) {
  await noSideScroll(page);
  await page.getByRole("button", { name: /^Next:/ }).click();
}

async function throughApproval(page: Page) {
  await page.getByRole("radio", { name: "~/Projects" }).check();
  await step(page, "Grant and scan");
  await expect(page.getByRole("heading", { name: "dockline-invoices" })).toBeVisible();
  await next(page);
  await step(page, "Read the project");
  await expect(page.getByText("feature/pdf-export").first()).toBeVisible();
  await next(page);
  await expect(page.getByText("3 of 7 tasks complete", { exact: false })).toBeVisible();
  await expect(page.getByText("src/pdf/renderInvoice.ts:14")).toBeVisible();
  await step(page, "Continue to the plan");
  await next(page);
  await step(page, "Build the plan");
  await expect(page.getByText("Sensitive: environment file, never captured")).toBeVisible();
  await next(page);
  await expect(page.getByRole("button", { name: "Approve this plan" })).toBeDisabled();
  await page.getByRole("radio", { name: "Leave it out" }).check();
  await step(page, "Approve this plan");
  await expect(page.getByText("Plan digest", { exact: false })).toBeVisible();
  await next(page);
  await step(page, "Make the capsule");
  await expect(page.getByText("recording", { exact: true })).toBeVisible();
  await next(page);
  await step(page, "Verify the capsule");
  await expect(page.getByRole("status").filter({ hasText: "Verified" })).toBeVisible();
}

async function throughBrief(page: Page) {
  await throughApproval(page);
  await next(page);
  // Step 8: the original name exists in ~/Restores, so the first try is refused.
  await step(page, "Restore");
  await expect(page.getByText("RESTORE_DEST_EXISTS")).toBeVisible();
  await page.getByLabel("New folder name").fill("dockline-restored");
  await step(page, "Restore");
  await expect(page.getByText("~/Restores/dockline-restored")).toBeVisible();
  await next(page);
  await step(page, "Re-hash every restored file");
  await expect(page.getByRole("status")).toContainText(/\b182 of 182 files match the manifest\.$/);
  await expect(page.getByText("Not run in the browser.", { exact: false })).toBeVisible();
  await next(page);
  await step(page, "Write the brief");
  await expect(
    page.getByText("Context reconstructed (inferred from evidence).", { exact: false }),
  ).toBeVisible();
  await expect(page.getByText("AI session resumed: not performed.")).toBeVisible();
  await expect(page.getByText("Builds/runs: unknown (never executed).")).toBeVisible();
  await expect(page.getByText(/Files restored and verified\. 182 files/)).toBeVisible();
}

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
