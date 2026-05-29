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

  it("fetches, then refetches branches without refetching the current branch", async () => {
    const user = userEvent.setup();
    const callsTo = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd).length;

    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    // Wait for the initial branch list load to settle before counting.
    await waitFor(() => expect(callsTo("list_git_branches")).toBeGreaterThan(0));
    const branchListCallsBefore = callsTo("list_git_branches");
    const currentBranchCallsBefore = callsTo("get_current_branch");

    await user.click(screen.getByRole("button", { name: /fetch/i }));

    // The mutation runs fetch_remote …
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("fetch_remote"));
    // … and on success invalidates the branch list so it refetches.
    await waitFor(() =>
      expect(callsTo("list_git_branches")).toBeGreaterThan(branchListCallsBefore)
    );
    // Fetch never moves HEAD, so the current branch must NOT be refetched.
    expect(callsTo("get_current_branch")).toBe(currentBranchCallsBefore);
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

  it("focuses the branch filter on open so the list is arrow-navigable", async () => {
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    // Placeholder flips from loading→search, so match either; the input must
    // be focusable even while branches are still loading.
    const input = screen.getByPlaceholderText(/loading branches|search branches/i);
    await waitFor(() => expect(input).toHaveFocus());
  });

  it("switches to the highlighted branch when Enter is pressed in the list", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("feature/foo")).toBeInTheDocument());
    const input = screen.getByPlaceholderText("Search branches...");
    await user.click(input);
    await user.type(input, "feature");
    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("switch_branch_with_migrations", {
        targetBranch: "feature/foo",
      });
    });
  });

  it("ignores mouse hover so it doesn't hijack the chosen branch", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("feature/foo")).toBeInTheDocument());
    // Deliberately pick one branch, then sweep the mouse over another.
    await user.click(screen.getByText("feature/foo"));
    await user.hover(screen.getByText("origin/release"));
    const submit = screen.getByRole("button", { name: /switch & update/i });
    await waitFor(() => expect(submit).not.toBeDisabled());
    await user.click(submit);
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("switch_branch_with_migrations", {
        targetBranch: "feature/foo",
      });
    });
  });

  it("moves the switch target as the cursor moves through the list", async () => {
    const user = userEvent.setup();
    renderWithProviders(<SwitchBranchDialog onClose={vi.fn()} />);
    await waitFor(() => expect(screen.getByText("feature/foo")).toBeInTheDocument());
    const input = screen.getByPlaceholderText("Search branches...");
    await user.click(input);
    // First item (main) auto-highlights; arrowing down moves the target.
    await user.keyboard("{ArrowDown}");
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
