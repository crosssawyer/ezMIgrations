import * as React from "react";
import { Plus, Pencil, Trash2, ArrowRightLeft, Check } from "lucide-react";

import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { useUI } from "@/lib/ui-store";
import { useSavedProjects, usePreferences, useProject } from "@/lib/queries";
import {
  useDeleteSavedProject,
  useSwitchProject,
  useSetPreferences,
} from "@/lib/mutations";
import { cn } from "@/lib/utils";

function SavedProjectRow({ project, isActive, onSwitch, onEdit, onDelete }) {
  return (
    <div
      className={cn(
        "flex items-center gap-2 rounded-md border border-border bg-card px-3 py-2",
        isActive && "border-primary/40 bg-primary/5"
      )}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="text-xs font-medium truncate">{project.name}</span>
          {isActive && <Check className="h-3 w-3 text-primary" />}
        </div>
        <div className="font-mono text-[10px] text-muted-foreground truncate">{project.project_path}</div>
      </div>
      {!isActive && (
        <Button size="icon-sm" variant="ghost" title="Switch to project" onClick={onSwitch}>
          <ArrowRightLeft className="h-3 w-3" />
        </Button>
      )}
      <Button size="icon-sm" variant="ghost" title="Edit" onClick={onEdit}>
        <Pencil className="h-3 w-3" />
      </Button>
      <Button
        size="icon-sm"
        variant="ghost"
        className="text-destructive hover:text-destructive"
        title="Delete"
        onClick={onDelete}
      >
        <Trash2 className="h-3 w-3" />
      </Button>
    </div>
  );
}

export function SettingsSheet() {
  const ui = useUI();
  const { data: savedProjects = [] } = useSavedProjects();
  const { data: currentProject } = useProject();
  const { data: preferences } = usePreferences();
  const switchProject = useSwitchProject();
  const deleteProject = useDeleteSavedProject();
  const setPrefs = useSetPreferences();

  const notify = preferences?.notify_on_branch_change ?? true;

  return (
    <Sheet open={ui.settingsOpen} onOpenChange={ui.setSettingsOpen}>
      <SheetContent side="right" className="w-[420px] sm:max-w-[420px] p-0">
        <SheetHeader>
          <SheetTitle>Settings</SheetTitle>
          <SheetDescription>Manage saved projects and preferences.</SheetDescription>
        </SheetHeader>
        <ScrollArea className="h-[calc(100vh-72px)] mt-2">
          <div className="px-5 pb-5 flex flex-col gap-4">
            <section>
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">Saved projects</h3>
                <Button size="xs" onClick={() => ui.openDialog("addProject")}>
                  <Plus className="h-3 w-3" /> Add
                </Button>
              </div>
              {savedProjects.length === 0 ? (
                <p className="text-xs text-muted-foreground py-3">No saved projects yet. Add one to switch quickly.</p>
              ) : (
                <div className="flex flex-col gap-1.5">
                  {savedProjects.map((p) => (
                    <SavedProjectRow
                      key={p.id}
                      project={p}
                      isActive={currentProject?.id === p.id}
                      onSwitch={() => switchProject.mutate({ id: p.id })}
                      onEdit={() => ui.openDialog("editProject", { project: p })}
                      onDelete={() => deleteProject.mutate(p.id)}
                    />
                  ))}
                </div>
              )}
            </section>

            <Separator />

            <section>
              <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground mb-2">Preferences</h3>
              <div className="flex items-start justify-between gap-3 rounded-md border border-border bg-card px-3 py-2.5">
                <div>
                  <div className="text-xs font-medium">Notify on branch change</div>
                  <div className="text-[10px] text-muted-foreground mt-0.5">
                    Show a prompt to update the database after switching git branches.
                  </div>
                </div>
                <Switch
                  checked={notify}
                  onCheckedChange={(v) =>
                    setPrefs.mutate({ ...(preferences || {}), notify_on_branch_change: v })
                  }
                />
              </div>
            </section>
          </div>
        </ScrollArea>
      </SheetContent>
    </Sheet>
  );
}
