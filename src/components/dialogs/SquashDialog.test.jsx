import * as React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const invoke = vi.fn();
vi.mock("@/lib/tauri", () => ({
  invoke: (...args) => invoke(...args),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("sonner", () => {
  const fn = vi.fn();
  fn.success = vi.fn();
  fn.error = vi.fn();
  fn.warning = vi.fn();
  fn.info = vi.fn();
  fn.loading = vi.fn();
  fn.promise = vi.fn();
  fn.dismiss = vi.fn();
  return { toast: fn };
});

import { renderWithProviders } from "@/test/render";
import { SquashDialog } from "./SquashDialog";
import { useUI } from "@/lib/ui-store";

const migrations = [
  { id: "m1", name: "20240101_Init", applied: true },
  { id: "m2", name: "20240102_AddUsers", applied: true },
  { id: "m3", name: "20240103_AddPosts", applied: true },
];

// Seeds ui.checked with the given ids, then renders children.
function SeedChecked({ ids, children }) {
  const ui = useUI();
  const seeded = React.useRef(false);
  if (!seeded.current) {
    seeded.current = true;
    ui.setChecked(new Set(ids));
  }
  return children;
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === "list_migrations") return Promise.resolve(migrations);
    if (cmd === "squash_migrations") return Promise.resolve("Squashed");
    return Promise.resolve(undefined);
  });
});

describe("SquashDialog", () => {
  it("returns null when no migrations are checked", () => {
    const { container } = renderWithProviders(<SquashDialog onClose={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("renders title, range summary, and the name input when migrations are selected", async () => {
    renderWithProviders(
      <SeedChecked ids={["m1", "m2", "m3"]}>
        <SquashDialog onClose={vi.fn()} />
      </SeedChecked>
    );
    await waitFor(() => expect(screen.getByText(/Squash migrations/)).toBeInTheDocument());
    expect(screen.getByLabelText(/new migration name/i)).toBeInTheDocument();
    // The summary renders "<from> → <to>" inside one div, so match on substring.
    expect(screen.getByText(/20240101_Init/)).toBeInTheDocument();
    expect(screen.getByText(/20240103_AddPosts/)).toBeInTheDocument();
  });

  it("submits the squash with from, to, and trimmed new name", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SeedChecked ids={["m1", "m2", "m3"]}>
        <SquashDialog onClose={vi.fn()} />
      </SeedChecked>
    );
    const input = await screen.findByLabelText(/new migration name/i);
    await user.type(input, "  Combined  ");
    await user.click(screen.getByRole("button", { name: /^squash$/i }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("squash_migrations", {
        fromMigration: "20240101_Init",
        toMigration: "20240103_AddPosts",
        newName: "Combined",
      });
    });
  });

  it("does not call invoke when the new name is empty/whitespace", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <SeedChecked ids={["m1", "m2"]}>
        <SquashDialog onClose={vi.fn()} />
      </SeedChecked>
    );
    await screen.findByLabelText(/new migration name/i);
    await user.click(screen.getByRole("button", { name: /^squash$/i }));
    // give any async submission time to fire (it shouldn't)
    await new Promise((r) => setTimeout(r, 50));
    expect(invoke).not.toHaveBeenCalledWith(
      "squash_migrations",
      expect.anything()
    );
  });
});
