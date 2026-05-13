import * as React from "react";
import { Plus, Layers, Database, GitBranch, FolderOpen, Search } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { useUI } from "@/lib/ui-store";
import { useUpdateDatabase } from "@/lib/mutations";

export function MigrationsToolbar() {
  const ui = useUI();
  const updateDb = useUpdateDatabase();

  return (
    <div className="flex items-center gap-1.5 px-3 py-1.5 border-b border-border bg-background/40">
      <Button
        size="xs"
        onClick={() => ui.openDialog("newMigration")}
        className="bg-emerald-600 hover:bg-emerald-600/90 text-white"
      >
        <Plus className="h-3 w-3" /> New
      </Button>
      <Button
        size="xs"
        variant="secondary"
        onClick={() => ui.openDialog("squash")}
        disabled={ui.checked.size < 2}
      >
        <Layers className="h-3 w-3" /> Squash
      </Button>
      <Button size="xs" onClick={() => updateDb.mutate({})} disabled={updateDb.isPending}>
        <Database className="h-3 w-3" /> {updateDb.isPending ? "Updating…" : "Update DB"}
      </Button>
      <Button
        size="xs"
        variant="ghost"
        onClick={() => ui.openDialog("switchBranch")}
      >
        <GitBranch className="h-3 w-3" /> Switch Branch
      </Button>

      <div className="relative ml-1.5 flex-1 max-w-[280px]">
        <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground" />
        <Input
          value={ui.searchQuery}
          onChange={(e) => ui.setSearchQuery(e.target.value)}
          placeholder="Filter migrations..."
          className="h-7 pl-7 text-[11px]"
          data-search-input
        />
      </div>

      <Button size="xs" variant="ghost" className="ml-auto" onClick={() => ui.openDialog("changeProject")}>
        <FolderOpen className="h-3 w-3" /> Change Project
      </Button>
    </div>
  );
}
