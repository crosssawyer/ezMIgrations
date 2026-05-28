import { describe, it, expect } from "vitest";
import { mergeWithDbHistory } from "./merge-db-history";

const localMigration = (name, overrides = {}) => ({
  id: name,
  name,
  applied: false,
  has_custom_sql: false,
  file_path: `/repo/${name}.cs`,
  custom_sql_up: [],
  custom_sql_down: [],
  ...overrides,
});

describe("mergeWithDbHistory", () => {
  it("returns local migrations with in_db_state=null when dbRows is null (DB not configured)", () => {
    const local = [localMigration("20250101_A"), localMigration("20250102_B")];
    const out = mergeWithDbHistory(local, null);
    expect(out).toHaveLength(2);
    expect(out.every((m) => m.in_db_state === null)).toBe(true);
  });

  it("marks rows present in dbRows as in_db, others as pending", () => {
    const local = [localMigration("20250101_A"), localMigration("20250102_B")];
    const db = [{ migration_id: "20250101_A", product_version: "8.0.4" }];

    const out = mergeWithDbHistory(local, db);
    const byName = Object.fromEntries(out.map((m) => [m.name, m]));

    expect(byName["20250101_A"].in_db_state).toBe("in_db");
    expect(byName["20250102_B"].in_db_state).toBe("pending");
  });

  it("inserts orphan rows for DB entries with no local file", () => {
    const local = [localMigration("20250101_A")];
    const db = [
      { migration_id: "20250101_A", product_version: "8.0.4" },
      { migration_id: "20250105_OrphanFromMain", product_version: "8.0.4" },
    ];

    const out = mergeWithDbHistory(local, db);
    const orphan = out.find((m) => m.name === "20250105_OrphanFromMain");

    expect(orphan).toBeDefined();
    expect(orphan.in_db_state).toBe("orphan");
    expect(orphan.is_orphan).toBe(true);
    expect(orphan.file_path).toBeNull();
    expect(orphan.product_version).toBe("8.0.4");
  });

  it("sorts the merged list chronologically by migration name", () => {
    const local = [localMigration("20250103_C"), localMigration("20250101_A")];
    const db = [
      { migration_id: "20250102_OrphanB", product_version: "8.0.4" },
      { migration_id: "20250101_A", product_version: "8.0.4" },
    ];

    const out = mergeWithDbHistory(local, db);
    expect(out.map((m) => m.name)).toEqual([
      "20250101_A",
      "20250102_OrphanB",
      "20250103_C",
    ]);
  });

  it("returns an empty array when both inputs are empty", () => {
    expect(mergeWithDbHistory([], [])).toEqual([]);
  });

  it("returns local migrations untouched when dbRows is an empty array (DB has no history)", () => {
    const local = [localMigration("20250101_A")];
    const out = mergeWithDbHistory(local, []);

    expect(out).toHaveLength(1);
    expect(out[0].in_db_state).toBe("pending");
  });
});
