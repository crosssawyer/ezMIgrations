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

/**
 * Roving-cursor keyboard navigation for a vertical list/grid of rows keyed by
 * id. `activeId` is a focus cursor independent of any selection the caller
 * tracks separately: arrows move it, Home/End jump to the ends, Enter calls
 * `onSelect(id)`, Space calls `onToggle(id)`. When `selectedId` is non-null the
 * cursor "follows" into selection on every move (master-detail behavior).
 *
 * Wire it up by spreading `gridProps` onto the scroll container and giving each
 * row `ref={registerRow(id)}`; the hook scrolls the cursor into view and
 * auto-focuses the container once rows first appear (without stealing focus
 * from an input the user is typing in).
 *
 *   ids       - ordered ids of the currently visible rows
 *   selectedId- the caller's selected id, or null when nothing is open
 *   onSelect  - open/select the row at id (Enter, and follow-on-move)
 *   onToggle  - toggle the row at id (Space)
 */
export function useGridKeyboardNav({ ids, selectedId, onSelect, onToggle }) {
  const idsKey = ids.join("|");
  const gridRef = React.useRef(null);
  const rowRefs = React.useRef(new Map());
  const didAutoFocus = React.useRef(false);
  const [activeId, setActiveId] = React.useState(null);

  // Keep the cursor pointing at a visible row as filtering/sorting changes.
  React.useEffect(() => {
    if (ids.length === 0) {
      setActiveId(null);
      return;
    }
    setActiveId((cur) => {
      if (cur && ids.includes(cur)) return cur;
      if (selectedId && ids.includes(selectedId)) return selectedId;
      return ids[0];
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idsKey, selectedId]);

  // Focus the grid once it first has rows so arrows work without a click —
  // but never steal focus from an input the user is already typing in.
  React.useEffect(() => {
    if (didAutoFocus.current || ids.length === 0 || !gridRef.current) return;
    const active = document.activeElement;
    const inInput = active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA");
    if (!inInput && (!active || active === document.body)) {
      gridRef.current.focus({ preventScroll: true });
    }
    didAutoFocus.current = true;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [idsKey]);

  function moveTo(nextId) {
    if (nextId == null) return;
    setActiveId(nextId);
    rowRefs.current.get(nextId)?.scrollIntoView({ block: "nearest" });
    // When something is open, let it follow the cursor (master-detail).
    if (selectedId != null) onSelect(nextId);
  }

  function moveBy(delta) {
    if (ids.length === 0) return;
    const curIdx = Math.max(0, ids.indexOf(activeId));
    const nextIdx = Math.min(ids.length - 1, Math.max(0, curIdx + delta));
    moveTo(ids[nextIdx]);
  }

  function onKeyDown(e) {
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); moveBy(1); break;
      case "ArrowUp": e.preventDefault(); moveBy(-1); break;
      case "Home": e.preventDefault(); moveTo(ids[0]); break;
      case "End": e.preventDefault(); moveTo(ids[ids.length - 1]); break;
      case "Enter":
      case " ": {
        // If a control (checkbox, action button) is focused, let it handle the key.
        if (e.target.closest?.('button, a, input, select, textarea, [role="checkbox"]')) return;
        if (!activeId) return;
        e.preventDefault();
        if (e.key === "Enter") onSelect(activeId);
        else onToggle(activeId);
        break;
      }
      default: break;
    }
  }

  const registerRow = React.useCallback(
    (id) => (el) => {
      if (el) rowRefs.current.set(id, el);
      else rowRefs.current.delete(id);
    },
    []
  );

  return {
    activeId,
    setActiveId,
    registerRow,
    gridProps: { ref: gridRef, tabIndex: 0, onKeyDown },
  };
}
