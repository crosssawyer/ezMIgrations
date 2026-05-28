import * as React from "react";
import { ArrowRight, GitBranch, Check, Cloud, RefreshCw } from "lucide-react";

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
import { useSwitchBranch, useFetchRemote } from "@/lib/mutations";

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

function BranchCommandItem({ name, isRemote, isSelected, onSelect }) {
  const Icon = isRemote ? Cloud : GitBranch;
  return (
    <CommandItem value={name} onSelect={onSelect} className="font-mono text-xs">
      <Icon className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
      <span className="flex-1 truncate">{name}</span>
      {isRemote && (
        <span className="text-[10px] uppercase tracking-wider text-muted-foreground/70 shrink-0">
          remote
        </span>
      )}
      <Check
        className={cn(
          "h-3.5 w-3.5 text-primary transition-opacity",
          isSelected ? "opacity-100" : "opacity-0"
        )}
      />
    </CommandItem>
  );
}

export function SwitchBranchDialog({ onClose }) {
  const { data: branches = [], isLoading } = useBranches();
  const { data: currentBranch = "" } = useCurrentBranch();
  const switchBranch = useSwitchBranch();
  const fetchRemote = useFetchRemote();
  const [selected, setSelected] = React.useState("");

  const locals = React.useMemo(
    () => branches.filter((b) => !b.isRemote),
    [branches]
  );
  const remotes = React.useMemo(
    () => branches.filter((b) => b.isRemote),
    [branches]
  );

  const onSubmit = (e) => {
    e?.preventDefault();
    if (!selected) return;
    switchBranch.mutate(
      { targetBranch: selected },
      { onSuccess: () => onClose() }
    );
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="!flex !flex-col !w-[min(28rem,calc(100vw-2rem))] !max-w-none !max-h-[calc(100vh-2rem)] p-0 gap-0 overflow-hidden">
        <DialogHeader className="shrink-0 px-5 pt-5 pb-2">
          <div className="flex items-start justify-between gap-3">
            <div className="flex flex-col gap-1.5 min-w-0">
              <DialogTitle>Switch branch</DialogTitle>
              <DialogDescription className="text-sm leading-relaxed text-foreground/80">
                Branch-only migrations roll back first, then the working tree switches
                and the database updates to latest.
              </DialogDescription>
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="shrink-0 gap-1.5"
              onClick={() => fetchRemote.mutate()}
              disabled={fetchRemote.isPending || switchBranch.isPending}
              title="Fetch the latest branches from the remote"
            >
              <RefreshCw
                className={cn("h-3.5 w-3.5", fetchRemote.isPending && "animate-spin")}
              />
              {fetchRemote.isPending ? "Fetching…" : "Fetch"}
            </Button>
          </div>
        </DialogHeader>

        <form onSubmit={onSubmit} className="flex flex-col min-h-0 flex-1">
          <div className="shrink-0 flex items-center gap-2 px-5 pb-3 min-w-0">
            <BranchChip label={currentBranch || "current"} tone="muted" />
            <ArrowRight className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
            <BranchChip label={selected || "select branch"} tone="primary" />
          </div>

          <div className="flex-1 min-h-0 flex flex-col border-y border-border bg-popover">
            <Command className="flex-1 min-h-0 rounded-none border-0 bg-transparent">
              <CommandInput placeholder={isLoading ? "Loading branches…" : "Search branches..."} disabled={isLoading} />
              <CommandList className="flex-1 min-h-0 max-h-none">
                <CommandEmpty>No matching branches.</CommandEmpty>
                {locals.length > 0 && (
                  <CommandGroup heading="Local">
                    {locals.map((b) => (
                      <BranchCommandItem
                        key={`local:${b.name}`}
                        name={b.name}
                        isRemote={false}
                        isSelected={selected === b.name}
                        onSelect={setSelected}
                      />
                    ))}
                  </CommandGroup>
                )}
                {remotes.length > 0 && (
                  <CommandGroup heading="Remote">
                    {remotes.map((b) => (
                      <BranchCommandItem
                        key={`remote:${b.name}`}
                        name={b.name}
                        isRemote
                        isSelected={selected === b.name}
                        onSelect={setSelected}
                      />
                    ))}
                  </CommandGroup>
                )}
              </CommandList>
            </Command>
          </div>

          <DialogFooter className="shrink-0 px-5 py-3">
            <Button type="button" variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
            <Button type="submit" size="sm" disabled={!selected || isLoading || switchBranch.isPending || fetchRemote.isPending}>
              {switchBranch.isPending ? "Switching…" : "Switch & Update"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
