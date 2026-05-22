import { describe, it, expect } from "vitest";
import { detectOutOfSync } from "./detect-sync";

const m = (id, applied) => ({ id, applied, name: `M_${id}` });

describe("detectOutOfSync", () => {
  it("returns clean state for an empty list", () => {
    const result = detectOutOfSync([]);
    expect(result.isOutOfSync).toBe(false);
    expect(result.foreignMigrations).toEqual([]);
    expect(result.firstPendingIdx).toBe(-1);
  });

  it("returns clean state when all migrations are applied", () => {
    const list = [m(1, true), m(2, true), m(3, true)];
    const result = detectOutOfSync(list);
    expect(result.isOutOfSync).toBe(false);
    expect(result.foreignMigrations).toEqual([]);
    expect(result.firstPendingIdx).toBe(-1);
  });

  it("returns the first pending index when all are pending", () => {
    const list = [m(1, false), m(2, false), m(3, false)];
    const result = detectOutOfSync(list);
    expect(result.isOutOfSync).toBe(false);
    expect(result.foreignMigrations).toEqual([]);
    expect(result.firstPendingIdx).toBe(0);
  });

  it("returns clean state for applied-then-pending in expected order", () => {
    const list = [m(1, true), m(2, true), m(3, false), m(4, false)];
    const result = detectOutOfSync(list);
    expect(result.isOutOfSync).toBe(false);
    expect(result.foreignMigrations).toEqual([]);
    expect(result.firstPendingIdx).toBe(2);
  });

  it("detects a single applied-after-pending foreign migration", () => {
    const list = [m(1, true), m(2, false), m(3, true), m(4, false)];
    const result = detectOutOfSync(list);
    expect(result.isOutOfSync).toBe(true);
    expect(result.foreignMigrations).toHaveLength(1);
    expect(result.foreignMigrations[0].id).toBe(3);
    expect(result.firstPendingIdx).toBe(1);
  });

  it("detects multiple foreign migrations after the first pending", () => {
    const list = [
      m(1, true),
      m(2, false),
      m(3, true),
      m(4, true),
      m(5, false),
    ];
    const result = detectOutOfSync(list);
    expect(result.isOutOfSync).toBe(true);
    expect(result.foreignMigrations.map((f) => f.id)).toEqual([3, 4]);
    expect(result.firstPendingIdx).toBe(1);
  });

  it("treats the leading-pending case correctly when foreign follows", () => {
    const list = [m(1, false), m(2, true)];
    const result = detectOutOfSync(list);
    expect(result.isOutOfSync).toBe(true);
    expect(result.foreignMigrations).toHaveLength(1);
    expect(result.foreignMigrations[0].id).toBe(2);
    expect(result.firstPendingIdx).toBe(0);
  });
});
