import * as React from "react";
import { AlertTriangle } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useUpdateDatabase } from "@/lib/mutations";
import { useUI } from "@/lib/ui-store";

function Banner({ className, children }) {
  return (
    <div className={cn("flex items-center gap-3 px-4 py-2 border-b", className)}>
      <AlertTriangle className="h-4 w-4 shrink-0" />
      {children}
    </div>
  );
}

export function MigrationsBanners({ migrations, syncInfo }) {
  const { syncDismissed, setSyncDismissed, previousBranch } = useUI();
  const updateDb = useUpdateDatabase();

  const pendingCount = migrations.filter((m) => !m.applied).length;
  const showSync = syncInfo.isOutOfSync && !syncDismissed;
  const showDrift = !showSync && pendingCount > 0;

  if (showSync) {
    return (
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
    );
  }

  if (showDrift) {
    return (
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
    );
  }

  return null;
}
