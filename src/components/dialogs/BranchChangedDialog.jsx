import * as React from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useUpdateDatabase } from "@/lib/mutations";
import { useProject } from "@/lib/queries";

export function BranchChangedDialog({ onClose, old_branch, new_branch, reverted_to_stable }) {
  const updateDb = useUpdateDatabase();
  const { data: project } = useProject();
  const stable = project?.stable_migration;
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md p-0 gap-0">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle>Branch changed</DialogTitle>
          <DialogDescription>
            Switched from <strong className="text-foreground">{old_branch}</strong> to{" "}
            <strong className="text-foreground">{new_branch}</strong>.
          </DialogDescription>
        </DialogHeader>
        <div className="px-5 pb-1 text-xs text-muted-foreground">
          {reverted_to_stable && stable && (
            <p>Database was automatically reverted to stable migration <strong>{stable}</strong>. Update to latest on <strong>{new_branch}</strong>?</p>
          )}
          {!reverted_to_stable && stable && (
            <>
              <p className="text-yellow-300">Stable migration <strong>{stable}</strong> is configured for this project.</p>
              <p className="mt-1">Update to latest on <strong>{new_branch}</strong>? If that fails, you may need to revert manually first.</p>
            </>
          )}
          {!reverted_to_stable && !stable && (
            <p>Would you like to update the database to match the migrations on the new branch?</p>
          )}
        </div>
        <DialogFooter className="px-5 py-3">
          <Button variant="ghost" size="sm" onClick={onClose}>Not now</Button>
          <Button
            size="sm"
            onClick={() => {
              updateDb.mutate({}, { onSettled: onClose });
            }}
            disabled={updateDb.isPending}
          >
            {updateDb.isPending ? "Updating…" : "Update to latest"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
