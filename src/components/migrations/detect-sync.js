export function detectOutOfSync(migrations) {
  let firstPendingIdx = -1;
  const foreignMigrations = [];
  for (let i = 0; i < migrations.length; i++) {
    if (!migrations[i].applied && firstPendingIdx === -1) firstPendingIdx = i;
    else if (migrations[i].applied && firstPendingIdx !== -1) foreignMigrations.push(migrations[i]);
  }
  return {
    isOutOfSync: foreignMigrations.length > 0,
    foreignMigrations,
    firstPendingIdx,
  };
}
