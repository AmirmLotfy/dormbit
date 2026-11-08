// Recording builds only (VITE_AUTOPILOT=1, with the Rust `test-picker` feature): clicks through
// the journey at a reading pace so the real app can be filmed for the demo video with no one
// at the keyboard. The folder and save panels are answered by environment variables on the
// Rust side. Release builds never include this module.

const wait = (ms: number) =>
  new Promise<void>((resolve) => {
    setTimeout(resolve, ms);
  });

function text(el: Element): string {
  return el.textContent.replace(/\s+/g, " ").trim();
}

async function find<T extends Element>(pick: () => T | undefined, what: string): Promise<T> {
  for (let i = 0; i < 300; i++) {
    const el = pick();
    if (el) return el;
    await wait(100);
  }
  throw new Error(`autopilot: ${what} never appeared`);
}

async function show(el: Element, pause = 700) {
  el.scrollIntoView({ behavior: "smooth", block: "center" });
  await wait(pause);
}

async function click(label: string | RegExp, pause = 900) {
  const match = (t: string) => (typeof label === "string" ? t === label : label.test(t));
  const b = await find(
    () => [...document.querySelectorAll("button")].find((x) => !x.disabled && match(text(x))),
    String(label),
  );
  await show(b, 500);
  b.click();
  await wait(pause);
}

async function look(selector: string, contains: string, pause = 2200) {
  const el = await find(
    () => [...document.querySelectorAll(selector)].find((x) => text(x).includes(contains)),
    `${selector} with "${contains}"`,
  );
  await show(el, pause);
}

async function type(selector: string, value: string) {
  const input = await find(
    () => document.querySelector<HTMLInputElement>(selector) ?? undefined,
    selector,
  );
  await show(input, 400);
  for (let i = 0; i <= value.length; i++) {
    // The prototype setter, so React sees the change like a typed one.
    Reflect.set(HTMLInputElement.prototype, "value", value.slice(0, i), input);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await wait(70);
  }
  await wait(500);
}

export async function autopilot(project = "dockline-invoices") {
  await wait(2500);
  await click("Choose a folder…", 2500);
  await look(".card", project, 2500);
  await click(`Open ${project}`);
  await click(/^Next:/);
  await click("Read the project", 1500);
  await look(".finding", "feature/pdf-export");
  await look(".finding", "uncommitted", 1800);
  await look(".finding", "tasks complete", 1800);
  await look(".finding", "renderInvoice.ts", 2400);
  await look(".finding", "unknown", 1800);
  await click(/^Next:/);
  await click("Build the plan", 1500);
  await look(".items li", ".env", 2600);
  await click(/^Next:/);
  const leaveOut = await find(
    () =>
      document.querySelector<HTMLInputElement>('input[type="radio"][value="exclude"]') ?? undefined,
    "Leave it out",
  );
  await show(leaveOut, 600);
  leaveOut.click();
  await wait(1200);
  await click("Approve this plan", 2200);
  await click(/^Next:/);
  await click("Make the capsule", 3500);
  await look("dt", "SHA-256", 2200);
  await click(/^Next:/);
  await click("Verify the capsule", 3000);
  await click(/^Next:/);
  await click("Choose where to restore…", 1800);
  await click("Restore", 2600);
  await type("#dest-name", `${project.replace(/-invoices$/, "")}-restored`);
  await click("Restore", 3500);
  await click(/^Next:/);
  await click("Write the brief", 2000);
  await look(".outcomes", "Files restored and verified", 4000);
  await look(".brief-lines", "First", 2000);
  await click("Save the brief…", 2500);
}
