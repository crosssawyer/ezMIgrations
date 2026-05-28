import * as React from "react";
import { AlertTriangle, Database } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useUpdateDatabase } from "@/lib/mutations";
import { useUI } from "@/lib/ui-store";
import { useProject, useSavedProjects, useDbHistory, useHasDbConnection } from "@/lib/queries";

function Banner({ className, icon: Icon = AlertTriangle, children }) {
  return (
    <div className={cn("flex items-center gap-3 px-4 py-2 border-b", className)}>
      <Icon className="h-4 w-4 shrink-0" />
      {children}
    </div>
  );
}

export function MigrationsBanners({ migrations, syncInfo }) {
  const ui = useUI();
  const { syncDismissed, setSyncDismissed, previousBranch } = ui;
  const updateDb = useUpdateDatabase();

  const pendingCount = migrations.filter((m) => !m.applied).length;
  const showSync = syncInfo.isOutOfSync && !syncDismissed;
  const showDrift = !showSync && pendingCount > 0;

  // DB-history failure: a connection is configured for the active project but
  // the fetch errored (bad credentials, network, etc.). Surface it inline
  // rather than silently falling back to local-only state.
  const dbErrorBanner = <DbHistoryErrorBanner ui={ui} />;

  return (
    <>
      {dbErrorBanner}
      {showSync && (
        <Banner className="border-destructive/40 bg-destructive/10 text-destructive">
          <span className="flex-1 text-xs">
            Out-of-sync — <strong>{syncInfo.foreignMigrations.length}</strong> migration
            {syncInfo.foreignMigrations.length === 1 ? "" : "s"} from{" "}
            <strong>{previousBranch || "another branch"}</strong> are still applied.
          </span>
          <Button
            size="xs"
            variant="destructive"
            onClick={() => {
              const { firstPendingIdx } = syncInfo;
              const target = firstPendingIdx === 0 ? "0" : migrations[firstPendingIdx - 1].name;
              setSyncDismissed(false);
              updateDb.mutate({ target });
            }}
            disabled={updateDb.isPending}
          >
            Revert Foreign
          </Button>
          <Button size="xs" variant="ghost" onClick={() => setSyncDismissed(true)}>
            Dismiss
          </Button>
        </Banner>
      )}
      {!showSync && showDrift && (
        <Banner className="border-yellow-500/40 bg-yellow-500/10 text-yellow-200">
          <span className="flex-1 text-xs">
            Database is out of sync — pending migrations need to be applied.
          </span>
          <Button
            size="xs"
            onClick={() => updateDb.mutate({})}
            disabled={updateDb.isPending}
          >
            Update Now
          </Button>
        </Banner>
      )}
    </>
  );
}

function DbHistoryErrorBanner({ ui }) {
  const { data: project } = useProject();
  const { data: savedProjects = [] } = useSavedProjects();
  const { data: dbConfigured } = useHasDbConnection(project?.id);
  const { isError, error } = useDbHistory(project?.id, { enabled: Boolean(dbConfigured) });

  if (!isError) return null;

  const savedProject = savedProjects.find((p) => p.id === project?.id);
  const message = typeof error === "string" ? error : error?.message || "Unknown error";

  return (
    <Banner className="border-amber-500/40 bg-amber-500/10 text-amber-200" icon={Database}>
      <span className="flex-1 text-xs truncate" title={message}>
        DB verification unavailable: <span className="text-amber-100/90">{message}</span>
      </span>
      {savedProject && (
        <Button
          size="xs"
          variant="outline"
          onClick={() => ui.openDialog("editProject", { project: savedProject })}
        >
          Reconfigure
        </Button>
      )}
    </Banner>
  );
}
