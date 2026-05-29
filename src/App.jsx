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

  // Keep the latest handler in a ref so the document listener can subscribe
  // once for the app's lifetime instead of re-binding on every UI state change.
  // The ref is refreshed in a commit-phase effect (below) to keep render pure.
  const onKeyRef = React.useRef(null);
  function onKey(e) {
    const mod = e.ctrlKey || e.metaKey;
    const shift = e.shiftKey;
    const key = e.key.toLowerCase();
    const tag = document.activeElement?.tagName;
    const inInput = tag === "INPUT" || tag === "TEXTAREA";
    const inMainView = !ui.dialog && !ui.settingsOpen && !ui.hotkeysOpen;

    const closeTopLayer = () => {
      if (ui.hotkeysOpen) ui.setHotkeysOpen(false);
      else if (ui.dialog) ui.closeDialog();
      else if (ui.settingsOpen) ui.setSettingsOpen(false);
      else if (ui.selectedMigrationId) ui.setSelectedMigrationId(null);
    };

    // Each row reads as: which key, the modifier expectation, where it's
    // allowed to fire, and what it does. `shift` is true (required) / false
    // (must be up, to split ⌘F from ⌘⇧F) / omitted (don't care). `scope` is
    // "always" | "noInput" | "mainView" (mainView also implies not-in-input).
    const BINDINGS = [
      { key: "escape", scope: "always", requiresProject: false, run: closeTopLayer },
      { mod: true, key: "n", scope: "mainView", run: () => ui.openDialog("newMigration") },
      { mod: true, key: "r", scope: "noInput", run: () => qc.invalidateQueries({ queryKey: queryKeys.migrations }) },
      { mod: true, shift: true, key: "f", scope: "always", run: () => !fetchRemote.isPending && fetchRemote.mutate() },
      { mod: true, shift: false, key: "f", scope: "always", run: () => document.querySelector("[data-search-input]")?.focus() },
      { mod: true, key: "u", scope: "mainView", run: () => !updateDb.isPending && updateDb.mutate({}) },
      { mod: true, key: "b", scope: "mainView", run: () => ui.openDialog("switchBranch") },
      { key: "?", scope: "noInput", run: () => ui.setHotkeysOpen((v) => !v) },
    ];

    for (const b of BINDINGS) {
      if (b.key !== key) continue;
      if ((b.mod ?? false) !== mod) continue;
      if (b.shift !== undefined && b.shift !== shift) continue;
      if (b.scope === "noInput" && inInput) continue;
      if (b.scope === "mainView" && (inInput || !inMainView)) continue;
      if ((b.requiresProject ?? true) && !project) return;
      if (b.mod) e.preventDefault();
      b.run();
      return;
    }
  }

  // Refresh the ref every commit so the once-bound listener always calls the
  // latest closure (fresh `ui`/`project`/mutation state) without re-binding.
  React.useEffect(() => {
    onKeyRef.current = onKey;
  });

  React.useEffect(() => {
    const handler = (e) => onKeyRef.current?.(e);
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, []);

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
