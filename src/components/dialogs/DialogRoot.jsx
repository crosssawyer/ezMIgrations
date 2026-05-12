import * as React from "react";
import { useUI } from "@/lib/ui-store";
import { NewMigrationDialog } from "./NewMigrationDialog";
import { SquashDialog } from "./SquashDialog";
import { ForceRemoveDialog } from "./ForceRemoveDialog";
import { ProjectDialog } from "./ProjectDialog";
import { ChangeProjectDialog } from "./ChangeProjectDialog";
import { SwitchBranchDialog } from "./SwitchBranchDialog";
import { BranchChangedDialog } from "./BranchChangedDialog";

const REGISTRY = {
  newMigration: NewMigrationDialog,
  squash: SquashDialog,
  forceRemove: ForceRemoveDialog,
  addProject: ProjectDialog,
  editProject: ProjectDialog,
  changeProject: ChangeProjectDialog,
  switchBranch: SwitchBranchDialog,
  branchChanged: BranchChangedDialog,
};

export function DialogRoot() {
  const { dialog, closeDialog } = useUI();
  if (!dialog) return null;
  const Component = REGISTRY[dialog.type];
  if (!Component) return null;
  return <Component {...dialog.props} mode={dialog.type} onClose={closeDialog} />;
}
