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
import { SwitchBranchDialog } from "./SwitchBranchDialog";

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    if (cmd === "list_git_branches") {
      return Promise.resolve([
        { name: "main", isRemote: false },
        { name: "feature/foo", isRemote: false },
        { name: "origin/release", isRemote: true },
      ]);
    }
    if (cmd === "get_current_branch") return Promise.resolve("main");
    if (cmd === "switch_branch_with_migrations") {
      return Promise.resolve({
        new_branch: "feature/foo",
        rollback_performed: false,
        rollback_target: "",
      });
    }
    return Promise.resolve(undefined);
  });
});

describe("SwitchBranchDialog", () => {
  it("renders title and Cancel/Switch buttons", async () => {
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    expect(screen.getByText("Switch branch")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /cancel/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /switch & update/i })).toBeInTheDocument();
  });

  it("disables the submit button until a branch is selected", async () => {
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    const submit = screen.getByRole("button", { name: /switch & update/i });
    expect(submit).toBeDisabled();
    await waitFor(() => expect(screen.getByText("feature/foo")).toBeInTheDocument());
  });

  it("shows local and remote branches once loaded", async () => {
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    await waitFor(() => {
      // "main" appears in the current-branch chip too, so allow >= 1 match.
      expect(screen.getAllByText("main").length).toBeGreaterThan(0);
      expect(screen.getByText("feature/foo")).toBeInTheDocument();
      expect(screen.getByText("origin/release")).toBeInTheDocument();
    });
    expect(screen.getByText("Local")).toBeInTheDocument();
    expect(screen.getByText("Remote")).toBeInTheDocument();
  });

  it("invokes fetch_remote when the Fetch button is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    const fetchBtn = screen.getByRole("button", { name: /fetch/i });
    await user.click(fetchBtn);
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("fetch_remote");
    });
  });

  it("invokes switch_branch_with_migrations with the selected branch on submit", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    renderWithProviders(<SwitchBranchDialog onClose={onClose} />);
    await waitFor(() => expect(screen.getByText("feature/foo")).toBeInTheDocument());
    await user.click(screen.getByText("feature/foo"));
    const submit = screen.getByRole("button", { name: /switch & update/i });
    await waitFor(() => expect(submit).not.toBeDisabled());
    await user.click(submit);
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("switch_branch_with_migrations", {
        targetBranch: "feature/foo",
      });
    });
  });
});
