// Parse EF Core / dotnet ef output for structured failure info.
// Returns { failedMigration, sqlError, statement, fullLog } or null if no
// recognizable EF failure pattern is present.

const MAX_STATEMENT_LENGTH = 1200;

const REVERTING_RE = /Reverting migration '([^']+)'\./g;
const APPLYING_RE = /Applying migration '([^']+)'\./g;
const FAIL_BLOCK_RE =
  /fail:\s*Microsoft\.EntityFrameworkCore\.Database\.Command\[\d+\]([\s\S]*?)(?=\n[a-z]+:\s|\nMicrosoft\.[A-Za-z.]+(Exception|SqlException)|$)/;
const SQL_EXCEPTION_RE =
  /(Microsoft\.Data\.SqlClient\.SqlException[\s\S]*?)(?=\n\s+at\s|\nClientConnectionId:|$)/;
const FAILED_DBCMD_RE =
  /Failed executing DbCommand[^\n]*\n([\s\S]*?)(?=\nMicrosoft\.[A-Za-z.]+(Exception|SqlException)|$)/;
const EF_LOG_PREFIX_RE = /^\s*(info|warn|fail|dbug|trce):/i;

function findLastMigrationMention(raw) {
  let last = null;
  for (const [re, direction] of [
    [REVERTING_RE, "reverting"],
    [APPLYING_RE, "applying"],
  ]) {
    re.lastIndex = 0;
    let m;
    while ((m = re.exec(raw)) !== null) last = { name: m[1], direction };
  }
  return last;
}

function cleanLines(text) {
  return text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter((l) => l && !EF_LOG_PREFIX_RE.test(l))
    .join("\n");
}

function truncate(text, max) {
  return text.length > max ? text.slice(0, max) + "…" : text;
}

export function parseEfError(raw) {
  if (!raw || typeof raw !== "string") return null;
  if (!/Microsoft\.EntityFrameworkCore|Microsoft\.Data\.SqlClient/.test(raw)) return null;

  const mention = findLastMigrationMention(raw);
  const failedMigration = mention?.name ?? null;
  const failedDirection = mention?.direction ?? null;

  let sqlError = null;
  const sqlEx = raw.match(SQL_EXCEPTION_RE);
  if (sqlEx) {
    sqlError = cleanLines(
      sqlEx[1].replace(/^Microsoft\.Data\.SqlClient\.SqlException[^:]*:\s*/, "")
    );
  } else {
    const failBlock = raw.match(FAIL_BLOCK_RE);
    if (failBlock) sqlError = cleanLines(failBlock[1]);
  }

  const failedCmd = raw.match(FAILED_DBCMD_RE);
  const statement = failedCmd ? truncate(cleanLines(failedCmd[1]), MAX_STATEMENT_LENGTH) : null;

  if (!failedMigration && !sqlError && !statement) return null;

  return {
    failedMigration,
    failedDirection,
    sqlError,
    statement: statement || null,
    fullLog: raw,
  };
}
