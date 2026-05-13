import * as React from "react";
import { Plus, Pencil, Trash2, ArrowRightLeft, Check, FolderOpen } from "lucide-react";

import {
  Sheet,
  SheetContent,
  SheetHeader,
  SheetTitle,
  SheetDescription,
} from "@/components/ui/sheet";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
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
        "flex items-center gap-2 rounded-md border bg-card px-3 py-2.5 transition-colors",
        isActive ? "border-primary/40 bg-primary/5" : "border-border"
      )}
    >
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <span className="text-sm font-medium truncate">{project.name}</span>
          {isActive && (
            <span className="inline-flex items-center gap-1 rounded-sm bg-primary/15 text-primary px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wide">
              <Check className="h-2.5 w-2.5" /> Active
            </span>
          )}
        </div>
        <div className="font-mono text-[11px] text-muted-foreground truncate mt-0.5">
          {project.project_path}
        </div>
        {(project.db_context || project.startup_project) && (
          <div className="font-mono text-[10px] text-muted-foreground/80 truncate mt-0.5">
            {project.db_context && <span>ctx: {project.db_context}</span>}
            {project.db_context && project.startup_project && <span className="px-1">·</span>}
            {project.startup_project && <span>startup: {project.startup_project.split("/").pop()}</span>}
          </div>
        )}
      </div>
      <div className="flex items-center gap-0.5 shrink-0">
        {!isActive && (
          <Button size="icon-sm" variant="ghost" title="Switch to project" onClick={onSwitch}>
            <ArrowRightLeft className="h-3.5 w-3.5" />
          </Button>
        )}
        <Button size="icon-sm" variant="ghost" title="Edit" onClick={onEdit}>
          <Pencil className="h-3.5 w-3.5" />
        </Button>
        <Button
          size="icon-sm"
          variant="ghost"
          className="text-destructive hover:text-destructive hover:bg-destructive/10"
          title="Delete"
          onClick={onDelete}
        >
          <Trash2 className="h-3.5 w-3.5" />
        </Button>
      </div>
    </div>
  );
}

function PreferenceRow({ label, description, checked, onChange }) {
  return (
    <div className="flex items-start justify-between gap-3 rounded-md border border-border bg-card px-3 py-3">
      <div className="min-w-0">
        <div className="text-sm font-medium">{label}</div>
        <div className="text-xs text-muted-foreground mt-0.5 leading-relaxed">{description}</div>
      </div>
      <Switch checked={checked} onCheckedChange={onChange} />
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
      <SheetContent side="right" className="w-[460px] sm:max-w-[460px] p-0 flex flex-col">
        <SheetHeader className="border-b border-border pb-3">
          <SheetTitle>Settings</SheetTitle>
          <SheetDescription>Manage saved projects and preferences.</SheetDescription>
        </SheetHeader>

        <div className="flex-1 min-h-0 overflow-y-auto">
          <div className="px-5 py-4 flex flex-col gap-5">
            <section className="flex flex-col gap-2">
              <div className="flex items-center justify-between">
                <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                  Saved projects
                </h3>
                <Button size="sm" onClick={() => ui.openDialog("addProject")}>
                  <Plus className="h-3.5 w-3.5" />
                  Add project
                </Button>
              </div>

              {savedProjects.length === 0 ? (
                <button
                  type="button"
                  onClick={() => ui.openDialog("addProject")}
                  className="group flex flex-col items-center justify-center gap-2 rounded-md border border-dashed border-border bg-card/40 px-4 py-8 text-center transition-colors hover:border-primary/40 hover:bg-primary/5"
                >
                  <FolderOpen className="h-6 w-6 text-muted-foreground group-hover:text-primary transition-colors" />
                  <div className="text-sm font-medium">No saved projects yet</div>
                  <div className="text-xs text-muted-foreground max-w-[280px]">
                    Add an EF Core data project to switch between them quickly. Click anywhere here to start.
                  </div>
                </button>
              ) : (
                <div className="flex flex-col gap-2">
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

            <section className="flex flex-col gap-2">
              <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Preferences
              </h3>
              <PreferenceRow
                label="Notify on branch change"
                description="Show a prompt to update the database after switching git branches."
                checked={notify}
                onChange={(v) =>
                  setPrefs.mutate({ ...(preferences || {}), notify_on_branch_change: v })
                }
              />
            </section>
          </div>
        </div>
      </SheetContent>
    </Sheet>
  );
}
