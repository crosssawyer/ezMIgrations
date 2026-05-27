import * as React from "react";
import {
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { Eye, Play, Trash2, FileCode2, ArrowUpDown, ArrowUp, ArrowDown, AlertTriangle, Copy } from "lucide-react";

import { copyToClipboard } from "@/lib/utils";

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

function buildColumns({ checked, setChecked, toggleChecked, setSelectedMigrationId, foreignNames, applyTo, isApplying, removeMigration }) {
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
        const isForeign = foreignNames.has(m.name);
        return (
          <div className="flex items-center gap-2 min-w-0">
            <button
              onClick={() => setSelectedMigrationId(m.id)}
              className="font-mono text-[11px] text-left hover:text-primary transition-colors truncate"
            >
              {m.name}
            </button>
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
      size: 132,
      header: () => <span className="block text-right">Actions</span>,
      cell: ({ row }) => {
        const m = row.original;
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

export function MigrationsTable({ migrations, isLoading, isFetching, isError, error, foreignNames }) {
  const { checked, setChecked, toggleChecked, setSelectedMigrationId, selectedMigrationId, searchQuery, setSearchQuery } = useUI();
  const { applyTo, isApplying } = useApplyTo();
  const removeMigration = useRemoveLastOrForce(migrations);

  const columns = React.useMemo(
    () => buildColumns({ checked, setChecked, toggleChecked, setSelectedMigrationId, foreignNames, applyTo, isApplying, removeMigration }),
    [checked, setChecked, toggleChecked, setSelectedMigrationId, foreignNames, applyTo, isApplying, removeMigration]
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

  if (isLoading && migrations.length === 0) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
        <Spinner className="size-5" />
        <span className="text-sm">Loading migrations...</span>
      </div>
    );
  }

  if (isError && migrations.length === 0) {
    const message = error?.message || String(error || "Unknown error");
    return (
      <div className="flex flex-1 items-start justify-center p-6">
        <div className="max-w-2xl w-full rounded-md border border-destructive/40 bg-destructive/5 p-4 space-y-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-destructive">
            <AlertTriangle className="h-4 w-4" />
            Couldn't list migrations
          </div>
          <pre className="font-mono text-xs leading-relaxed text-foreground/90 whitespace-pre-wrap break-words max-h-80 overflow-auto rounded border border-border bg-background/60 p-3">
{message}
          </pre>
          <div className="flex items-center justify-between gap-2">
            <p className="text-xs text-muted-foreground">
              This is the raw error from <code>dotnet ef migrations list</code>. Share this with support to diagnose.
            </p>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => copyToClipboard(message, { successMessage: "Error copied to clipboard" })}
            >
              <Copy className="h-3.5 w-3.5 mr-1.5" />
              Copy
            </Button>
          </div>
        </div>
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

  const rows = table.getRowModel().rows;
  if (rows.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
        No migrations match your filter.
      </div>
    );
  }

  return (
    <ScrollArea className="flex-1">
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
            return (
              <TableRow
                key={row.id}
                data-state={isSelected ? "selected" : undefined}
                className={cn("group h-9", isForeign && "border-l-2 border-l-destructive")}
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
  );
}
