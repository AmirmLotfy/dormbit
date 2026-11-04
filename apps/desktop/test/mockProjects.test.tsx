// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { DormbitError } from "@dormbit/schema";
import { MockClient } from "@dormbit/client";
import { MockProjects } from "../src/MockProjects";

afterEach(cleanup);

describe("MockProjects (Requirement 13.8)", () => {
  it("lists every MockClient project", async () => {
    render(<MockProjects client={new MockClient()} />);
    expect(await screen.findByText("dockline-invoices")).toBeTruthy();
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
  });

  it("shows a plain message when the list fails", async () => {
    const client = new MockClient({
      overrides: { projectsList: new DormbitError("SCOPE_DENIED", "no", false) },
    });
    render(<MockProjects client={client} />);
    expect((await screen.findByRole("alert")).textContent).toContain("Could not load");
  });
});
