import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "./toast";
import { invoke } from "./tauri";
import { queryKeys } from "./queries";
import { useEfErrorHandler } from "./ef-error-handler";
import { useUI } from "./ui-store";

const { errToast } = toast;

const invalidateMigrations = (qc) =>
  qc.invalidateQueries({ queryKey: queryKeys.migrations });

const invalidateProjectAndMigrations = (qc) => {
  qc.invalidateQueries({ queryKey: queryKeys.project });
  qc.invalidateQueries({ queryKey: queryKeys.migrations });
  qc.invalidateQueries({ queryKey: queryKeys.currentBranch });
};

const invalidateBranchState = (qc) => {
  qc.invalidateQueries({ queryKey: queryKeys.currentBranch });
  qc.invalidateQueries({ queryKey: queryKeys.branches });
  qc.invalidateQueries({ queryKey: queryKeys.migrations });
};

/**
 * useMutation wrapper that shows OperationOverlay for the duration of the call.
 * The global `operation-phase` listener (App.jsx) replaces the message once
 * backend events arrive.
 *
 *   operation       -> matches the backend operation id; rendered as a title chip
 *                      and used by the cancellation-as-neutral check below.
 *   overlayMessage  -> initial text shown the instant the user clicks, before
 *                      the first backend phase event arrives.
 */
function useOperationMutation({
  operation,
  overlayMessage,
  onSuccess,
  onError,
  ...rest
}) {
  const { setOverlay } = useUI();
  return useMutation({
    ...rest,
    mutationFn: async (vars) => {
      setOverlay({
        operation,
        message: overlayMessage(vars),
        cancelable: true,
      });
      return rest.mutationFn(vars);
    },
    onSuccess: (data, vars, ctx) => {
      setOverlay(null);
      onSuccess?.(data, vars, ctx);
    },
    onError: (err, vars, ctx) => {
      setOverlay(null);
      // Cancellation is a user action, not a failure — surface neutrally and
      // skip the EF/error toast pipeline.
      const message = typeof err === "string" ? err : err?.message || "";
      if (message.includes("Canceled by user")) {
        toast("Operation canceled.");
        return;
      }
      onError?.(err, vars, ctx);
    },
  });
}

export function useSetProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (vars) => invoke("set_project", vars),
    onSuccess: () => {
      invalidateProjectAndMigrations(qc);
      toast.success("Project connected");
    },
    onError: errToast("Failed to connect project"),
  });
}

export function useSwitchProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id }) => invoke("switch_project", { id }),
    onSuccess: (project) => {
      invalidateProjectAndMigrations(qc);
      toast.success(`Switched to ${project?.name || "project"}`);
    },
    onError: errToast("Failed to switch project"),
  });
}

export function useSaveProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (project) =>
      invoke("save_project", {
        name: project.name,
        path: project.project_path,
        dbContext: project.db_context,
        startupProject: project.startup_project,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.savedProjects });
      toast.success("Project saved");
    },
    onError: errToast("Failed to save project"),
  });
}

export function useUpdateSavedProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (project) =>
      invoke("update_saved_project", {
        id: project.id,
        name: project.name,
        path: project.project_path,
        dbContext: project.db_context,
        startupProject: project.startup_project,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.savedProjects });
      toast.success("Project updated");
    },
    onError: errToast("Failed to update project"),
  });
}

export function useDeleteSavedProject() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id) => invoke("delete_saved_project", { id }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.savedProjects });
      toast.success("Project removed");
    },
    onError: errToast("Failed to remove project"),
  });
}

