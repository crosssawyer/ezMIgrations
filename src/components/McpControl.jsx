import * as React from "react";
import { Box, Check, ChevronDown, Copy, Power, SquareTerminal } from "lucide-react";

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { toast } from "@/lib/toast";
import { useMcpStatus } from "@/lib/queries";
import {
  useOpenMcpTerminal,
  useStartMcpServer,
  useStopMcpServer,
} from "@/lib/mutations";
import { cn } from "@/lib/utils";

function platformKey() {
  const ua = navigator.userAgent.toLowerCase();
  if (ua.includes("windows")) return "windows";
  if (ua.includes("mac")) return "mac";
  return "linux";
}

const TERMINALS = {
  mac: [
    { id: "system", label: "Terminal" },
    { id: "iterm", label: "iTerm2" },
  ],
  windows: [
    { id: "system", label: "Windows Terminal" },
    { id: "powershell", label: "PowerShell" },
    { id: "cmd", label: "Command Prompt" },
  ],
  linux: [
    { id: "system", label: "System terminal" },
    { id: "gnome", label: "GNOME Terminal" },
    { id: "konsole", label: "Konsole" },
    { id: "xterm", label: "xterm" },
  ],
};

export function McpControl({ project }) {
  const { data: status, isLoading, isError } = useMcpStatus();
  const start = useStartMcpServer();
  const stop = useStopMcpServer();
  const open = useOpenMcpTerminal();

  const terminals = React.useMemo(() => TERMINALS[platformKey()] ?? TERMINALS.linux, []);
  const running = Boolean(status?.running);
  const busy = start.isPending || stop.isPending || open.isPending;
  const canOpen = Boolean(project) && !busy;
  const statusText = isError ? "Error" : isLoading ? "..." : running ? "Up" : "Off";
  const statusColor = isError
    ? "bg-destructive"
    : running
    ? "bg-emerald-500"
    : "bg-muted-foreground";

  const openTerminal = (terminal = terminals[0]?.id ?? "system") => {
    if (!project) {
      toast.warning("Connect a project before opening an MCP terminal.");
      return;
    }
    open.mutate({ terminal });
  };

  const toggleServer = () => {
    if (running) stop.mutate();
    else start.mutate();
  };

  const copyUrl = async () => {
    if (!status?.url) return;
    try {
      await navigator.clipboard.writeText(status.url);
      toast.success("MCP URL copied");
    } catch (err) {
      toast.error(`Failed to copy MCP URL: ${err}`);
    }
  };

  return (
    <div className="inline-flex h-7 overflow-hidden rounded-md border border-border bg-secondary/60">
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            className="inline-flex h-7 items-center gap-1.5 px-2 text-xs font-medium transition-colors hover:bg-accent disabled:opacity-50"
            onClick={() => openTerminal()}
            disabled={!canOpen}
            aria-label={`Open MCP terminal (${statusText})`}
          >
            <Box className="h-3.5 w-3.5" />
            <span>MCP</span>
            <span className={cn("h-1.5 w-1.5 rounded-full", statusColor)} />
            <span className="text-[10px] text-muted-foreground">{statusText}</span>
          </button>
        </TooltipTrigger>
        <TooltipContent>
          {project ? "Open a repo terminal with ezMigrations MCP loaded" : "Connect a project first"}
        </TooltipContent>
      </Tooltip>

      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            className="inline-flex h-7 w-7 items-center justify-center border-l border-border transition-colors hover:bg-accent disabled:opacity-50"
            disabled={busy}
            aria-label="MCP menu"
          >
            <ChevronDown className="h-3.5 w-3.5" />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-64">
          <DropdownMenuLabel className="flex flex-col gap-1">
            <span className="flex items-center justify-between gap-2">
              <span className="flex items-center gap-2">
                <span className={cn("h-2 w-2 rounded-full", statusColor)} />
                MCP server
              </span>
              <span className="text-[10px] font-normal text-muted-foreground">{statusText}</span>
            </span>
            {status?.url ? (
              <span className="truncate font-mono text-[10px] font-normal text-muted-foreground">
                {status.url}
              </span>
            ) : null}
          </DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={toggleServer} disabled={busy}>
            <Power className="h-3.5 w-3.5" />
            {running ? "Turn off server" : "Turn on server"}
          </DropdownMenuItem>
          <DropdownMenuItem onSelect={copyUrl} disabled={!status?.url}>
            <Copy className="h-3.5 w-3.5" />
            Copy MCP URL
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuLabel className="text-[10px] font-semibold uppercase tracking-wide text-muted-foreground">
            Open terminal
          </DropdownMenuLabel>
          {terminals.map((terminal, index) => (
            <DropdownMenuItem
              key={terminal.id}
              onSelect={() => openTerminal(terminal.id)}
              disabled={!project || busy}
            >
              <SquareTerminal className="h-3.5 w-3.5" />
              {terminal.label}
              {index === 0 ? <Check className="ml-auto h-3 w-3 text-muted-foreground" /> : null}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
