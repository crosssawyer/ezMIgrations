import { toast } from "./toast";
import { parseEfError } from "./parse-ef-error";
import { useUI } from "./ui-store";

/**
 * Routes a mutation error to either the MigrationErrorDialog (when the
 * output is a parseable EF Core / SQL failure) or a plain toast.
 *
 * `messages` is either:
 *   - { title, context, toastPrefix } — same copy for both directions, OR
 *   - { rollback: {...}, apply: {...} } — different copy depending on
 *     whether the failure happened while reverting or applying.
 *
 * Reusable across any mutation that shells out to `dotnet ef`.
 */
export function useEfErrorHandler() {
  const ui = useUI();
  return (err, messages) => {
    const raw = String(err);
    const parsed = parseEfError(raw);
    const variant = /roll back|reverting/i.test(raw) ? "rollback" : "apply";
    const resolved = messages?.[variant] ?? messages?.apply ?? messages ?? {};
    const { title, context, toastPrefix } = resolved;

    if (parsed) {
      ui.openDialog("migrationError", { title, context, error: parsed });
    } else {
      toast.error(toastPrefix ? `${toastPrefix}: ${raw}` : raw);
    }
  };
}
