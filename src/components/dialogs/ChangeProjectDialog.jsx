import * as React from "react";
import { FolderOpen } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useUI } from "@/lib/ui-store";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys } from "@/lib/queries";

export function ChangeProjectDialog({ onClose }) {
  const qc = useQueryClient();
  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md p-0 gap-0">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle className="flex items-center gap-2">
            <FolderOpen className="h-4 w-4 text-primary" /> Change project?
          </DialogTitle>
          <DialogDescription>
            This will disconnect the current project. Pick a new one from the setup screen or settings.
          </DialogDescription>
        </DialogHeader>
        <DialogFooter className="px-5 py-3">
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button
            size="sm"
            onClick={() => {
              qc.setQueryData(queryKeys.project, null);
              onClose();
            }}
          >
            Disconnect
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
