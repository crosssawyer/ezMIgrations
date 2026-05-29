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

/**
 * Drag-to-resize state for a panel anchored to the right edge of the window.
 * Returns the current `width` and an `onResizeStart` mousedown handler to wire
 * onto a drag handle on the panel's left edge — dragging left widens it,
 * dragging right narrows it. Width is clamped to [minWidth, maxWidth] and never
 * exceeds the viewport less `viewportGap`, so the rest of the UI stays usable.
 */
export function useHorizontalResize({ initialWidth, minWidth, maxWidth, viewportGap = 320 }) {
  const [width, setWidth] = React.useState(initialWidth);
  const dragRef = React.useRef(null);

  const onResizeStart = React.useCallback(
    (e) => {
      e.preventDefault();
      dragRef.current = { startX: e.clientX, startWidth: width };

      function onMove(ev) {
        const { startX, startWidth } = dragRef.current;
        const cap = Math.min(maxWidth, window.innerWidth - viewportGap);
        const next = startWidth + (startX - ev.clientX);
        setWidth(Math.max(minWidth, Math.min(cap, next)));
      }
      function onUp() {
        document.removeEventListener("mousemove", onMove);
        document.removeEventListener("mouseup", onUp);
        document.body.style.removeProperty("cursor");
        document.body.style.removeProperty("user-select");
      }
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
      document.addEventListener("mousemove", onMove);
      document.addEventListener("mouseup", onUp);
    },
    [width, minWidth, maxWidth, viewportGap]
  );

  return { width, onResizeStart };
}
