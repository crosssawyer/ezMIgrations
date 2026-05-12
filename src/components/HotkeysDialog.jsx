import * as React from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useUI } from "@/lib/ui-store";

const SHORTCUTS = [
  ["⌘ N", "New migration"],
  ["⌘ R", "Refresh migrations"],
  ["⌘ F", "Focus search / filter"],
  ["Esc", "Close panel / dialog"],
  ["?", "Toggle this help"],
];

export function HotkeysDialog() {
  const { hotkeysOpen, setHotkeysOpen } = useUI();
  return (
    <Dialog open={hotkeysOpen} onOpenChange={setHotkeysOpen}>
      <DialogContent className="max-w-sm">
        <DialogHeader>
          <DialogTitle>Keyboard Shortcuts</DialogTitle>
        </DialogHeader>
        <div className="px-5 pb-5 flex flex-col gap-1">
          {SHORTCUTS.map(([keys, label]) => (
            <div key={keys} className="flex items-center justify-between py-1.5 border-b border-border last:border-0">
              <span className="text-xs text-muted-foreground">{label}</span>
              <kbd className="font-mono text-[10px] px-1.5 py-0.5 rounded border border-border bg-muted">
                {keys}
              </kbd>
            </div>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  );
}
