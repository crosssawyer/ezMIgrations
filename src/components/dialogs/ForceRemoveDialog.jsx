import * as React from "react";
import { AlertTriangle } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

export function ForceRemoveDialog({ onClose, onConfirm }) {
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md p-0 gap-0">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle className="flex items-center gap-2 text-yellow-300">
            <AlertTriangle className="h-4 w-4" /> Remove migration?
          </DialogTitle>
          <DialogDescription>
            This is not the last migration. Force-removing deletes the migration
            without reverting changes from the database. Use with caution.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="px-5 py-3">
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            variant="destructive"
            size="sm"
            onClick={() => {
              onConfirm?.();
              onClose();
            }}
          >
            Force Remove
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
