import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "./toast";
import { invoke } from "./tauri";
import { queryKeys } from "./queries";

const { errToast } = toast;

const invalidateMigrations = (qc) =>
  qc.invalidateQueries({ queryKey: queryKeys.migrations });

const invalidateProjectAndMigrations = (qc) => {
  qc.invalidateQueries({ queryKey: queryKeys.project });
  qc.invalidateQueries({ queryKey: queryKeys.migrations });
  qc.invalidateQueries({ queryKey: queryKeys.currentBranch });
};

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
    mutationFn: (project) => invoke("save_project", { project }),
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
    mutationFn: (project) => invoke("update_saved_project", { project }),
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
  return useMutation({
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
  return useMutation({
    mutationFn: ({ fromMigration, toMigration, newName }) =>
      invoke("squash_migrations", { fromMigration, toMigration, newName }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: errToast("Failed to squash migrations"),
  });
}

export function useUpdateDatabase() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ target = "" } = {}) => invoke("update_database", { target }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: errToast("Migration failed"),
  });
}

export function useRemoveMigration() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ force = false } = {}) => invoke("remove_migration", { force }),
    onSuccess: (msg) => {
      toast.success(msg);
      invalidateMigrations(qc);
    },
    onError: errToast("Failed to remove migration"),
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

export function useSwitchBranch() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ targetBranch }) => invoke("switch_branch_with_migrations", { targetBranch }),
    onSuccess: (result) => {
      qc.invalidateQueries({ queryKey: queryKeys.currentBranch });
      qc.invalidateQueries({ queryKey: queryKeys.branches });
      qc.invalidateQueries({ queryKey: queryKeys.migrations });
      if (result?.rollback_performed) {
        const target = result.rollback_target === "0" ? "base" : result.rollback_target;
        toast.success(`Switched to ${result.new_branch}; rolled back to ${target} first.`);
      } else {
        toast.success(`Switched to ${result.new_branch}; database updated.`);
      }
    },
    onError: errToast("Failed to switch branch"),
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

export function useCancelOperation() {
  return useMutation({
    mutationFn: () => invoke("cancel_running_operation"),
    onSuccess: (msg) => toast(msg),
    onError: errToast("Failed to cancel operation"),
  });
}
