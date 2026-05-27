import * as React from "react";
import { useForm } from "@tanstack/react-form";
import { CheckCircle2, XCircle, Loader2 } from "lucide-react";

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
import { Badge } from "@/components/ui/badge";
import { FolderInput } from "@/components/FolderInput";
import {
  useSaveProject,
  useUpdateSavedProject,
  useSetDbConnection,
  useClearDbConnection,
  useTestDbConnection,
} from "@/lib/mutations";
import { useHasDbConnection } from "@/lib/queries";

export function ProjectDialog({ onClose, mode, project }) {
  const isEdit = mode === "editProject";
  const save = useSaveProject();
  const update = useUpdateSavedProject();
  const setDbConn = useSetDbConnection();
  const clearDbConn = useClearDbConnection();

  const hasDbConn = useHasDbConnection(project?.id, { enabled: isEdit });

  const form = useForm({
    defaultValues: {
      id: project?.id || null,
      name: project?.name || "",
      project_path: project?.project_path || "",
      db_context: project?.db_context || "",
      startup_project: project?.startup_project || "",
      db_connection_string: "",
    },
    onSubmit: async ({ value }) => {
      if (!value.name.trim() || !value.project_path.trim()) return;

      const { db_connection_string, ...rest } = value;
      const payload = {
        ...rest,
        name: value.name.trim(),
        project_path: value.project_path.trim(),
        db_context: value.db_context.trim(),
        startup_project: value.startup_project.trim(),
      };

      const saved = isEdit
        ? await update.mutateAsync(payload)
        : await save.mutateAsync(payload);

      const trimmedConn = db_connection_string.trim();
      if (trimmedConn) {
        // Best-effort: surface the project save even if keyring write fails.
        try {
          await setDbConn.mutateAsync({
            projectId: saved.id,
            connectionString: trimmedConn,
          });
        } catch {
          /* mutation already toasts the error */
        }
      }

      onClose();
    },
  });

  const pending = save.isPending || update.isPending || setDbConn.isPending;

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

            <DbConnectionSection
              form={form}
              isEdit={isEdit}
              projectId={project?.id}
              configured={Boolean(hasDbConn.data)}
              clearDbConn={clearDbConn}
            />
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

function DbConnectionSection({ form, isEdit, projectId, configured, clearDbConn }) {
  const test = useTestDbConnection();
  // Tri-state: null = no probe yet, true = OK, string = error message
  const [probeResult, setProbeResult] = React.useState(null);

  async function handleTest(value) {
    const trimmed = value.trim();
    if (!trimmed) return;
    setProbeResult(null);
    try {
      await test.mutateAsync({ connectionString: trimmed });
      setProbeResult(true);
    } catch (err) {
      setProbeResult(typeof err === "string" ? err : err?.message || "Connection failed");
    }
  }

  async function handleClear() {
    if (!projectId) return;
    await clearDbConn.mutateAsync({ projectId });
    setProbeResult(null);
  }

  return (
    <div className="flex flex-col gap-1.5 border-t pt-3 mt-1">
      <div className="flex items-center justify-between">
        <Label htmlFor="db_connection_string">
          Database connection <span className="text-muted-foreground font-normal">(optional)</span>
        </Label>
        {isEdit && configured && (
          <Badge variant="secondary" className="text-[10px] font-normal">
            Configured
          </Badge>
        )}
      </div>
      <p className="text-xs text-muted-foreground">
        Used only to read <code className="text-[10px]">__EFMigrationsHistory</code>. Stored in your OS keyring, never in the config file.
      </p>
      <form.Field name="db_connection_string">
        {(field) => (
          <>
            <Input
              id="db_connection_string"
              type="password"
              value={field.state.value}
              onChange={(e) => {
                field.handleChange(e.target.value);
                setProbeResult(null);
              }}
              placeholder={
                isEdit && configured
                  ? "Leave blank to keep existing connection"
                  : "Server=…;Database=…;User Id=…;Password=…"
              }
              className="font-mono text-xs"
            />
            <div className="flex items-center justify-between gap-2 min-h-[24px]">
              <ProbeStatus state={probeResult} pending={test.isPending} />
              <div className="flex items-center gap-2">
                {isEdit && configured && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={handleClear}
                    disabled={clearDbConn.isPending}
                  >
                    Clear
                  </Button>
                )}
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-7 text-xs"
                  onClick={() => handleTest(field.state.value)}
                  disabled={!field.state.value.trim() || test.isPending}
                >
                  Test connection
                </Button>
              </div>
            </div>
          </>
        )}
      </form.Field>
    </div>
  );
}

function ProbeStatus({ state, pending }) {
  if (pending) {
    return (
      <span className="flex items-center gap-1 text-xs text-muted-foreground">
        <Loader2 className="h-3 w-3 animate-spin" />
        Connecting…
      </span>
    );
  }
  if (state === true) {
    return (
      <span className="flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
        <CheckCircle2 className="h-3 w-3" />
        Connection works
      </span>
    );
  }
  if (typeof state === "string") {
    return (
      <span
        className="flex items-center gap-1 text-xs text-destructive truncate"
        title={state}
      >
        <XCircle className="h-3 w-3 flex-shrink-0" />
        <span className="truncate">{state}</span>
      </span>
    );
  }
  return <span />;
}
