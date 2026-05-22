import * as React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

vi.mock("@/lib/tauri", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const copyToClipboard = vi.fn().mockResolvedValue(true);
vi.mock("@/lib/utils", async () => {
  const actual = await vi.importActual("@/lib/utils");
  return { ...actual, copyToClipboard: (...args) => copyToClipboard(...args) };
});

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
import { MigrationErrorDialog } from "./MigrationErrorDialog";

beforeEach(() => {
  vi.clearAllMocks();
});

const sampleError = {
  failedMigration: "20240101_AddUsers",
  failedDirection: "applying",
  sqlError: "Cannot insert duplicate key.",
  statement: "INSERT INTO Users VALUES (1);",
  fullLog: "full ef log here",
};

describe("MigrationErrorDialog", () => {
  it("renders without crashing with default props", () => {
    const onClose = vi.fn();
    renderWithProviders(<MigrationErrorDialog onClose={onClose} error={sampleError} />);
    expect(screen.getByText("Migration failed")).toBeInTheDocument();
  });

  it("shows the failed migration name, sql error, and statement", () => {
    renderWithProviders(<MigrationErrorDialog onClose={vi.fn()} error={sampleError} />);
    expect(screen.getByText("20240101_AddUsers")).toBeInTheDocument();
    expect(screen.getByText("Cannot insert duplicate key.")).toBeInTheDocument();
    expect(screen.getByText("INSERT INTO Users VALUES (1);")).toBeInTheDocument();
    expect(screen.getByText("Failed while applying")).toBeInTheDocument();
  });

  it("calls onClose when Dismiss is clicked", async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderWithProviders(<MigrationErrorDialog onClose={onClose} error={sampleError} />);
    await user.click(screen.getByRole("button", { name: /dismiss/i }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("copies the full log when the Copy button is clicked", async () => {
    const user = userEvent.setup();
    renderWithProviders(<MigrationErrorDialog onClose={vi.fn()} error={sampleError} />);
    await user.click(screen.getByRole("button", { name: /copy full log/i }));
    expect(copyToClipboard).toHaveBeenCalledTimes(1);
    expect(copyToClipboard.mock.calls[0][0]).toBe("full ef log here");
  });

  it("disables the Copy button when there is no fullLog", () => {
    renderWithProviders(
      <MigrationErrorDialog onClose={vi.fn()} error={{ ...sampleError, fullLog: null }} />
    );
    expect(screen.getByRole("button", { name: /copy full log/i })).toBeDisabled();
  });
});
