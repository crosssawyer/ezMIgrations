import * as React from "react";
import { useForm } from "@tanstack/react-form";
import { Layers } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useUI } from "@/lib/ui-store";
import { useMigrations } from "@/lib/queries";
import { useSquashMigrations } from "@/lib/mutations";

export function SquashDialog({ onClose }) {
  const ui = useUI();
  const { data: migrations = [] } = useMigrations();
  const squash = useSquashMigrations();

  const selected = React.useMemo(
    () => migrations.filter((m) => ui.checked.has(m.id)),
    [migrations, ui.checked]
  );
  const fromM = selected[0];
  const toM = selected[selected.length - 1];

  const form = useForm({
    defaultValues: { newName: "" },
    onSubmit: async ({ value }) => {
      if (!fromM || !toM || !value.newName.trim()) return;
      await squash.mutateAsync({
        fromMigration: fromM.name,
        toMigration: toM.name,
        newName: value.newName.trim(),
      });
      ui.setChecked(new Set());
      onClose();
    },
  });

  if (!fromM || !toM) return null;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md p-0 gap-0">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle className="flex items-center gap-2">
            <Layers className="h-4 w-4 text-primary" /> Squash migrations
          </DialogTitle>
          <DialogDescription>
            Reverts the DB, removes the selected migrations, creates a new one,
            then re-applies. Custom SQL is preserved.
          </DialogDescription>
        </DialogHeader>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            form.handleSubmit();
          }}
        >
          <div className="px-5 py-3 flex flex-col gap-3">
            <div className="rounded-md border border-border bg-muted/40 px-3 py-2 text-xs">
              Squashing <strong>{selected.length}</strong> migrations:
              <div className="mt-1 font-mono text-[11px] text-muted-foreground">
                {fromM.name} <span className="text-foreground">→</span> {toM.name}
              </div>
            </div>
            <form.Field name="newName">
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name}>New migration name</Label>
                  <Input
                    id={field.name}
                    autoFocus
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    placeholder="SquashedMigration"
                    className="font-mono"
                  />
                </div>
              )}
            </form.Field>
          </div>
          <DialogFooter className="px-5 py-3">
            <Button type="button" variant="ghost" size="sm" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" size="sm" disabled={squash.isPending}>
              {squash.isPending ? "Squashing…" : "Squash"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
