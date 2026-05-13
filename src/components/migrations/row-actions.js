import * as React from "react";
import { useUpdateDatabase, useRemoveMigration } from "@/lib/mutations";
import { useUI } from "@/lib/ui-store";

export function useApplyTo() {
  const { mutate } = useUpdateDatabase();
  return React.useCallback(
    (migration) => mutate({ target: migration.name }),
    [mutate]
  );
}

export function useRemoveLastOrForce(migrations) {
  const { openDialog } = useUI();
  const { mutate } = useRemoveMigration();
  return React.useCallback(
    (migration) => {
      const isLast = migrations[migrations.length - 1]?.id === migration.id;
      if (!isLast) {
        openDialog("forceRemove", { onConfirm: () => mutate({ force: true }) });
        return;
      }
      mutate({ force: false });
    },
    [migrations, openDialog, mutate]
  );
}
