import * as React from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Settings, RefreshCw, HelpCircle, GitBranch } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui/tooltip";
import { useMigrations, useCurrentBranch, queryKeys } from "@/lib/queries";
import { useUI } from "@/lib/ui-store";
import { cn } from "@/lib/utils";

function TooltipIconButton({ tooltip, onClick, children }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button size="icon-sm" variant="ghost" onClick={onClick}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{tooltip}</TooltipContent>
    </Tooltip>
  );
}

export function Header({ project }) {
  const ui = useUI();
  const qc = useQueryClient();
  const enabled = Boolean(project);
  const { data: migrations = [], isError } = useMigrations({ enabled });
  const { data: currentBranch = "" } = useCurrentBranch();

  const pending = migrations.filter((m) => !m.applied).length;
  const dbConnected = enabled && !isError;
  const branch = currentBranch || project?.branch || "";

  const refresh = () => {
    qc.invalidateQueries({ queryKey: queryKeys.migrations });
    qc.invalidateQueries({ queryKey: queryKeys.currentBranch });
  };

  return (
    <header
      className="flex items-center justify-between px-4 h-10 border-b border-border bg-card/60 select-none"
      style={{ ["WebkitAppRegion"]: "drag" }}
    >
      <div className="flex items-center gap-2 pl-16">
        <h1 className="text-sm font-semibold tracking-tight">ezMigrations</h1>
        <span className="text-[10px] text-muted-foreground font-normal">v0.6.0</span>
        <span
          className={cn(
            "h-2 w-2 rounded-full transition-colors",
            !enabled
              ? "bg-muted-foreground"
              : dbConnected
              ? "bg-emerald-500"
              : "bg-destructive"
          )}
          title={dbConnected ? "Connected" : "Connection error"}
        />
      </div>

      <div
        className="flex items-center gap-1.5"
        style={{ ["WebkitAppRegion"]: "no-drag" }}
      >
        {pending > 0 && (
          <Badge size="sm" className="h-5 border-yellow-500/40 bg-yellow-500/10 text-yellow-300">
            {pending} pending
          </Badge>
        )}
        {branch && (
          <Badge variant="default" size="sm" className="h-5 font-mono">
            <GitBranch className="h-2.5 w-2.5" />
            {branch}
          </Badge>
        )}
        <TooltipIconButton tooltip="Keyboard shortcuts (?)" onClick={() => ui.setHotkeysOpen(true)}>
          <HelpCircle className="h-3.5 w-3.5" />
        </TooltipIconButton>
        <TooltipIconButton tooltip="Settings" onClick={() => ui.setSettingsOpen(true)}>
          <Settings className="h-3.5 w-3.5" />
        </TooltipIconButton>
        <TooltipIconButton tooltip="Refresh (⌘R)" onClick={refresh}>
          <RefreshCw className="h-3.5 w-3.5" />
        </TooltipIconButton>
      </div>
    </header>
  );
}
