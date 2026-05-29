import * as React from "react";
import {
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { Eye, Play, Pin, PinOff, Trash2, FileCode2, ArrowUpDown, ArrowUp, ArrowDown } from "lucide-react";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { cn } from "@/lib/utils";
import { useUI } from "@/lib/ui-store";
import { useApplyTo, useRemoveLastOrForce } from "./row-actions";
import { useSetStable } from "@/lib/mutations";

function buildColumns({ checked, setChecked, toggleChecked, setSelectedMigrationId, project, foreignNames, applyTo, isApplying, removeMigration, setStable }) {
  return [
    {
      id: "select",
      enableSorting: false,
      size: 36,
      header: ({ table }) => {
        const all = table.getIsAllRowsSelected();
        const some = table.getIsSomeRowsSelected();
        return (
          <Checkbox
            checked={all ? true : some ? "indeterminate" : false}
            onCheckedChange={(v) => {
              if (v) {
                const next = new Set(table.getRowModel().rows.map((r) => r.original.id));
                setChecked(next);
              } else {
                setChecked(new Set());
              }
            }}
            aria-label="Select all"
          />
        );
      },
      cell: ({ row }) => (
        <Checkbox
          checked={checked.has(row.original.id)}
          onCheckedChange={() => toggleChecked(row.original.id)}
          aria-label={`Select ${row.original.name}`}
        />
      ),
    },
    {
      accessorKey: "name",
      header: ({ column }) => (
        <SortHeader column={column}>Migration</SortHeader>
      ),
      cell: ({ row }) => {
        const m = row.original;
        const isStable = project?.stable_migration === m.name;
        const isForeign = foreignNames.has(m.name);
        return (
          <div className="flex items-center gap-2 min-w-0">
            <button
              onClick={() => setSelectedMigrationId(m.id)}
              className="font-mono text-[11px] text-left hover:text-primary transition-colors truncate"
            >
              {m.name}
            </button>
            {isStable && (
              <Badge variant="primary" size="xs" className="font-semibold uppercase tracking-wider shrink-0">
                <Pin className="h-2.5 w-2.5" /> Stable
              </Badge>
            )}
            {isForeign && (
              <Badge size="xs" className="font-semibold uppercase tracking-wider shrink-0 border-destructive/40 bg-destructive/10 text-destructive">
                Foreign
              </Badge>
            )}
          </div>
        );
      },
    },
    {
      accessorKey: "applied",
      size: 100,
      header: ({ column }) => <SortHeader column={column}>Status</SortHeader>,
      cell: ({ row }) =>
        row.original.applied ? (
          <Badge size="sm" className="border-emerald-500/30 bg-emerald-500/10 text-emerald-300">
            <span className="h-1 w-1 rounded-full bg-emerald-400" /> Applied
          </Badge>
        ) : (
          <Badge variant="muted" size="sm">
            <span className="h-1 w-1 rounded-full bg-muted-foreground" /> Pending
          </Badge>
        ),
    },
    {
      accessorKey: "has_custom_sql",
      size: 56,
      header: "SQL",
      cell: ({ row }) =>
        row.original.has_custom_sql ? (
          <FileCode2 className="h-3.5 w-3.5 text-primary" />
        ) : (
          <span className="text-muted-foreground/40 text-xs">—</span>
        ),
    },
    {
      id: "actions",
      size: 168,
      header: () => <span className="block text-right">Actions</span>,
      cell: ({ row }) => {
        const m = row.original;
        const isStable = project?.stable_migration === m.name;
        return (
          <div className="flex items-center gap-0.5 justify-end opacity-60 group-hover:opacity-100 transition-opacity">
            <Button size="xxs" variant="ghost" onClick={() => setSelectedMigrationId(m.id)} title="View details">
              <Eye className="h-3 w-3" />
            </Button>
            <Button size="xxs" variant="ghost" onClick={() => applyTo(m)} disabled={isApplying} title="Update DB to this migration">
              {isApplying ? <Spinner className="size-3" /> : <Play className="h-3 w-3" />}
            </Button>
            <Button
              size="xxs"
              variant="ghost"
              onClick={() => setStable.mutate({ migrationName: isStable ? null : m.name })}
              title={isStable ? "Unset stable migration" : "Set as stable migration"}
              className={cn(isStable && "text-primary")}
            >
              {isStable ? <PinOff className="h-3 w-3" /> : <Pin className="h-3 w-3" />}
            </Button>
            <Button
              size="xxs"
              variant="ghost"
              onClick={() => removeMigration(m)}
              title="Remove migration"
              className="text-destructive hover:text-destructive"
            >
              <Trash2 className="h-3 w-3" />
            </Button>
          </div>
        );
      },
    },
  ];
}

function SortHeader({ column, children }) {
  const dir = column.getIsSorted();
  const Icon = dir === "asc" ? ArrowUp : dir === "desc" ? ArrowDown : ArrowUpDown;
  return (
    <button
      onClick={() => column.toggleSorting(dir === "asc")}
      className="inline-flex items-center gap-1 hover:text-foreground transition-colors"
    >
      {children}
      <Icon className="h-3 w-3 opacity-60" />
    </button>
  );
}

export function MigrationsTable({ migrations, isLoading, isFetching, project, foreignNames }) {
  const { checked, setChecked, toggleChecked, setSelectedMigrationId, selectedMigrationId, searchQuery, setSearchQuery } = useUI();
  const { applyTo, isApplying } = useApplyTo();
  const removeMigration = useRemoveLastOrForce(migrations);
  const setStable = useSetStable();

  const columns = React.useMemo(
    () => buildColumns({ checked, setChecked, toggleChecked, setSelectedMigrationId, project, foreignNames, applyTo, isApplying, removeMigration, setStable }),
    [checked, setChecked, toggleChecked, setSelectedMigrationId, project, foreignNames, applyTo, isApplying, removeMigration, setStable]
  );

  const table = useReactTable({
    data: migrations,
    columns,
    state: {
      globalFilter: searchQuery,
    },
    onGlobalFilterChange: setSearchQuery,
    globalFilterFn: (row, _columnId, filterValue) => {
      const q = String(filterValue || "").toLowerCase();
      if (!q) return true;
      return row.original.name.toLowerCase().includes(q);
    },
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
    getSortedRowModel: getSortedRowModel(),
  });

  const rows = table.getRowModel().rows;
  const visibleIds = rows.map((r) => r.original.id);
  const visibleKey = visibleIds.join("|");

  // Keyboard navigation: `activeId` is a focus cursor, independent of the
  // detail panel (which `selectedMigrationId` drives). Arrows move the cursor,
  // Enter opens its detail, Space toggles its squash checkbox.
  const gridRef = React.useRef(null);
  const rowRefs = React.useRef(new Map());
  const didAutoFocus = React.useRef(false);
  const [activeId, setActiveId] = React.useState(null);

  // Keep the cursor pointing at a visible row as filtering/sorting changes.
  React.useEffect(() => {
    if (visibleIds.length === 0) {
      setActiveId(null);
      return;
    }
    setActiveId((cur) => {
      if (cur && visibleIds.includes(cur)) return cur;
      if (selectedMigrationId && visibleIds.includes(selectedMigrationId)) return selectedMigrationId;
      return visibleIds[0];
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visibleKey, selectedMigrationId]);

  // Focus the grid once it first has rows so arrows work without a click —
  // but never steal focus from an input the user is already typing in.
  React.useEffect(() => {
    if (didAutoFocus.current || visibleIds.length === 0 || !gridRef.current) return;
    const active = document.activeElement;
    const inInput = active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA");
    if (!inInput && (!active || active === document.body)) {
      gridRef.current.focus({ preventScroll: true });
    }
    didAutoFocus.current = true;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visibleKey]);

  function moveActive(target) {
    if (visibleIds.length === 0) return;
    const curIdx = Math.max(0, visibleIds.indexOf(activeId));
    const nextIdx =
      target === "first" ? 0
      : target === "last" ? visibleIds.length - 1
      : Math.min(visibleIds.length - 1, Math.max(0, curIdx + target));
    const nextId = visibleIds[nextIdx];
    setActiveId(nextId);
    rowRefs.current.get(nextId)?.scrollIntoView({ block: "nearest" });
    // When the panel is open, let it follow the cursor (master-detail).
    if (selectedMigrationId != null) setSelectedMigrationId(nextId);
  }

  function onGridKeyDown(e) {
    switch (e.key) {
      case "ArrowDown": e.preventDefault(); moveActive(1); break;
      case "ArrowUp": e.preventDefault(); moveActive(-1); break;
      case "Home": e.preventDefault(); moveActive("first"); break;
      case "End": e.preventDefault(); moveActive("last"); break;
      case "Enter":
      case " ": {
        // If a control (checkbox, action button) is focused, let it handle the key.
        if (e.target.closest?.('button, a, input, select, textarea, [role="checkbox"]')) return;
        if (!activeId) return;
        e.preventDefault();
        if (e.key === "Enter") setSelectedMigrationId(activeId);
        else toggleChecked(activeId);
        break;
      }
      default: break;
    }
  }

  if (isLoading && migrations.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
        <Spinner className="size-5" />
        <span className="text-sm">Loading migrations...</span>
      </div>
    );
  }

  if (migrations.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
        No migrations found. Create one to get started.
      </div>
    );
  }

  if (rows.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
        No migrations match your filter.
      </div>
    );
  }

  return (
    <div
      ref={gridRef}
      role="grid"
      aria-label="Migrations"
      tabIndex={0}
      onKeyDown={onGridKeyDown}
      className="flex-1 min-h-0 rounded-sm outline-none focus-visible:ring-1 focus-visible:ring-ring/40"
    >
      <ScrollArea className="h-full">
        <Table style={{ tableLayout: "fixed", width: "100%" }} className={cn(isFetching && "opacity-60 transition-opacity")}>
          <TableHeader>
            {table.getHeaderGroups().map((hg) => (
              <TableRow key={hg.id} className="hover:bg-transparent">
                {hg.headers.map((h) => (
                  <TableHead key={h.id} style={{ width: h.column.columnDef.size ? `${h.column.columnDef.size}px` : undefined }}>
                    {h.isPlaceholder ? null : flexRender(h.column.columnDef.header, h.getContext())}
                  </TableHead>
                ))}
              </TableRow>
            ))}
          </TableHeader>
          <TableBody>
            {rows.map((row) => {
              const m = row.original;
              const isForeign = foreignNames.has(m.name);
              const isSelected = selectedMigrationId === m.id;
              const isActive = activeId === m.id;
              return (
                <TableRow
                  key={row.id}
                  ref={(el) => {
                    if (el) rowRefs.current.set(m.id, el);
                    else rowRefs.current.delete(m.id);
                  }}
                  role="row"
                  aria-selected={isActive}
                  onMouseDown={() => setActiveId(m.id)}
                  data-state={isSelected ? "selected" : undefined}
                  className={cn(
                    "group h-9",
                    isForeign && "border-l-2 border-l-destructive",
                    isActive && "bg-accent/40 ring-1 ring-inset ring-ring/40"
                  )}
                >
                  {row.getVisibleCells().map((cell) => (
                    <TableCell key={cell.id} style={{ width: cell.column.columnDef.size ? `${cell.column.columnDef.size}px` : undefined }}>
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </TableCell>
                  ))}
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </ScrollArea>
    </div>
  );
}
