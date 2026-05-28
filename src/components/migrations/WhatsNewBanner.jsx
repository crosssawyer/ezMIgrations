import * as React from "react";
import { Sparkles, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useProject, useHasDbConnection, useSavedProjects } from "@/lib/queries";
import { useUI } from "@/lib/ui-store";
import { compareVersions, getSeenVersion, markSeen } from "@/lib/seen-version";

// Defined by vite.config.js → define.__APP_VERSION__ from package.json
// eslint-disable-next-line no-undef
const APP_VERSION = __APP_VERSION__;

// The version that introduced the DB-history feature. Users on this version
// or later see the hint at most once until they configure a connection (which
// makes the hint disappear automatically) or click Dismiss.
const FEATURE_VERSION = "1.2.0";

export function WhatsNewBanner() {
  const { data: project } = useProject();
  const { data: savedProjects = [] } = useSavedProjects();
  const { data: dbConfigured } = useHasDbConnection(project?.id);
  const ui = useUI();

  const [seen, setSeen] = React.useState(() => getSeenVersion());

  const isUnseen = compareVersions(seen, FEATURE_VERSION) < 0;
  const isOnNewVersion = compareVersions(APP_VERSION, FEATURE_VERSION) >= 0;
  // The edit dialog needs the SavedProject shape (project_path, name, …), not
  // the ProjectInfo shape returned by useProject. Look it up by id.
  const savedProject = savedProjects.find((p) => p.id === project?.id);

  if (!isUnseen || !isOnNewVersion || !savedProject || dbConfigured) {
    return null;
  }

  const dismiss = () => {
    markSeen(APP_VERSION);
    setSeen(APP_VERSION);
  };

  return (
    <div className="flex items-center gap-3 px-4 py-2 border-b border-primary/30 bg-primary/5 text-foreground">
      <Sparkles className="h-4 w-4 shrink-0 text-primary" />
      <span className="flex-1 text-xs">
        <strong>New in {FEATURE_VERSION}:</strong> verify migrations against{" "}
        <code className="text-[10px]">__EFMigrationsHistory</code> directly. Add a
        connection string in project settings to enable.
      </span>
      <Button
        size="xs"
        onClick={() => ui.openDialog("editProject", { project: savedProject })}
      >
        Open settings
      </Button>
      <Button
        size="xs"
        variant="ghost"
        onClick={dismiss}
        aria-label="Dismiss"
        title="Dismiss"
      >
        <X className="h-3 w-3" />
      </Button>
    </div>
  );
}
