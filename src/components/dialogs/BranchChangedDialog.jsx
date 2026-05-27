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

export function BranchChangedDialog({ onClose, old_branch, new_branch }) {
  const updateDb = useUpdateDatabase();
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
          <p>Would you like to update the database to match the migrations on the new branch?</p>
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
