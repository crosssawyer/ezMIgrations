import * as React from "react";
import { ArrowRight, GitBranch, Check, Info } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import {
  Command,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
} from "@/components/ui/command";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useBranches, useCurrentBranch } from "@/lib/queries";
import { useSwitchBranch } from "@/lib/mutations";
import { useUI } from "@/lib/ui-store";

function BranchChip({ label, tone = "muted" }) {
  return (
    <div
      className={cn(
        "inline-flex items-center gap-1.5 rounded-md border px-2 py-1 font-mono text-[11px] leading-none min-w-0 max-w-[200px]",
        tone === "primary"
          ? "border-primary/40 bg-primary/10 text-foreground"
          : "border-border bg-secondary text-foreground"
      )}
    >
      <span
        className={cn(
          "h-1.5 w-1.5 rounded-full shrink-0",
          tone === "primary"
            ? "bg-primary shadow-[0_0_0_3px_hsl(var(--primary)/0.18)]"
            : "bg-muted-foreground"
        )}
      />
      <span className="truncate">{label}</span>
    </div>
  );
}

export function SwitchBranchDialog({ onClose }) {
  const ui = useUI();
  const { data: branches = [], isLoading } = useBranches(true);
  const { data: currentBranch = "" } = useCurrentBranch(true);
  const switchBranch = useSwitchBranch();
  const [selected, setSelected] = React.useState("");

  React.useEffect(() => {
    if (!selected && branches.length > 0) setSelected(branches[0]);
  }, [branches, selected]);

  const onSubmit = (e) => {
    e?.preventDefault();
    if (!selected) return;
    ui.setOverlay({ message: `Switching to ${selected}...`, cancelable: true });
    switchBranch.mutate({ targetBranch: selected }, {
      onSettled: () => {
        ui.setOverlay(null);
        onClose();
      },
    });
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md p-0 gap-0 overflow-hidden">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle>Switch branch</DialogTitle>
          <DialogDescription>
            Branch-only migrations roll back first, then the working tree switches
            and the database updates to latest.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={onSubmit} className="flex flex-col">
          <div className="flex items-center gap-2 px-5 pb-3">
            <BranchChip label={currentBranch || "current"} tone="muted" />
            <ArrowRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
            <BranchChip label={selected || "select branch"} tone="primary" />
          </div>

          <div className="border-y border-border bg-popover">
            <Command className="rounded-none border-0 bg-transparent" value={selected} onValueChange={setSelected}>
              <CommandInput placeholder={isLoading ? "Loading branches…" : "Search branches..."} disabled={isLoading} />
              <CommandList className="max-h-[260px]">
                <CommandEmpty>No matching branches.</CommandEmpty>
                <CommandGroup>
                  {branches.map((b) => (
                    <CommandItem
                      key={b}
                      value={b}
                      onSelect={(v) => setSelected(v)}
                      className="font-mono text-xs"
                    >
                      <GitBranch className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
                      <span className="flex-1 truncate">{b}</span>
                      <Check className={cn("h-3.5 w-3.5 text-primary transition-opacity", selected === b ? "opacity-100" : "opacity-0")} />
                    </CommandItem>
                  ))}
                </CommandGroup>
              </CommandList>
            </Command>
          </div>

          <DialogFooter className="px-5 py-3">
            <Button type="button" variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
            <Button type="submit" size="sm" disabled={!selected || isLoading || switchBranch.isPending}>
              {switchBranch.isPending ? "Switching…" : "Switch & Update"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
