import * as React from "react";
import { useForm } from "@tanstack/react-form";
import { Folder, Database } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { FolderInput } from "@/components/FolderInput";
import { useSetProject } from "@/lib/mutations";

export function SetupView() {
  const setProject = useSetProject();
  const form = useForm({
    defaultValues: { projectPath: "", dbContext: "", startupProject: "" },
    onSubmit: async ({ value }) => {
      if (!value.projectPath.trim()) return;
      await setProject.mutateAsync({
        projectPath: value.projectPath.trim(),
        dbContext: value.dbContext.trim(),
        startupProject: value.startupProject.trim(),
      });
    },
  });

  return (
    <div className="flex flex-1 items-center justify-center px-6">
      <div className="w-full max-w-md rounded-lg border border-border bg-card p-6 shadow-sm">
        <div className="flex items-center gap-2 mb-4">
          <div className="p-1.5 rounded-md bg-primary/10 text-primary">
            <Database className="h-4 w-4" />
          </div>
          <div>
            <h2 className="text-base font-semibold leading-none">Configure Project</h2>
            <p className="text-xs text-muted-foreground mt-1">
              Point ezMigrations at your EF Core data project to get started.
            </p>
          </div>
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            form.handleSubmit();
          }}
          className="flex flex-col gap-3"
        >
          <form.Field name="projectPath">
            {(field) => (
              <div className="flex flex-col gap-1.5">
                <Label htmlFor={field.name}>
                  Migrations Project
                  <span className="ml-1 text-muted-foreground font-normal">(contains DbContext &amp; migrations)</span>
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
          <form.Field name="dbContext">
            {(field) => (
              <div className="flex flex-col gap-1.5">
                <Label htmlFor={field.name}>
                  DbContext Name <span className="ml-1 text-muted-foreground font-normal">(optional)</span>
                </Label>
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
          <form.Field name="startupProject">
            {(field) => (
              <div className="flex flex-col gap-1.5">
                <Label htmlFor={field.name}>
                  Startup Project
                  <span className="ml-1 text-muted-foreground font-normal">(optional — the executable project)</span>
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
          <Button type="submit" disabled={setProject.isPending} className="mt-2">
            <Folder className="h-3.5 w-3.5" />
            {setProject.isPending ? "Connecting…" : "Connect Project"}
          </Button>
        </form>
      </div>
    </div>
  );
}
