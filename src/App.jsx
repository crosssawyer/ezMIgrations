import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";

import { UIProvider, useUI } from "@/lib/ui-store";
import { useProject, usePreferences, queryKeys } from "@/lib/queries";
import { useUpdateDatabase, useFetchRemote } from "@/lib/mutations";
import { listen, invoke } from "@/lib/tauri";
import { useRefreshOnVisible } from "@/lib/hooks";
import { Spinner } from "@/components/ui/spinner";
import { Header } from "@/components/Header";
import { SetupView } from "@/components/SetupView";
import { MainView } from "@/components/MainView";
import { OperationOverlay } from "@/components/OperationOverlay";

const SettingsSheet = React.lazy(() =>
  import("@/components/SettingsSheet").then((m) => ({ default: m.SettingsSheet }))
);
const HotkeysDialog = React.lazy(() =>
  import("@/components/HotkeysDialog").then((m) => ({ default: m.HotkeysDialog }))
);
const DialogRoot = React.lazy(() =>
  import("@/components/dialogs/DialogRoot").then((m) => ({ default: m.DialogRoot }))
);

function AppShell() {
  const { data: project, isLoading } = useProject();
  const { data: preferences } = usePreferences();
  const qc = useQueryClient();
  const ui = useUI();
  const updateDb = useUpdateDatabase();
  const fetchRemote = useFetchRemote();

  React.useEffect(() => {
    let cancelled = false;
    let unMigrations = null;
    let unBranch = null;
    let unPhase = null;
    listen("migrations-changed", () => {
      qc.invalidateQueries({ queryKey: queryKeys.migrations });
    }).then((un) => {
      if (cancelled) un();
      else unMigrations = un;
    });
    listen("branch-changed", (event) => {
      const { old_branch, new_branch, reverted_to_stable } = event.payload;
      ui.setPreviousBranch(old_branch);
      ui.setSyncDismissed(false);
      qc.setQueryData(queryKeys.currentBranch, new_branch);
      qc.invalidateQueries({ queryKey: queryKeys.migrations });
      if (preferences?.notify_on_branch_change !== false) {
        ui.openDialog("branchChanged", { old_branch, new_branch, reverted_to_stable });
      }
    }).then((un) => {
      if (cancelled) un();
      else unBranch = un;
    });
    listen("operation-phase", (event) => {
      const { operation, message } = event?.payload ?? {};
      if (!message) return;
      ui.setOverlay((prev) =>
        prev ? { ...prev, operation: operation ?? prev.operation, message } : prev
      );
    }).then((un) => {
      if (cancelled) un();
      else unPhase = un;
    });
    return () => {
      cancelled = true;
      unMigrations?.();
      unBranch?.();
      unPhase?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [qc, preferences?.notify_on_branch_change]);

  React.useEffect(() => {
    if (!project) return;
    invoke("start_branch_watcher").catch(() => {});
    invoke("start_migration_watcher").catch(() => {});
  }, [project]);

  useRefreshOnVisible(() => {
    qc.invalidateQueries({ queryKey: queryKeys.currentBranch });
    qc.invalidateQueries({ queryKey: queryKeys.migrations });
  }, { enabled: !!project });

  React.useEffect(() => {
    function onKey(e) {
      const mod = e.ctrlKey || e.metaKey;
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      const tag = document.activeElement?.tagName;
      const inInput = tag === "INPUT" || tag === "TEXTAREA";
      // Whole-app actions should only fire from the main view, not while a
      // dialog/settings/help surface is in front.
      const inMainView = !ui.dialog && !ui.settingsOpen && !ui.hotkeysOpen;

      if (e.key === "Escape") {
        if (ui.hotkeysOpen) ui.setHotkeysOpen(false);
        else if (ui.dialog) ui.closeDialog();
        else if (ui.settingsOpen) ui.setSettingsOpen(false);
        else if (ui.selectedMigrationId) ui.setSelectedMigrationId(null);
        return;
      }
      if (!project) return;
      if (mod && !inInput && inMainView && key === "n") {
        e.preventDefault();
        ui.openDialog("newMigration");
        return;
      }
      if (mod && !inInput && key === "r") {
        e.preventDefault();
        qc.invalidateQueries({ queryKey: queryKeys.migrations });
        return;
      }
      // Fetch remote branches — also works inside the branch dialog.
      if (mod && e.shiftKey && key === "f") {
        e.preventDefault();
        if (!fetchRemote.isPending) fetchRemote.mutate();
        return;
      }
      if (mod && !e.shiftKey && key === "f") {
        e.preventDefault();
        document.querySelector('[data-search-input]')?.focus();
        return;
      }
      if (mod && !inInput && inMainView && key === "u") {
        e.preventDefault();
        if (!updateDb.isPending) updateDb.mutate({});
        return;
      }
      if (mod && !inInput && inMainView && key === "b") {
        e.preventDefault();
        ui.openDialog("switchBranch");
        return;
      }
      if (e.key === "?" && !inInput) {
        ui.setHotkeysOpen((v) => !v);
      }
    }
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [ui, project, qc, updateDb.mutate, updateDb.isPending, fetchRemote.mutate, fetchRemote.isPending]);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground overflow-hidden">
      <Header project={project} />
      <main className="flex-1 min-h-0 flex flex-col">
        {isLoading ? (
          <div className="flex flex-1 items-center justify-center gap-3 text-muted-foreground">
            <Spinner className="size-5" />
            <span className="text-sm">Loading project...</span>
          </div>
        ) : project ? (
          <MainView project={project} />
        ) : (
          <SetupView />
        )}
      </main>

      <React.Suspense fallback={null}>
        <SettingsSheet />
        <HotkeysDialog />
        <DialogRoot />
      </React.Suspense>
      <OperationOverlay />
    </div>
  );
}

export function App() {
  return (
    <UIProvider>
      <AppShell />
    </UIProvider>
  );
}
