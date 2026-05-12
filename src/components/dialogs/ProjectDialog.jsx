import * as React from "react";
import { useForm } from "@tanstack/react-form";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { FolderInput } from "@/components/FolderInput";
import { useSaveProject, useUpdateSavedProject } from "@/lib/mutations";

export function ProjectDialog({ onClose, mode, project }) {
  const isEdit = mode === "editProject";
  const save = useSaveProject();
  const update = useUpdateSavedProject();

  const form = useForm({
    defaultValues: {
      id: project?.id || null,
      name: project?.name || "",
      project_path: project?.project_path || "",
      db_context: project?.db_context || "",
      startup_project: project?.startup_project || "",
    },
    onSubmit: async ({ value }) => {
      if (!value.name.trim() || !value.project_path.trim()) return;
      const payload = {
        ...value,
        name: value.name.trim(),
        project_path: value.project_path.trim(),
        db_context: value.db_context.trim(),
        startup_project: value.startup_project.trim(),
      };
      if (isEdit) await update.mutateAsync(payload);
      else await save.mutateAsync(payload);
      onClose();
    },
  });

  const pending = save.isPending || update.isPending;

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-md p-0 gap-0">
        <DialogHeader className="px-5 pt-5 pb-2">
          <DialogTitle>{isEdit ? "Edit project" : "Add project"}</DialogTitle>
          <DialogDescription>
            Save project paths so you can swap between EF Core data projects from settings.
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={(e) => { e.preventDefault(); form.handleSubmit(); }}>
          <div className="px-5 py-3 flex flex-col gap-3">
            <form.Field name="name">
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name}>Project name</Label>
                  <Input
                    id={field.name}
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    placeholder="My Project"
                  />
                </div>
              )}
            </form.Field>
            <form.Field name="project_path">
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name}>
                    Migrations Project <span className="ml-1 text-muted-foreground font-normal">(contains DbContext)</span>
                  </Label>
                  <FolderInput
                    id={field.name}
                    value={field.state.value}
                    onChange={(v) => field.handleChange(v)}
                    placeholder="/path/to/solution/MyApp.Data"
                  />
                </div>
              )}
            </form.Field>
            <form.Field name="db_context">
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name}>DbContext name <span className="text-muted-foreground font-normal">(optional)</span></Label>
                  <Input
                    id={field.name}
                    value={field.state.value}
                    onChange={(e) => field.handleChange(e.target.value)}
                    placeholder="ApplicationDbContext"
                    className="font-mono text-xs"
                  />
                </div>
              )}
            </form.Field>
            <form.Field name="startup_project">
              {(field) => (
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor={field.name}>
                    Startup project <span className="text-muted-foreground font-normal">(optional)</span>
                  </Label>
                  <FolderInput
                    id={field.name}
                    value={field.state.value}
                    onChange={(v) => field.handleChange(v)}
                    placeholder="/path/to/solution/MyApp.Api"
                  />
                </div>
              )}
            </form.Field>
          </div>
          <DialogFooter className="px-5 py-3">
            <Button type="button" variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
            <Button type="submit" size="sm" disabled={pending}>
              {pending ? "Saving…" : isEdit ? "Save changes" : "Add project"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
