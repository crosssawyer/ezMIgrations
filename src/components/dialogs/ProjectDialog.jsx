import * as React from "react";
import { useForm } from "@tanstack/react-form";
import { CheckCircle2, XCircle, Loader2, Database } from "lucide-react";

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
import {
  useSaveProject,
  useUpdateSavedProject,
  useSetDbConnection,
  useClearDbConnection,
  useTestDbConnection,
} from "@/lib/mutations";
import { useHasDbConnection } from "@/lib/queries";
import { invoke } from "@/lib/tauri";

function errorMessage(err, fallback) {
  if (typeof err === "string") return err;
  if (err && typeof err.message === "string") return err.message;
  return fallback;
}

export function ProjectDialog({ onClose, mode, project }) {
  const isEdit = mode === "editProject";
  const save = useSaveProject();
  const update = useUpdateSavedProject();
  const setDbConn = useSetDbConnection();
  const clearDbConn = useClearDbConnection();

  const hasDbConn = useHasDbConnection(project?.id, { enabled: isEdit });

  // Captured after a successful save so we can keep the dialog open with a
  // visible "Saved" state instead of silently closing.
  const [saveStatus, setSaveStatus] = React.useState(null);

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

      // Project save — wrap so a backend failure renders inline instead of
      // throwing through react-form (which swallows the error silently).
      let saved;
      try {
        saved = isEdit
          ? await update.mutateAsync(payload)
          : await save.mutateAsync(payload);
      } catch (err) {
        const message = errorMessage(err, "Project save failed");
        setSaveStatus({ projectError: message, dbStatus: null, ignored_keys: [] });
        return;
      }

      const trimmedConn = db_connection_string.trim();
      let ignored_keys = [];
      let dbStatus = null;
      if (trimmedConn) {
        try {
          ignored_keys = await setDbConn.mutateAsync({
            projectId: saved.id,
            connectionString: trimmedConn,
          });
          // Verification read: confirm the value actually round-trips. On macOS,
          // an unsigned-binary keychain prompt can be denied, so a "successful"
          // write that subsequent reads can't see is a real possibility.
          const verified = await invoke("has_db_connection", { projectId: saved.id });
          dbStatus = verified
            ? "saved"
            : "Saved to keyring, but read-back returned no entry. Your OS keyring may have denied access — check for any prompts and try again.";
        } catch (err) {
          dbStatus = errorMessage(err, "Connection save failed");
        }
      }

      setSaveStatus({ projectError: null, dbStatus, ignored_keys });
    },
  });

  const pending = save.isPending || update.isPending || setDbConn.isPending;
  const saved = Boolean(saveStatus);

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
              checkError={hasDbConn.isError ? errorMessage(hasDbConn.error, "Couldn't read keyring") : null}
              clearDbConn={clearDbConn}
            />

            {saveStatus && <SavePostStatus status={saveStatus} />}
          </div>
          <DialogFooter className="px-5 py-3">
            {saved ? (
              <Button type="button" size="sm" onClick={onClose}>
                Done
              </Button>
            ) : (
              <>
                <Button type="button" variant="ghost" size="sm" onClick={onClose}>
                  Cancel
                </Button>
                <Button type="submit" size="sm" disabled={pending}>
                  {pending ? "Saving…" : isEdit ? "Save changes" : "Add project"}
                </Button>
              </>
            )}
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function DbConnectionSection({ form, isEdit, projectId, configured, checkError, clearDbConn }) {
  const test = useTestDbConnection();
  // null = no probe yet; { ok: true, ignored } on success; { ok: false, error } on failure
  const [probeResult, setProbeResult] = React.useState(null);

  async function handleTest(value) {
    const trimmed = value.trim();
    if (!trimmed) return;
    setProbeResult(null);
    try {
      const ignored = await test.mutateAsync({ connectionString: trimmed });
      setProbeResult({ ok: true, ignored: ignored || [] });
    } catch (err) {
      const message = typeof err === "string" ? err : err?.message || "Connection failed";
      setProbeResult({ ok: false, error: message });
    }
  }

  async function handleClear() {
    if (!projectId) return;
    await clearDbConn.mutateAsync({ projectId });
    setProbeResult(null);
  }

  return (
    <div className="flex flex-col gap-1.5 border-t pt-3 mt-1">
      <div className="flex items-center gap-2">
        <Database className="h-3.5 w-3.5 text-muted-foreground" />
        <Label htmlFor="db_connection_string" className="text-sm">
          Database connection <span className="text-muted-foreground font-normal">(optional)</span>
        </Label>
      </div>

      {isEdit && configured && (
        <div className="flex items-center justify-between gap-2 rounded border border-emerald-500/30 bg-emerald-500/5 px-2.5 py-1.5">
          <span className="flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400">
            <CheckCircle2 className="h-3 w-3" />
            Connection saved for this project
          </span>
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-6 text-xs"
            onClick={handleClear}
            disabled={clearDbConn.isPending}
          >
            Remove
          </Button>
        </div>
      )}

      {isEdit && checkError && (
        <div className="rounded border border-amber-500/40 bg-amber-500/5 px-2.5 py-1.5 text-xs text-amber-300 flex items-start gap-1.5">
          <XCircle className="h-3 w-3 mt-0.5 shrink-0" />
          <span className="break-words">
            Couldn't check if a connection is saved: {checkError}
          </span>
        </div>
      )}

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
                  ? "Paste a new connection string to replace…"
                  : "Server=…;Database=…;User Id=…;Password=…"
              }
              className="font-mono text-xs"
            />
            <div className="flex items-center justify-between gap-2 min-h-[24px]">
              <ProbeStatus result={probeResult} pending={test.isPending} />
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
          </>
        )}
      </form.Field>
    </div>
  );
}

