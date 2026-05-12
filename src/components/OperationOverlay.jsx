import * as React from "react";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useUI } from "@/lib/ui-store";
import { useCancelOperation } from "@/lib/mutations";

export function OperationOverlay() {
  const { overlay } = useUI();
  const cancel = useCancelOperation();
  if (!overlay) return null;

  return (
    <div className="fixed inset-0 z-[60] flex flex-col items-center justify-center gap-3 bg-background/80 backdrop-blur-sm">
      <Loader2 className="h-6 w-6 animate-spin text-primary" />
      <p className="text-sm text-muted-foreground">{overlay.message || "Working..."}</p>
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
