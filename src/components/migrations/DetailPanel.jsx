import * as React from "react";
import { X, ChevronDown, ChevronRight, Loader2 } from "lucide-react";

import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { useUI } from "@/lib/ui-store";
import { useMigrationSql } from "@/lib/queries";

function extractSqlMeta(sql) {
  const m = sql.match(
    /(?:CREATE\s+(?:OR\s+ALTER\s+)?|ALTER\s+)(PROCEDURE|VIEW|FUNCTION|TRIGGER)\s+([\w.\[\]]+(?:\.[\w.\[\]]+)*)/i
  );
  if (m) return { type: m[1].toUpperCase(), name: m[2].replace(/\[|\]/g, "") };
  const drop = sql.match(/DROP\s+(VIEW|PROCEDURE|FUNCTION|TRIGGER)\s+(?:IF\s+EXISTS\s+)?([\w.\[\]]+(?:\.[\w.\[\]]+)*)/i);
  if (drop) return { type: "DROP " + drop[1].toUpperCase(), name: drop[2].replace(/\[|\]/g, "") };
  const update = sql.match(/^\s*UPDATE\s+([\w.\[\]]+(?:\.[\w.\[\]]+)*)/i);
  if (update) return { type: "UPDATE", name: update[1].replace(/\[|\]/g, "") };
  return null;
}

function SqlCard({ statement, index }) {
  const long = statement.split("\n").length > 6;
  const [expanded, setExpanded] = React.useState(!long);
  const meta = extractSqlMeta(statement);
  return (
    <div className="rounded-md border border-border bg-background overflow-hidden">
      <button
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center justify-between px-3 py-2 text-left hover:bg-muted/40 transition-colors"
      >
        <div className="flex items-center gap-2 min-w-0">
          {meta && (
            <Badge variant="primary" size="xs" className="font-mono uppercase tracking-wider px-1.5">
              {meta.type}
            </Badge>
          )}
          <span className="font-mono text-xs truncate">
            {meta ? meta.name : `Statement ${index + 1}`}
          </span>
        </div>
        {expanded ? <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" /> : <ChevronRight className="h-3.5 w-3.5 text-muted-foreground" />}
      </button>
      {expanded && (
        <pre className="m-0 max-h-80 overflow-auto border-t border-border bg-background px-3 py-2 font-mono text-xs leading-relaxed">
          <code>{statement.trim()}</code>
        </pre>
      )}
    </div>
  );
}

function SqlList({ statements, direction }) {
  if (!statements?.length) {
    return (
      <div className="rounded-md border border-dashed border-border py-6 text-center text-xs text-muted-foreground">
        No custom SQL in {direction}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      {statements.map((s, i) => (
        <SqlCard key={i} statement={s} index={i} />
      ))}
    </div>
  );
}

export function DetailPanel({ migrations }) {
  const { selectedMigrationId, setSelectedMigrationId } = useUI();
  const migration = migrations.find((m) => m.id === selectedMigrationId);
  const { data: sql, isLoading } = useMigrationSql(migration?.name);

  if (!migration) return null;

  return (
    <div className="flex h-full w-[420px] min-w-[360px] max-w-[50%] flex-col border-l border-border bg-card">
      <div className="flex items-center justify-between border-b border-border px-4 py-2.5">
        <div className="font-mono text-[11px] truncate min-w-0">{migration.name}</div>
        <Button size="icon-sm" variant="ghost" onClick={() => setSelectedMigrationId(null)}>
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>
      <div className="flex-1 overflow-hidden">
        {isLoading || !sql ? (
          <div className="p-4 text-xs text-muted-foreground flex items-center gap-2">
            <Loader2 className="h-3.5 w-3.5 animate-spin" /> Loading migration details…
          </div>
        ) : (
          <Tabs defaultValue="up" className="flex h-full flex-col">
            <div className="px-4 pt-3">
              <TabsList>
                <TabsTrigger value="up">Up()</TabsTrigger>
                <TabsTrigger value="down">Down()</TabsTrigger>
                <TabsTrigger value="sql-up" className="gap-1.5">
                  SQL Up
                  {sql.custom_sql_up?.length ? (
                    <Badge variant="primary" size="xs" className="px-1.5">{sql.custom_sql_up.length}</Badge>
                  ) : null}
                </TabsTrigger>
                <TabsTrigger value="sql-down" className="gap-1.5">
                  SQL Down
                  {sql.custom_sql_down?.length ? (
                    <Badge variant="primary" size="xs" className="px-1.5">{sql.custom_sql_down.length}</Badge>
                  ) : null}
                </TabsTrigger>
              </TabsList>
            </div>
            <ScrollArea className="flex-1 px-4 pb-4">
              <TabsContent value="up">
                <pre className="m-0 rounded-md border border-border bg-background p-3 font-mono text-xs leading-relaxed">
                  <code>{sql.up_body || "(empty)"}</code>
                </pre>
              </TabsContent>
              <TabsContent value="down">
                <pre className="m-0 rounded-md border border-border bg-background p-3 font-mono text-xs leading-relaxed">
                  <code>{sql.down_body || "(empty)"}</code>
                </pre>
              </TabsContent>
              <TabsContent value="sql-up">
                <SqlList statements={sql.custom_sql_up} direction="Up()" />
              </TabsContent>
              <TabsContent value="sql-down">
                <SqlList statements={sql.custom_sql_down} direction="Down()" />
              </TabsContent>
            </ScrollArea>
          </Tabs>
        )}
      </div>
    </div>
  );
}
