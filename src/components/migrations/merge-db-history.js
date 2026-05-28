/**
 * Merge local migrations with rows from __EFMigrationsHistory.
 *
 * When db rows are available, the badge state on each row reflects the DB
 * truth — not EF's `applied` field, since the whole point of this view is to
 * verify the EF assertion against the canonical history table.
 *
 * Returns a single chronologically-sorted array. Orphan rows (in DB, no local
 * file) are inserted in place by their migration_id timestamp.
 *
 *   in_db_state values:
 *     "in_db"   — local file exists, row exists in __EFMigrationsHistory
 *     "pending" — local file exists, no DB row
 *     "orphan"  — DB row exists, no local file (drift — shouldn't happen)
 *     null      — DB history not available; UI should fall back to `applied`
 */
export function mergeWithDbHistory(local, dbRows) {
  if (!dbRows) {
    return local.map((m) => ({ ...m, in_db_state: null }));
  }

  const dbSet = new Set(dbRows.map((r) => r.migration_id));
  const localNames = new Set(local.map((m) => m.name));

  const merged = local.map((m) => ({
    ...m,
    in_db_state: dbSet.has(m.name) ? "in_db" : "pending",
  }));

  for (const r of dbRows) {
    if (localNames.has(r.migration_id)) continue;
    merged.push({
      id: r.migration_id,
      name: r.migration_id,
      applied: true,
      has_custom_sql: false,
      file_path: null,
      custom_sql_up: [],
      custom_sql_down: [],
      in_db_state: "orphan",
      is_orphan: true,
      product_version: r.product_version,
    });
  }

  merged.sort((a, b) => a.name.localeCompare(b.name));
  return merged;
}
