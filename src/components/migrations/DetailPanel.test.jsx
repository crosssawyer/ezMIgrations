import * as React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, fireEvent } from "@testing-library/react";

import { renderWithProviders } from "@/test/render";
import { DetailPanel } from "./DetailPanel";

const copyToClipboard = vi.fn().mockResolvedValue(true);
vi.mock("@/lib/utils", async (importOriginal) => {
  const actual = await importOriginal();
  return { ...actual, copyToClipboard: (...args) => copyToClipboard(...args) };
});

const useMigrationSql = vi.fn();
vi.mock("@/lib/queries", () => ({
  useMigrationSql: (...args) => useMigrationSql(...args),
}));

vi.mock("@/lib/ui-store", async (importOriginal) => {
  const actual = await importOriginal();
  return {
    ...actual,
    useUI: () => ({ selectedMigrationId: "m1", setSelectedMigrationId: vi.fn() }),
  };
});

const migrations = [{ id: "m1", name: "20240101_Init" }];

const sql = {
  up_body: "migrationBuilder.CreateTable(...)",
  down_body: "migrationBuilder.DropTable(...)",
  custom_sql_up: ["CREATE VIEW v AS SELECT 1", "UPDATE t SET x = 1"],
  custom_sql_down: ["DROP VIEW v"],
};

beforeEach(() => {
  copyToClipboard.mockClear();
  useMigrationSql.mockReturnValue({ data: sql, isLoading: false });
});

describe("DetailPanel", () => {
  it("shows an All tab by default with all four sections", () => {
    renderWithProviders(<DetailPanel migrations={migrations} />);
    expect(screen.getByRole("tab", { name: "All" })).toHaveAttribute("data-state", "active");
    expect(screen.getByText("Custom SQL Up")).toBeInTheDocument();
    expect(screen.getByText("Custom SQL Down")).toBeInTheDocument();
  });

  it("copies all custom SQL Up statements joined together", () => {
    renderWithProviders(<DetailPanel migrations={migrations} />);
    // The All tab renders both Up and Down "Copy all" buttons; grab the first.
    const copyAll = screen.getAllByRole("button", { name: /copy all/i })[0];
    fireEvent.click(copyAll);
    expect(copyToClipboard).toHaveBeenCalledWith(
      "CREATE VIEW v AS SELECT 1\n\nUPDATE t SET x = 1"
    );
  });

  it("renders a Copy all button only when custom SQL exists", () => {
    useMigrationSql.mockReturnValue({
      data: { ...sql, custom_sql_down: [] },
      isLoading: false,
    });
    renderWithProviders(<DetailPanel migrations={migrations} />);
    // Up has statements, Down does not -> exactly one "Copy all".
    expect(screen.getAllByRole("button", { name: /copy all/i })).toHaveLength(1);
    expect(screen.getByText("No custom SQL in Down()")).toBeInTheDocument();
  });
});
