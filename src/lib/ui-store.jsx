import * as React from "react";

const UIContext = React.createContext(null);

export function useUI() {
  const ctx = React.useContext(UIContext);
  if (!ctx) throw new Error("useUI must be inside UIProvider");
  return ctx;
}

export function UIProvider({ children }) {
  const [overlay, setOverlay] = React.useState(null);
  const [dialog, setDialog] = React.useState(null);
  const [hotkeysOpen, setHotkeysOpen] = React.useState(false);
  const [settingsOpen, setSettingsOpen] = React.useState(false);
  const [selectedMigrationId, setSelectedMigrationId] = React.useState(null);
  const [checked, setChecked] = React.useState(() => new Set());
  const [searchQuery, setSearchQuery] = React.useState("");
  const [previousBranch, setPreviousBranch] = React.useState(null);
  const [syncDismissed, setSyncDismissed] = React.useState(false);

  const openDialog = React.useCallback((type, props = {}) => setDialog({ type, props }), []);
  const closeDialog = React.useCallback(() => setDialog(null), []);
  const toggleChecked = React.useCallback((id) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);
  const clearChecked = React.useCallback(() => setChecked(new Set()), []);

  const value = React.useMemo(
    () => ({
      overlay,
      setOverlay,
      dialog,
      openDialog,
      closeDialog,
      hotkeysOpen,
      setHotkeysOpen,
      settingsOpen,
      setSettingsOpen,
      selectedMigrationId,
      setSelectedMigrationId,
      checked,
      setChecked,
      toggleChecked,
      clearChecked,
      searchQuery,
      setSearchQuery,
      previousBranch,
      setPreviousBranch,
      syncDismissed,
      setSyncDismissed,
    }),
    [
      overlay,
      dialog,
      openDialog,
      closeDialog,
      hotkeysOpen,
      settingsOpen,
      selectedMigrationId,
      checked,
      toggleChecked,
      clearChecked,
      searchQuery,
      previousBranch,
      syncDismissed,
    ]
  );

  return <UIContext.Provider value={value}>{children}</UIContext.Provider>;
}
