"use client"

import { useState } from "react"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { LocCell, TruncCell, relPath } from "@/components/app/cell"
import type { ColumnEntry, ModelEntry, ModelReport, Payload, RelationEntry } from "@/lib/types"

function EmptyValue() {
  return <span className="text-xs text-muted-foreground">—</span>
}

function TokenList({ values, mono = true }: { values: string[]; mono?: boolean }) {
  if (!values.length) return <EmptyValue />
  return (
    <div className="flex flex-wrap gap-1">
      {values.map((value) => (
        <Badge
          key={value}
          variant="outline"
          className={mono ? "h-5 rounded-sm font-mono text-[0.65rem]" : "h-5 rounded-sm text-[0.65rem]"}
        >
          {value}
        </Badge>
      ))}
    </div>
  )
}

function BoolBadge({ value, trueLabel, falseLabel }: { value: boolean; trueLabel: string; falseLabel: string }) {
  return (
    <Badge variant={value ? "default" : "secondary"} className="h-5 rounded-sm text-[0.65rem]">
      {value ? trueLabel : falseLabel}
    </Badge>
  )
}

function PropertyGrid({ items }: { items: Array<{ label: string; value: React.ReactNode }> }) {
  return (
    <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {items.map((item) => (
        <div key={item.label} className="rounded-md border bg-muted/20 p-3">
          <div className="text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">{item.label}</div>
          <div className="mt-1 text-[0.75rem]">{item.value}</div>
        </div>
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

function ColumnFlags({ column }: { column: ColumnEntry }) {
  const flags = [
    column.primary ? "primary" : null,
    column.nullable ? "nullable" : "required",
    column.unique ? "unique" : null,
    column.unsigned ? "unsigned" : null,
  ].filter(Boolean) as string[]

  return <TokenList values={flags} />
}

function ColumnRows({ columns }: { columns: ColumnEntry[] }) {
  if (!columns.length) return <EmptyValue />

  return (
    <div className="overflow-x-auto">
      <table className="min-w-full text-left text-[0.72rem]">
        <thead className="border-b text-[0.64rem] uppercase tracking-wider text-muted-foreground">
          <tr>
            <th className="px-2 py-2 font-semibold">Column</th>
            <th className="px-2 py-2 font-semibold">Type</th>
            <th className="px-2 py-2 font-semibold">Flags</th>
            <th className="px-2 py-2 font-semibold">Default</th>
            <th className="px-2 py-2 font-semibold">References</th>
          </tr>
        </thead>
        <tbody>
          {columns.map((column) => {
            const refs =
              column.references && column.on_table
                ? `${column.on_table}.${column.references}`
                : column.references || column.on_table || ""

            return (
              <tr key={column.name} className="border-b last:border-b-0">
                <td className="px-2 py-2 align-top">
                  <div className="font-mono">{column.name}</div>
                  {column.comment && <div className="mt-0.5 text-muted-foreground">{column.comment}</div>}
                  {column.enum_values.length > 0 && (
                    <div className="mt-1">
                      <TokenList values={column.enum_values} mono={false} />
                    </div>
                  )}
                </td>
                <td className="px-2 py-2 align-top font-mono">{column.column_type}</td>
                <td className="px-2 py-2 align-top">
                  <ColumnFlags column={column} />
                </td>
                <td className="px-2 py-2 align-top">{column.default ? <TruncCell value={column.default} maxW="max-w-[160px]" /> : <EmptyValue />}</td>
                <td className="px-2 py-2 align-top">{refs ? <span className="font-mono">{refs}</span> : <EmptyValue />}</td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

function RelationRows({ relations, root }: { relations: RelationEntry[]; root?: string }) {
  if (!relations.length) return <EmptyValue />

  return (
    <div className="overflow-x-auto">
      <table className="min-w-full text-left text-[0.72rem]">
        <thead className="border-b text-[0.64rem] uppercase tracking-wider text-muted-foreground">
          <tr>
            <th className="px-2 py-2 font-semibold">Method</th>
            <th className="px-2 py-2 font-semibold">Type</th>
            <th className="px-2 py-2 font-semibold">Related Model</th>
            <th className="px-2 py-2 font-semibold">Keys</th>
          </tr>
        </thead>
        <tbody>
          {relations.map((relation) => (
            <tr key={`${relation.method}:${relation.line}`} className="border-b last:border-b-0">
              <td className="px-2 py-2 align-top">
                <div className="font-mono">{relation.method}()</div>
                <div className="mt-1">
                  <Badge variant="outline" className="h-4 rounded-sm text-[0.6rem]">
                    line {relation.line}
                  </Badge>
                </div>
              </td>
              <td className="px-2 py-2 align-top">
                <Badge variant="secondary" className="h-5 rounded-sm font-mono text-[0.65rem]">
                  {relation.relation_type}
                </Badge>
              </td>
              <td className="px-2 py-2 align-top">
                <div className="font-mono">{relation.related_model}</div>
                {relation.related_model_file && (
                  <div className="mt-0.5 text-muted-foreground">{relPath(relation.related_model_file, root)}</div>
                )}
              </td>
              <td className="px-2 py-2 align-top">
                <div className="flex flex-wrap gap-1">
                  {relation.foreign_key && (
                    <Badge variant="outline" className="h-5 rounded-sm font-mono text-[0.65rem]">
                      fk:{relation.foreign_key}
                    </Badge>
                  )}
                  {relation.local_key && (
                    <Badge variant="outline" className="h-5 rounded-sm font-mono text-[0.65rem]">
                      local:{relation.local_key}
                    </Badge>
                  )}
                  {relation.pivot_table && (
                    <Badge variant="outline" className="h-5 rounded-sm font-mono text-[0.65rem]">
                      pivot:{relation.pivot_table}
                    </Badge>
                  )}
                  {!relation.foreign_key && !relation.local_key && !relation.pivot_table && <EmptyValue />}
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function ModelSummary({ model, root }: { model: ModelEntry; root?: string }) {
  return (
    <Card className="gap-3">
      <CardHeader className="border-b">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <CardTitle>{model.class_name}</CardTitle>
            <div className="mt-1 text-[0.72rem] text-muted-foreground">
              <span className="font-mono">{model.namespace}</span>
            </div>
          </div>
          <div className="flex flex-wrap justify-end gap-1">
            <BoolBadge value={model.timestamps} trueLabel="timestamps" falseLabel="no timestamps" />
            <BoolBadge value={model.soft_deletes} trueLabel="soft deletes" falseLabel="no soft deletes" />
            <BoolBadge value={model.incrementing} trueLabel="incrementing" falseLabel="manual key" />
          </div>
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        <PropertyGrid
          items={[
            {
              label: "Source",
              value: <LocCell file={model.file} line={model.line} col={1} root={root} />,
            },
            {
              label: "Table",
              value: (
                <div className="flex flex-wrap items-center gap-1.5">
                  <span className="font-mono">{model.table}</span>
                  <Badge variant={model.table_inferred ? "secondary" : "default"} className="h-5 rounded-sm text-[0.65rem]">
                    {model.table_inferred ? "inferred" : "declared"}
                  </Badge>
                </div>
              ),
            },
            { label: "Connection", value: model.connection ? <span className="font-mono">{model.connection}</span> : <EmptyValue /> },
            { label: "Primary Key", value: <span className="font-mono">{model.primary_key}</span> },
            { label: "Key Type", value: <span className="font-mono">{model.key_type}</span> },
            { label: "Eager Loads", value: <TokenList values={model.with} /> },
          ]}
        />
      </CardContent>
    </Card>
  )
}

function ModelBehavior({ model }: { model: ModelEntry }) {
  const casts = Object.entries(model.casts)

  return (
    <div className="grid gap-4 xl:grid-cols-2">
      <SectionCard title="Attributes">
        <div className="space-y-3">
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Fillable</div>
            <TokenList values={model.fillable} />
          </div>
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Guarded</div>
            <TokenList values={model.guarded} />
          </div>
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Hidden</div>
            <TokenList values={model.hidden} />
          </div>
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Appends</div>
            <TokenList values={model.appends} />
          </div>
        </div>
      </SectionCard>

      <SectionCard title="Model Hooks">
        <div className="space-y-3">
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Traits</div>
            <TokenList values={model.traits} mono={false} />
          </div>
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Scopes</div>
            <TokenList values={model.scopes} />
          </div>
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Accessors</div>
            <TokenList values={model.accessors} />
          </div>
          <div>
            <div className="mb-1 text-[0.65rem] font-semibold uppercase tracking-wider text-muted-foreground">Mutators</div>
            <TokenList values={model.mutators} />
          </div>
        </div>
      </SectionCard>

      <SectionCard title="Casts" count={casts.length}>
        {casts.length === 0 ? (
          <EmptyValue />
        ) : (
          <div className="grid gap-2 sm:grid-cols-2">
            {casts.map(([name, value]) => (
              <div key={name} className="rounded-md border bg-muted/20 p-2">
                <div className="font-mono text-[0.72rem]">{name}</div>
                <div className="mt-0.5 text-muted-foreground">{value}</div>
              </div>
            ))}
          </div>
        )}
      </SectionCard>
    </div>
  )
}

export function ModelsView({ payload }: { payload: Payload }) {
  const report = payload.report as ModelReport
  const root = payload.root as string | undefined
  const [selectedIndex, setSelectedIndex] = useState(0)
  const model = report.models[Math.min(selectedIndex, report.models.length - 1)]

  if (!report.models.length) {
    return (
      <Card>
        <CardContent className="py-12 text-center text-sm text-muted-foreground">No models found.</CardContent>
      </Card>
    )
  }

  return (
    <div className="grid gap-4 xl:grid-cols-[320px_minmax(0,1fr)]">
      <Card className="gap-0">
        <CardHeader className="border-b">
          <div className="flex items-center justify-between gap-3">
            <CardTitle>Models</CardTitle>
            <Badge variant="secondary" className="h-5 font-mono tabular-nums">
              {report.model_count}
            </Badge>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <div className="max-h-[calc(100svh-14rem)] overflow-y-auto">
            {report.models.map((entry, index) => (
              <button
                key={`${entry.class_name}:${entry.file}`}
                type="button"
                onClick={() => setSelectedIndex(index)}
                className={[
                  "w-full border-b px-4 py-3 text-left transition-colors last:border-b-0",
                  index === selectedIndex ? "bg-muted/60" : "hover:bg-muted/30",
                ].join(" ")}
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="truncate font-medium">{entry.class_name}</div>
                    <div className="mt-0.5 truncate font-mono text-[0.68rem] text-muted-foreground">{entry.table}</div>
                  </div>
                  <Badge variant="outline" className="h-5 rounded-sm font-mono text-[0.6rem]">
                    {entry.columns.length} cols
                  </Badge>
                </div>
                <div className="mt-2 text-[0.68rem] text-muted-foreground">
                  {relPath(entry.file, root)}:{entry.line}
                </div>
              </button>
            ))}
          </div>
        </CardContent>
      </Card>

      <div className="space-y-4">
        <ModelSummary model={model} root={root} />

        <SectionCard title="Columns" count={model.columns.length}>
          <ColumnRows columns={model.columns} />
        </SectionCard>

        <SectionCard title="Relations" count={model.relations.length}>
          <RelationRows relations={model.relations} root={root} />
        </SectionCard>

        <ModelBehavior model={model} />
      </div>
    </div>
  )
}
