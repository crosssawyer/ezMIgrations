// Tracks the highest app version the user has seen the "what's new" hint for.
// Lives in localStorage — this is incidental UI state, not a user preference.
// If the webview's storage is cleared, the banner re-appears; that's fine.

const KEY = "ez-migrations:last-seen-version";

export function getSeenVersion() {
  try {
    return localStorage.getItem(KEY) ?? "0.0.0";
  } catch {
    return "0.0.0";
  }
}

export function markSeen(version) {
  try {
    localStorage.setItem(KEY, version);
  } catch {
    /* private mode / quota — silently ignore */
  }
}

/** Loose semver compare: returns negative if a < b, 0 if equal, positive if a > b. */
export function compareVersions(a, b) {
  const parse = (v) =>
    v
      .split(/[.-]/)
      .map((p) => Number.parseInt(p, 10))
      .map((n) => (Number.isFinite(n) ? n : 0));
  const aa = parse(a);
  const bb = parse(b);
  const len = Math.max(aa.length, bb.length);
  for (let i = 0; i < len; i++) {
    const diff = (aa[i] ?? 0) - (bb[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}
