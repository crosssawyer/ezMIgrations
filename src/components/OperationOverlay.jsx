import * as React from "react";
import { Spinner } from "@/components/ui/spinner";
import { Button } from "@/components/ui/button";
import { useUI } from "@/lib/ui-store";
import { useCancelOperation } from "@/lib/mutations";

// Backend operation IDs → human-readable titles shown above the phase message.
const OPERATION_TITLES = {
  switch_branch: "Switching branch",
  update_database: "Updating database",
  squash: "Squashing migrations",
  add_migration: "Creating migration",
  remove_migration: "Removing migration",
};

export function OperationOverlay() {
  const { overlay } = useUI();
  const cancel = useCancelOperation();
  if (!overlay) return null;

  const title = overlay.operation ? OPERATION_TITLES[overlay.operation] : null;

  return (
    <div className="fixed inset-0 z-[60] flex flex-col items-center justify-center gap-3 bg-background/80 backdrop-blur-sm">
      <Spinner className="size-6 text-primary" />
      {title && (
        <div className="rounded-full border border-border bg-secondary px-2.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-muted-foreground">
          {title}
        </div>
      )}
      <p className="text-sm text-muted-foreground text-center max-w-md px-6">
        {overlay.message || "Working..."}
      </p>
      {overlay.cancelable && (
        <Button
          size="sm"
          variant="ghost"
          onClick={() => cancel.mutate()}
          disabled={cancel.isPending}
        >
          {cancel.isPending ? "Cancelling..." : "Cancel"}
        </Button>
      )}
    </div>
  );
}
