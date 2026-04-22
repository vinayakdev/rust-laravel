"use client"

import { useState } from "react"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { TruncCell, relPath } from "@/components/app/cell"
import type {
  ColumnEntry,
  IndexEntry,
  MigrationEntry,
  MigrationReport,
  Payload,
} from "@/lib/types"

function EmptyValue() {
  return <span className="text-xs text-muted-foreground">—</span>
}

function TokenList({ values }: { values: string[] }) {
  if (!values.length) return <EmptyValue />
  return (
    <div className="flex flex-wrap gap-1">
      {values.map((value) => (
        <Badge
          key={value}
          variant="outline"
          className="h-5 rounded-sm font-mono text-[0.65rem]"
        >
          {value}
        </Badge>
      ))}
    </div>
  )
}

function SectionCard({
  title,
  count,
  children,
}: {
  title: string
  count?: number
  children: React.ReactNode
}) {
  return (
    <Card size="sm" className="gap-3">
      <CardHeader className="border-b">
        <div className="flex items-center justify-between gap-3">
          <CardTitle>{title}</CardTitle>
          {count != null && (
            <Badge variant="secondary" className="h-5 font-mono tabular-nums">
              {count}
            </Badge>
          )}
        </div>
      </CardHeader>
      <CardContent>{children}</CardContent>
    </Card>
  )
}

function ColumnRows({ columns }: { columns: ColumnEntry[] }) {
  if (!columns.length) return <EmptyValue />

  return (
    <div className="overflow-x-auto">
      <table className="min-w-full text-left text-[0.72rem]">
        <thead className="border-b text-[0.64rem] tracking-wider text-muted-foreground uppercase">
          <tr>
            <th className="px-2 py-2 font-semibold">Column</th>
            <th className="px-2 py-2 font-semibold">Type</th>
            <th className="px-2 py-2 font-semibold">Default</th>
            <th className="px-2 py-2 font-semibold">Flags</th>
          </tr>
        </thead>
        <tbody>
          {columns.map((column) => {
            const flags = [
              column.primary ? "primary" : null,
              column.nullable ? "nullable" : "required",
              column.unique ? "unique" : null,
              column.unsigned ? "unsigned" : null,
              column.references ? `ref:${column.references}` : null,
              column.on_table ? `table:${column.on_table}` : null,
            ].filter(Boolean) as string[]

            return (
              <tr key={column.name} className="border-b last:border-b-0">
                <td className="px-2 py-2 align-top">
                  <div className="font-mono">{column.name}</div>
                  {column.enum_values.length > 0 && (
                    <div className="mt-1">
                      <TokenList values={column.enum_values} />
                    </div>
                  )}
                </td>
                <td className="px-2 py-2 align-top font-mono">
                  {column.column_type}
                </td>
                <td className="px-2 py-2 align-top">
                  {column.default ? (
                    <TruncCell value={column.default} maxW="max-w-[180px]" />
                  ) : (
                    <EmptyValue />
                  )}
                </td>
                <td className="px-2 py-2 align-top">
                  <TokenList values={flags} />
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function IndexRows({ indexes }: { indexes: IndexEntry[] }) {
  if (!indexes.length) return <EmptyValue />

  return (
    <div className="grid gap-2 sm:grid-cols-2">
      {indexes.map((index, idx) => (
        <div
          key={`${index.index_type}:${idx}`}
          className="rounded-md border bg-muted/20 p-3"
        >
          <div className="text-[0.65rem] font-semibold tracking-wider text-muted-foreground uppercase">
            {index.index_type}
          </div>
          <div className="mt-2">
            <TokenList values={index.columns} />
          </div>
        </div>
      ))}
    </div>
  )
}

function MigrationSummary({
  migration,
  root,
}: {
  migration: MigrationEntry
  root?: string
}) {
  return (
    <Card className="gap-3">
      <CardHeader className="border-b">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle>{migration.class_name}</CardTitle>
            <div className="mt-1 font-mono text-[0.72rem] text-muted-foreground">
              {migration.timestamp}
            </div>
          </div>
          <Badge
            variant="default"
            className="h-5 rounded-sm font-mono text-[0.65rem]"
          >
            {migration.operation}
          </Badge>
        </div>
      </CardHeader>
      <CardContent className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
        <div className="rounded-md border bg-muted/20 p-3">
          <div className="text-[0.65rem] font-semibold tracking-wider text-muted-foreground uppercase">
            Table
          </div>
          <div className="mt-1 font-mono text-[0.75rem]">{migration.table}</div>
        </div>
        <div className="rounded-md border bg-muted/20 p-3 sm:col-span-2 xl:col-span-2">
          <div className="text-[0.65rem] font-semibold tracking-wider text-muted-foreground uppercase">
            File
          </div>
          <div className="mt-1 font-mono text-[0.72rem] text-muted-foreground">
            {relPath(migration.file, root)}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

export function MigrationsView({ payload }: { payload: Payload }) {
  const report = payload.report as MigrationReport
  const root = payload.root as string | undefined
  const [selectedIndex, setSelectedIndex] = useState(0)
  const migration =
    report.migrations[Math.min(selectedIndex, report.migrations.length - 1)]

  if (!report.migrations.length) {
    return (
      <Card>
        <CardContent className="py-12 text-center text-sm text-muted-foreground">
          No migrations found.
        </CardContent>
      </Card>
    )
  }

  return (
    <div className="grid gap-4 xl:grid-cols-[340px_minmax(0,1fr)]">
      <Card className="gap-0">
        <CardHeader className="border-b">
          <div className="flex items-center justify-between gap-3">
            <CardTitle>Migrations</CardTitle>
            <Badge variant="secondary" className="h-5 font-mono tabular-nums">
              {report.migration_count}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <div className="max-h-[calc(100svh-14rem)] overflow-y-auto">
            {report.migrations.map((entry, index) => (
              <button
                key={`${entry.file}:${entry.class_name}`}
                type="button"
                onClick={() => setSelectedIndex(index)}
                className={[
                  "w-full border-b px-4 py-3 text-left transition-colors last:border-b-0",
                  index === selectedIndex ? "bg-muted/60" : "hover:bg-muted/30",
                ].join(" ")}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-medium">
                      {entry.class_name}
                    </div>
                    <div className="mt-0.5 truncate font-mono text-[0.68rem] text-muted-foreground">
                      {entry.table}
                    </div>
                  </div>
                  <Badge
                    variant="outline"
                    className="h-5 rounded-sm font-mono text-[0.6rem]"
                  >
                    {entry.operation}
                  </Badge>
                </div>
                <div className="mt-2 text-[0.68rem] text-muted-foreground">
                  {entry.timestamp}
                </div>
              </button>
            ))}
          </div>
        </CardContent>
      </Card>

      <div className="space-y-4">
        <MigrationSummary migration={migration} root={root} />

        <SectionCard title="Columns" count={migration.columns.length}>
          <ColumnRows columns={migration.columns} />
        </SectionCard>

        <SectionCard title="Indexes" count={migration.indexes.length}>
          <IndexRows indexes={migration.indexes} />
        </SectionCard>

        <SectionCard
          title="Dropped Columns"
          count={migration.dropped_columns.length}
        >
          <TokenList values={migration.dropped_columns} />
        </SectionCard>
      </div>
    </div>
  )
}