function ProbeStatus({ result, pending }) {
  if (pending) {
    return (
      <span className="flex items-center gap-1 text-xs text-muted-foreground">
        <Loader2 className="h-3 w-3 animate-spin" />
        Connecting…
      </span>
    );
  }
  if (!result) return <span />;
  if (result.ok) {
    return (
      <span className="flex items-center gap-1 text-xs text-emerald-600 dark:text-emerald-400">
        <CheckCircle2 className="h-3 w-3" />
        Connection works
        {result.ignored.length > 0 && (
          <span
            className="ml-1 text-muted-foreground"
            title={`These keys aren't recognized by the SQL Server driver and will be stripped on save: ${result.ignored.join(", ")}`}
          >
            (ignored: {result.ignored.join(", ")})
          </span>
        )}
      </span>
    );
  }
  return (
    <span
      className="flex items-center gap-1 text-xs text-destructive truncate"
      title={result.error}
    >
      <XCircle className="h-3 w-3 flex-shrink-0" />
      <span className="truncate">{result.error}</span>
    </span>
  );
}

function SavePostStatus({ status }) {
  const { projectError, dbStatus, ignored_keys } = status;

  if (projectError) {
    return (
      <div className="rounded border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs flex items-start gap-2">
        <XCircle className="h-3.5 w-3.5 text-destructive mt-0.5" />
        <div className="flex flex-col gap-0.5 min-w-0">
          <span className="text-destructive font-medium">Project save failed</span>
          <span className="text-muted-foreground break-words">{projectError}</span>
        </div>
      </div>
    );
  }

  if (dbStatus === "saved") {
    return (
      <div className="rounded border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs flex items-start gap-2">
        <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400 mt-0.5" />
        <div className="flex flex-col gap-0.5">
          <span className="text-emerald-700 dark:text-emerald-400 font-medium">
            Project saved · Database connection stored
          </span>
          {ignored_keys.length > 0 && (
            <span className="text-muted-foreground">
              Ignored unsupported keys: <code className="text-[10px]">{ignored_keys.join(", ")}</code>
            </span>
          )}
        </div>
      </div>
    );
  }

  if (typeof dbStatus === "string" && dbStatus !== "saved") {
    return (
      <div className="rounded border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs flex items-start gap-2">
        <XCircle className="h-3.5 w-3.5 text-destructive mt-0.5" />
        <div className="flex flex-col gap-0.5 min-w-0">
          <span className="text-destructive font-medium">
            Project saved, but the database connection couldn't be stored
          </span>
          <span className="text-muted-foreground break-words">{dbStatus}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="rounded border border-emerald-500/30 bg-emerald-500/5 px-3 py-2 text-xs flex items-center gap-2">
      <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
      <span className="text-emerald-700 dark:text-emerald-400 font-medium">Project saved</span>
    </div>
  );
}