export function useAddMigration() {
  const qc = useQueryClient();
  return useOperationMutation({
    operation: "add_migration",
    overlayMessage: ({ name }) => `Creating migration ${name}…`,
    mutationFn: ({ name }) => invoke("add_migration", { name }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: errToast("Failed to create migration"),
  });
}

export function useSquashMigrations() {
  const qc = useQueryClient();
  const handleEfError = useEfErrorHandler();
  return useOperationMutation({
    operation: "squash",
    overlayMessage: ({ fromMigration, toMigration }) =>
      `Preparing to squash ${fromMigration} → ${toMigration}…`,
    mutationFn: ({ fromMigration, toMigration, newName }) =>
      invoke("squash_migrations", { fromMigration, toMigration, newName }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: (err) =>
      handleEfError(err, {
        title: "Squash failed",
        context: "The squash operation could not complete. The database is at the last successful step.",
        toastPrefix: "Failed to squash migrations",
      }),
  });
}

export function useUpdateDatabase() {
  const qc = useQueryClient();
  const handleEfError = useEfErrorHandler();
  return useOperationMutation({
    operation: "update_database",
    overlayMessage: ({ target = "" } = {}) => {
      const label = target === "" ? "latest" : target === "0" ? "base" : target;
      return `Starting database update to ${label}…`;
    },
    mutationFn: ({ target = "" } = {}) => invoke("update_database", { target }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: (err) =>
      handleEfError(err, {
        rollback: {
          title: "Migration rollback failed",
          context: "Couldn't roll the database back. The DB is at the last successful step.",
          toastPrefix: "Migration failed",
        },
        apply: {
          title: "Migration failed",
          context: "An error occurred while applying a migration. The DB is at the last successful step.",
          toastPrefix: "Migration failed",
        },
      }),
  });
}

export function useRemoveMigration() {
  const qc = useQueryClient();
  const handleEfError = useEfErrorHandler();
  return useOperationMutation({
    operation: "remove_migration",
    overlayMessage: ({ force = false } = {}) =>
      force ? "Removing migration (force)…" : "Removing last migration…",
    mutationFn: ({ force = false } = {}) => invoke("remove_migration", { force }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: (err) =>
      handleEfError(err, {
        title: "Failed to remove migration",
        context: "The migration could not be removed. The database is at the last successful step.",
        toastPrefix: "Failed to remove migration",
      }),
  });
}

export function useSetStable() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ migrationName }) => invoke("set_stable_migration", { migrationName }),
    onSuccess: (_, vars) => {
      qc.invalidateQueries({ queryKey: queryKeys.project });
      toast.success(
        vars.migrationName ? `Stable migration set to ${vars.migrationName}` : "Stable migration cleared"
      );
    },
    onError: errToast("Failed to set stable migration"),
  });
}

export function useFetchRemote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => invoke("fetch_remote"),
    onSuccess: () => {
      // Fetch only updates remote-tracking refs; it never moves HEAD, so the
      // current branch can't change — just refresh the branch list.
      qc.invalidateQueries({ queryKey: queryKeys.branches });
      toast.success("Fetched latest from remote");
    },
    onError: errToast("Failed to fetch from remote"),
  });
}

export function useSwitchBranch() {
  const qc = useQueryClient();
  const handleEfError = useEfErrorHandler();
  return useOperationMutation({
    operation: "switch_branch",
    overlayMessage: ({ targetBranch }) => `Preparing to switch to ${targetBranch}…`,
    mutationFn: ({ targetBranch }) => invoke("switch_branch_with_migrations", { targetBranch }),
    onSuccess: (result) => {
      invalidateBranchState(qc);
      if (result?.rollback_performed) {
        const target = result.rollback_target === "0" ? "base" : result.rollback_target;
        toast.success(`Switched to ${result.new_branch}; rolled back to ${target} first.`);
      } else {
        toast.success(`Switched to ${result.new_branch}; database updated.`);
      }
    },
    onError: (err) => {
      invalidateBranchState(qc);
      handleEfError(err, {
        rollback: {
          title: "Branch switch aborted — rollback failed",
          context:
            "Couldn't roll the database back to the common migration, so the branch switch was not performed. Your working tree is unchanged.",
          toastPrefix: "Failed to switch branch",
        },
        apply: {
          title: "Branch switch failed",
          context: "The branch was switched, but the database update on the new branch failed.",
          toastPrefix: "Failed to switch branch",
        },
      });
    },
  });
}

export function useSetPreferences() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (preferences) => invoke("set_preferences", { preferences }),
    onSuccess: (_, vars) => qc.setQueryData(queryKeys.preferences, vars),
    onError: errToast("Failed to save preferences"),
  });
}

export function useStartMcpServer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => invoke("start_mcp_server"),
    onSuccess: (status) => {
      qc.setQueryData(queryKeys.mcpStatus, status);
      toast.success("MCP server started");
    },
    onError: errToast("Failed to start MCP server"),
  });
}

export function useStopMcpServer() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => invoke("stop_mcp_server"),
    onSuccess: (status) => {
      qc.setQueryData(queryKeys.mcpStatus, status);
      toast("MCP server stopped");
    },
    onError: errToast("Failed to stop MCP server"),
  });
}

export function useOpenMcpTerminal() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ terminal = "system" } = {}) => invoke("open_mcp_terminal", { terminal }),
    onSuccess: (msg) => {
      qc.invalidateQueries({ queryKey: queryKeys.mcpStatus });
      toast.success(msg);
    },
    onError: errToast("Failed to open MCP terminal"),
  });
}

export function useCancelOperation() {
  return useMutation({
    mutationFn: () => invoke("cancel_running_operation"),
    onSuccess: (msg) => toast(msg),
    onError: errToast("Failed to cancel operation"),
  });
}
