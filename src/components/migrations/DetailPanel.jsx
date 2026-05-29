import * as React from "react";
import { X, ChevronDown, ChevronRight, Copy } from "lucide-react";

import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { useUI } from "@/lib/ui-store";
import { useMigrationSql } from "@/lib/queries";
import { cn, copyToClipboard } from "@/lib/utils";

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

function CopyButton({ text, title = "Copy", className }) {
  return (
    <Button
      size="icon-sm"
      variant="ghost"
      className={cn("absolute right-1.5 top-1.5 opacity-50 hover:opacity-100", className)}
      onClick={(e) => {
        e.stopPropagation();
        copyToClipboard(text);
      }}
      title={title}
    >
      <Copy className="h-3 w-3" />
    </Button>
  );
}

function CodeBlock({ body }) {
  return (
    <div className="relative">
      {body && <CopyButton text={body} />}
      <pre className="m-0 overflow-x-auto rounded-md border border-border bg-background p-3 font-mono text-xs leading-relaxed">
        <code>{body || "(empty)"}</code>
      </pre>
    </div>
  );
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
        <div className="relative border-t border-border">
          <CopyButton text={statement.trim()} title="Copy SQL" />
          <pre className="m-0 max-h-80 overflow-auto bg-background px-3 py-2 font-mono text-xs leading-relaxed">
            <code>{statement.trim()}</code>
          </pre>
        </div>
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
  const allSql = statements.map((s) => s.trim()).join("\n\n");
  return (
    <div className="flex flex-col gap-2">
      <div className="flex justify-end">
        <Button
          size="sm"
          variant="ghost"
          className="h-6 gap-1.5 px-2 text-xs text-muted-foreground hover:text-foreground"
          onClick={() => copyToClipboard(allSql)}
          title={`Copy all custom SQL in ${direction}`}
        >
          <Copy className="h-3 w-3" />
          Copy all
        </Button>
      </div>
      {statements.map((s, i) => (
        <SqlCard key={i} statement={s} index={i} />
      ))}
    </div>
  );
}

function CountBadge({ count }) {
  if (!count) return null;
  return (
    <Badge variant="primary" size="xs" className="px-1.5">
      {count}
    </Badge>
  );
}

function Section({ title, count, children }) {
  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
          {title}
        </span>
        <CountBadge count={count} />
      </div>
      {children}
    </div>
  );
}

// The detail panel shows four views of a migration's SQL. Each is defined once
// here and rendered both as its own tab and stacked together under the "All"
// tab, so the two views never drift apart.
const SQL_SECTIONS = [
  { value: "up", tab: "Up()", title: "Up()", render: (sql) => <CodeBlock body={sql.up_body} /> },
  { value: "down", tab: "Down()", title: "Down()", render: (sql) => <CodeBlock body={sql.down_body} /> },
  {
    value: "sql-up",
    tab: "SQL Up",
    title: "Custom SQL Up",
    count: (sql) => sql.custom_sql_up?.length,
    render: (sql) => <SqlList statements={sql.custom_sql_up} direction="Up()" />,
  },
  {
    value: "sql-down",
    tab: "SQL Down",
    title: "Custom SQL Down",
    count: (sql) => sql.custom_sql_down?.length,
    render: (sql) => <SqlList statements={sql.custom_sql_down} direction="Down()" />,
  },
];

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
          <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <Spinner className="size-5" />
            <span className="text-xs">Loading migration details...</span>
          </div>
        ) : (
          <Tabs defaultValue="all" className="flex h-full flex-col">
            <div className="px-4 pt-3">
              <div className="overflow-x-auto pb-0.5">
                <TabsList>
                  <TabsTrigger value="all">All</TabsTrigger>
                  {SQL_SECTIONS.map((s) => (
                    <TabsTrigger key={s.value} value={s.value} className="gap-1.5">
                      {s.tab}
                      <CountBadge count={s.count?.(sql)} />
                    </TabsTrigger>
                  ))}
                </TabsList>
              </div>
            </div>
            <ScrollArea className="flex-1 px-4 pb-4">
              <TabsContent value="all">
                <div className="flex flex-col gap-5">
                  {SQL_SECTIONS.map((s) => (
                    <Section key={s.value} title={s.title} count={s.count?.(sql)}>
                      {s.render(sql)}
                    </Section>
                  ))}
                </div>
              </TabsContent>
              {SQL_SECTIONS.map((s) => (
                <TabsContent key={s.value} value={s.value}>
                  {s.render(sql)}
                </TabsContent>
              ))}
            </ScrollArea>
          </Tabs>
        )}
      </div>
    </div>
  );
}
