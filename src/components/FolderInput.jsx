import * as React from "react";
import { FolderOpen } from "lucide-react";

import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { openFolderDialog } from "@/lib/tauri";

export function FolderInput({ id, value, onChange, placeholder }) {
  return (
    <div className="flex gap-1.5">
      <Input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className="font-mono text-xs"
      />
      <Button
        type="button"
        variant="outline"
        size="sm"
        className="h-9 shrink-0"
        onClick={async () => {
          const picked = await openFolderDialog();
          if (picked) onChange(picked);
        }}
      >
        <FolderOpen className="h-3.5 w-3.5" /> Browse
      </Button>
    </div>
  );
}
