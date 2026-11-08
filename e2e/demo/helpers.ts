import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page } from "@playwright/test";

// Shared steps for the judge demo E2E tests.

export const BANNER = "Synthetic Demo — No Local Files Accessed";

export async function start(page: Page) {
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

export async function axeClean(page: Page) {
  const axe = await new AxeBuilder({ page }).analyze();
  expect(
    axe.violations.map((v) => ({
      id: v.id,
      nodes: v.nodes.map((n) => `${n.target.join(" ")} :: ${n.failureSummary ?? ""}`),
    })),
  ).toEqual([]);
}

export async function step(page: Page, button: string) {
  // Exact: the step rail has buttons named after the steps too.
  await page.getByRole("button", { name: button, exact: true }).click();
}

// No step may scroll sideways at any viewport, including 360 px phones.
export async function noSideScroll(page: Page) {
  const over = await page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
  expect(over).toBeLessThanOrEqual(0);
}

export async function next(page: Page) {
  await noSideScroll(page);
  await page.getByRole("button", { name: /^Next:/ }).click();
}

export async function throughApproval(page: Page) {
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

export async function throughBrief(page: Page) {
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
