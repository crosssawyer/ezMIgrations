import { useQuery } from "@tanstack/react-query";
import { invoke } from "./tauri";

export const queryKeys = {
  project: ["project"],
  migrations: ["migrations"],
  migrationSql: (name) => ["migration-sql", name],
  branches: ["branches"],
  currentBranch: ["current-branch"],
  savedProjects: ["saved-projects"],
  preferences: ["preferences"],
  hasDbConnection: (projectId) => ["has-db-connection", projectId],
};

export function useProject() {
  return useQuery({
    queryKey: queryKeys.project,
    queryFn: () => invoke("get_project").catch(() => null),
    staleTime: 60_000,
  });
}

export function useMigrations({ enabled = true } = {}) {
  return useQuery({
    queryKey: queryKeys.migrations,
    queryFn: () => invoke("list_migrations"),
    enabled,
    staleTime: 5_000,
  });
}

export function useMigrationSql(name) {
  return useQuery({
    queryKey: queryKeys.migrationSql(name),
    queryFn: () => invoke("get_migration_sql", { migrationName: name }),
    enabled: Boolean(name),
    staleTime: 60_000,
  });
}

export function useBranches() {
  return useQuery({
    queryKey: queryKeys.branches,
    queryFn: () => invoke("list_git_branches"),
    staleTime: 30_000,
  });
}

export function useCurrentBranch() {
  return useQuery({
    queryKey: queryKeys.currentBranch,
    queryFn: () => invoke("get_current_branch").catch(() => ""),
    staleTime: 30_000,
  });
}

export function useSavedProjects() {
  return useQuery({
    queryKey: queryKeys.savedProjects,
    queryFn: () => invoke("get_saved_projects"),
    staleTime: 30_000,
  });
}

export function useHasDbConnection(projectId, { enabled = true } = {}) {
  return useQuery({
    queryKey: queryKeys.hasDbConnection(projectId),
    queryFn: () => invoke("has_db_connection", { projectId }),
    enabled: enabled && Boolean(projectId),
    staleTime: Infinity,
  });
}

export function usePreferences() {
  return useQuery({
    queryKey: queryKeys.preferences,
    queryFn: () => invoke("get_preferences").catch(() => ({ notify_on_branch_change: true })),
    staleTime: Infinity,
  });
}
