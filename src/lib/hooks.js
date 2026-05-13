import * as React from "react";

/**
 * Fires `onVisible` whenever the document transitions to visible. Used to
 * refresh state that may have drifted while the app was backgrounded —
 * e.g. the user switched git branches in a terminal.
 *
 * Only listens to `visibilitychange` (not `focus`) to avoid double-firing
 * on macOS, which emits `focus` for every cmd-tab even when the document
 * stayed visible.
 */
export function useRefreshOnVisible(onVisible, { enabled = true } = {}) {
  const ref = React.useRef(onVisible);
  ref.current = onVisible;

  React.useEffect(() => {
    if (!enabled) return undefined;
    function handler() {
      if (document.visibilityState === "visible") ref.current?.();
    }
    document.addEventListener("visibilitychange", handler);
    return () => document.removeEventListener("visibilitychange", handler);
  }, [enabled]);
}
