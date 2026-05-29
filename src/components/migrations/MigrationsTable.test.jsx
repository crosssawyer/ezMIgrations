import * as React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen, fireEvent, within } from "@testing-library/react";

import { renderWithProviders } from "@/test/render";
import { MigrationsTable } from "./MigrationsTable";

// The table pulls in mutation hooks that talk to Tauri; stub them so the
// component renders without a backend. We exercise navigation, not mutations.
vi.mock("./row-actions", () => ({
  useApplyTo: () => ({ applyTo: vi.fn(), isApplying: false }),
  useRemoveLastOrForce: () => vi.fn(),
}));
vi.mock("@/lib/mutations", () => ({
  useSetStable: () => ({ mutate: vi.fn() }),
}));

const migrations = [
  { id: "m1", name: "20240101_Init", applied: true, has_custom_sql: false },
  { id: "m2", name: "20240202_AddUsers", applied: false, has_custom_sql: false },
  { id: "m3", name: "20240303_AddOrders", applied: false, has_custom_sql: false },
];

function renderTable() {
  return renderWithProviders(
    <MigrationsTable
      migrations={migrations}
      isLoading={false}
      isFetching={false}
      project={{ stable_migration: null }}
      foreignNames={new Set()}
    />
  );
}

function activeRowName() {
  const row = document.querySelector('[role="row"][aria-selected="true"]');
  return row ? within(row).getByRole("button", { name: /\d+_/ }).textContent : null;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("MigrationsTable keyboard navigation", () => {
  it("starts with the first row active", () => {
    renderTable();
    expect(activeRowName()).toBe("20240101_Init");
  });

  it("moves the active cursor with ArrowDown / ArrowUp, clamped at the ends", () => {
    renderTable();
    const grid = screen.getByRole("grid", { name: "Migrations" });

    fireEvent.keyDown(grid, { key: "ArrowDown" });
    expect(activeRowName()).toBe("20240202_AddUsers");

    fireEvent.keyDown(grid, { key: "ArrowDown" });
    expect(activeRowName()).toBe("20240303_AddOrders");

    // Already on the last row — ArrowDown should not wrap past the end.
    fireEvent.keyDown(grid, { key: "ArrowDown" });
    expect(activeRowName()).toBe("20240303_AddOrders");

    fireEvent.keyDown(grid, { key: "ArrowUp" });
    expect(activeRowName()).toBe("20240202_AddUsers");
  });

  it("jumps to the last / first row with End / Home", () => {
    renderTable();
    const grid = screen.getByRole("grid", { name: "Migrations" });

    fireEvent.keyDown(grid, { key: "End" });
    expect(activeRowName()).toBe("20240303_AddOrders");

    fireEvent.keyDown(grid, { key: "Home" });
    expect(activeRowName()).toBe("20240101_Init");
  });

  it("opens the active row's detail on Enter (row becomes selected)", () => {
    renderTable();
    const grid = screen.getByRole("grid", { name: "Migrations" });

    fireEvent.keyDown(grid, { key: "ArrowDown" });
    fireEvent.keyDown(grid, { key: "Enter" });

    const selected = document.querySelector('[role="row"][data-state="selected"]');
    expect(selected).not.toBeNull();
    expect(within(selected).getByText("20240202_AddUsers")).toBeInTheDocument();
  });

  it("toggles the active row's squash checkbox on Space", () => {
    renderTable();
    const grid = screen.getByRole("grid", { name: "Migrations" });

    const checkbox = () =>
      within(
        document.querySelector('[role="row"][aria-selected="true"]')
      ).getByRole("checkbox");

    expect(checkbox()).toHaveAttribute("data-state", "unchecked");
    fireEvent.keyDown(grid, { key: " " });
    expect(checkbox()).toHaveAttribute("data-state", "checked");
    fireEvent.keyDown(grid, { key: " " });
    expect(checkbox()).toHaveAttribute("data-state", "unchecked");
  });
});
