import * as React from "react";
import { AlertTriangle, Copy } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { copyToClipboard } from "@/lib/utils";

function Section({ label, children, mono = false }) {
  const body = mono
    ? "rounded-md border border-border bg-muted/40 px-3 py-2 font-mono text-xs leading-relaxed text-foreground/90 whitespace-pre-wrap break-words max-h-40 overflow-auto"
    : "font-mono text-xs text-foreground/90";
  return (
    <div className="space-y-1">
      <div className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      <div className={body}>{children}</div>
    </div>
  );
}

export function MigrationErrorDialog({
  onClose,
  title = "Migration failed",
  context,
  error,
}) {
  const { failedMigration, failedDirection, sqlError, statement, fullLog } = error ?? {};
  const migrationLabel = failedDirection === "applying" ? "Failed while applying" : "Failed while reverting";

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-xl p-0 gap-0">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle className="flex items-center gap-2">
            <AlertTriangle className="h-4 w-4 text-destructive" />
            {title}
          </DialogTitle>
          {context && (
            <DialogDescription className="text-sm leading-relaxed text-foreground/80">
              {context}
            </DialogDescription>
          )}
        </DialogHeader>

        <div className="px-5 pb-3 space-y-3">
          {failedMigration && <Section label={migrationLabel}>{failedMigration}</Section>}
          {sqlError && <Section label="SQL error" mono>{sqlError}</Section>}
          {statement && <Section label="Offending statement" mono>{statement}</Section>}
          <p className="text-xs text-muted-foreground leading-relaxed">
            The database is left at the last successful step. Resolve the underlying issue
            (e.g. delete conflicting rows or fix the migration's{" "}
            <code>{failedDirection === "applying" ? "Up()" : "Down()"}</code>) and try again.
          </p>
        </div>

        <DialogFooter className="px-5 py-3 gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => copyToClipboard(fullLog, { successMessage: "Full log copied to clipboard" })}
            disabled={!fullLog}
          >
            <Copy className="h-3.5 w-3.5 mr-1.5" />
            Copy full log
          </Button>
          <Button type="button" size="sm" onClick={onClose}>
            Dismiss
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
