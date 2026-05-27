import * as React from "react";

import { MigrationsToolbar } from "./migrations/MigrationsToolbar";
import { MigrationsBanners } from "./migrations/MigrationsBanners";
import { MigrationsTable } from "./migrations/MigrationsTable";
import { DetailPanel } from "./migrations/DetailPanel";
import { useMigrations } from "@/lib/queries";
import { useUI } from "@/lib/ui-store";
import { detectOutOfSync } from "./migrations/detect-sync";

export function MainView() {
  const { selectedMigrationId } = useUI();
  const { data: migrations = [], isLoading, isFetching, isError, error } = useMigrations();

  const syncInfo = React.useMemo(() => detectOutOfSync(migrations), [migrations]);
  const foreignNames = React.useMemo(
    () => new Set(syncInfo.foreignMigrations.map((m) => m.name)),
    [syncInfo]
  );

  return (
    <div className="flex flex-1 min-h-0 flex-col">
      <MigrationsToolbar isFetching={isFetching} />
      <MigrationsBanners migrations={migrations} syncInfo={syncInfo} />
      <div className="flex flex-1 min-h-0 w-full">
        <MigrationsTable
          migrations={migrations}
          isLoading={isLoading}
          isFetching={isFetching}
          isError={isError}
          error={error}
          foreignNames={foreignNames}
        />
        {selectedMigrationId ? <DetailPanel migrations={migrations} /> : null}
      </div>
    </div>
  );
}
