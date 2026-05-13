import * as React from "react";
import { useUI } from "@/lib/ui-store";

const LazyProjectDialog = React.lazy(() =>
  import("./ProjectDialog").then((m) => ({ default: m.ProjectDialog }))
);

const REGISTRY = {
  newMigration: React.lazy(() =>
    import("./NewMigrationDialog").then((m) => ({ default: m.NewMigrationDialog }))
  ),
  squash: React.lazy(() =>
    import("./SquashDialog").then((m) => ({ default: m.SquashDialog }))
  ),
  forceRemove: React.lazy(() =>
    import("./ForceRemoveDialog").then((m) => ({ default: m.ForceRemoveDialog }))
  ),
  addProject: LazyProjectDialog,
  editProject: LazyProjectDialog,
  changeProject: React.lazy(() =>
    import("./ChangeProjectDialog").then((m) => ({ default: m.ChangeProjectDialog }))
  ),
  switchBranch: React.lazy(() =>
    import("./SwitchBranchDialog").then((m) => ({ default: m.SwitchBranchDialog }))
  ),
  branchChanged: React.lazy(() =>
    import("./BranchChangedDialog").then((m) => ({ default: m.BranchChangedDialog }))
  ),
};

export function DialogRoot() {
  const { dialog, closeDialog } = useUI();
  if (!dialog) return null;
  const Component = REGISTRY[dialog.type];
  if (!Component) return null;
  return (
    <React.Suspense fallback={null}>
      <Component {...dialog.props} mode={dialog.type} onClose={closeDialog} />
    </React.Suspense>
  );
}
