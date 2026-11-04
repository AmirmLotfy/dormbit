// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { DormbitError } from "@dormbit/schema";
import { App } from "../src/App";
import { loadAppInfo, type AppInfoHost } from "../src/appInfo";

afterEach(cleanup);

const info = {
  name: "DORMBIT",
  version: "0.1.0",
  core_version: "0.1.0",
  profile: "debug",
  min_macos: "13.0",
};

/** A host that sends the given messages, then returns `ret` or throws `thrown`. */
function host(messages: unknown[], ret: unknown, thrown?: object): AppInfoHost {
  return {
    call(onMessage) {
      for (const m of messages) onMessage(m);
      return thrown === undefined
        ? Promise.resolve(ret)
        : Promise.reject(Object.assign(new Error("ipc"), thrown));
    },
  };
}

describe("loadAppInfo", () => {
  it("streams info then done and returns the info", async () => {
    const seen: string[] = [];
    const out = await loadAppInfo(
      (e) => seen.push(e.kind),
      host([{ kind: "info", info }, { kind: "done" }], info),
    );
    expect(seen).toEqual(["info", "done"]);
    expect(out.version).toBe("0.1.0");
  });

  it("maps an unknown event to IPC_SCHEMA_MISMATCH", async () => {
    await expect(
      loadAppInfo(() => undefined, host([{ kind: "surprise" }], info)),
    ).rejects.toMatchObject({
      code: "IPC_SCHEMA_MISMATCH",
      retryable: false,
    });
  });

  it("maps a bad return value to IPC_SCHEMA_MISMATCH", async () => {
    await expect(loadAppInfo(() => undefined, host([], { name: "DORMBIT" }))).rejects.toMatchObject(
      {
        code: "IPC_SCHEMA_MISMATCH",
      },
    );
  });

  it("passes a valid Rust error through", async () => {
    const wire = { code: "CANCELLED", message: "window closed", retryable: false };
    const err = await loadAppInfo(() => undefined, host([], null, wire)).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(DormbitError);
    expect(err).toMatchObject({ code: "CANCELLED" });
  });

  it("maps an error with an unknown code to IPC_SCHEMA_MISMATCH", async () => {
    const wire = { code: "NEW_CODE", message: "x", retryable: true };
    await expect(loadAppInfo(() => undefined, host([], null, wire))).rejects.toMatchObject({
      code: "IPC_SCHEMA_MISMATCH",
      retryable: false,
    });
  });
});

describe("App", () => {
  it("shows the name and version from Rust", async () => {
    render(<App host={host([{ kind: "info", info }, { kind: "done" }], info)} />);
    expect(await screen.findByRole("heading", { name: "DORMBIT 0.1.0" })).toBeTruthy();
  });

  it("shows a plain message when the call fails", async () => {
    render(<App host={host([], { bad: true })} />);
    expect((await screen.findByRole("alert")).textContent).toContain(
      "could not read its own version",
    );
  });
});
