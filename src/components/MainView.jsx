import * as React from "react";

import { MigrationsToolbar } from "./migrations/MigrationsToolbar";
import { MigrationsBanners } from "./migrations/MigrationsBanners";
import { WhatsNewBanner } from "./migrations/WhatsNewBanner";
import { MigrationsTable } from "./migrations/MigrationsTable";
import { DetailPanel } from "./migrations/DetailPanel";
import { useMigrations, useProject, useHasDbConnection, useDbHistory } from "@/lib/queries";
import { useUI } from "@/lib/ui-store";
import { detectOutOfSync } from "./migrations/detect-sync";
import { mergeWithDbHistory } from "./migrations/merge-db-history";

export function MainView() {
  const { selectedMigrationId } = useUI();
  const { data: migrations = [], isLoading, isFetching, isError, error } = useMigrations();
  const { data: project } = useProject();

  const projectId = project?.id;
  const { data: dbConfigured } = useHasDbConnection(projectId);
  const { data: dbRows } = useDbHistory(projectId, { enabled: Boolean(dbConfigured) });

  const mergedMigrations = React.useMemo(
    () => mergeWithDbHistory(migrations, dbConfigured ? dbRows ?? null : null),
    [migrations, dbConfigured, dbRows]
  );

  const syncInfo = React.useMemo(() => detectOutOfSync(migrations), [migrations]);
  const foreignNames = React.useMemo(
    () => new Set(syncInfo.foreignMigrations.map((m) => m.name)),
    [syncInfo]
  );

  return (
    <div className="flex flex-1 min-h-0 flex-col">
      <MigrationsToolbar isFetching={isFetching} />
      <WhatsNewBanner />
      <MigrationsBanners migrations={migrations} syncInfo={syncInfo} />
      <div className="flex flex-1 min-h-0 w-full">
        <MigrationsTable
          migrations={mergedMigrations}
          isLoading={isLoading}
          isFetching={isFetching}
          isError={isError}
          error={error}
          foreignNames={foreignNames}
        />
        {selectedMigrationId ? <DetailPanel migrations={mergedMigrations} /> : null}
      </div>
    </div>
  );
}
