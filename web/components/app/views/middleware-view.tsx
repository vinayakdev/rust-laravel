"use client"

import { Badge } from "@/components/ui/badge"
import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { MiddlewareReport, Payload, RegistrationSource } from "@/lib/types"

function SrcCell({
  source,
  root,
}: {
  source: RegistrationSource
  root?: string
}) {
  return (
    <div className="flex flex-col gap-0.5">
      {source.provider_class && (
        <TruncCell
          value={source.provider_class}
          maxW="max-w-[200px]"
          size="text-[0.7rem]"
        />
      )}
      <LocCell
        file={source.declared_in}
        line={source.line}
        col={source.column}
        root={root}
      />
    </div>
  )
}

export function MiddlewareView({ payload }: { payload: Payload }) {
  const report = payload.report as MiddlewareReport
  const root = payload.root as string | undefined

  const aliasRows = report.aliases.map((a) => [
    <code className="font-mono text-[0.75rem] font-semibold">{a.name}</code>,
    <TruncCell value={a.target} maxW="max-w-[260px]" />,
    <SrcCell source={a.source} root={root} />,
  ])

  const groupRows = report.groups.map((g) => [
    <code className="font-mono text-[0.75rem] font-semibold">{g.name}</code>,
    g.members.length ? (
      <div className="flex min-w-[160px] flex-wrap gap-0.5">
        {g.members.map((m, i) => (
          <Badge
            key={i}
            variant="outline"
            className="h-4 rounded-sm font-mono text-[0.6rem]"
          >
            {m}
          </Badge>
        ))}
      </div>
    ) : (
      <span className="text-xs text-muted-foreground">—</span>
    ),
    <SrcCell source={g.source} root={root} />,
  ])

  const patternRows = report.patterns.map((p) => [
    <code className="font-mono text-[0.75rem] font-semibold">
      {p.parameter}
    </code>,
    <code className="font-mono text-[0.72rem]">{p.pattern}</code>,
    <SrcCell source={p.source} root={root} />,
  ])

  return (
    <div className="flex flex-col gap-4">
      <DataSection
        title="Middleware Aliases"
        count={report.alias_count}
        columns={[
          { key: "alias", label: "Alias" },
          { key: "target", label: "Target Class" },
          { key: "declared", label: "Declared In" },
        ]}
        rows={aliasRows}
      />
      <DataSection
        title="Middleware Groups"
        count={report.group_count}
        columns={[
          { key: "group", label: "Group" },
          { key: "members", label: "Members" },
          { key: "declared", label: "Declared In" },
        ]}
        rows={groupRows}
      />
      <DataSection
        title="Route Patterns"
        count={report.pattern_count}
        columns={[
          { key: "param", label: "Parameter" },
          { key: "pattern", label: "Pattern (Regex)" },
          { key: "declared", label: "Declared In" },
        ]}
        rows={patternRows}
      />
    </div>
  )
}
