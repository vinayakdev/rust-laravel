"use client"

import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { ConfigReport, Payload, RegistrationSource } from "@/lib/types"

function SrcCell({
  source,
  root,
}: {
  source: RegistrationSource
  root?: string
}) {
  return (
    <div className="flex min-w-[140px] flex-col gap-0.5">
      <span className="text-[0.7rem] font-medium">{source.kind}</span>
      {source.provider_class && (
        <TruncCell value={source.provider_class} maxW="max-w-[200px]" muted />
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

export function ConfigView({
  payload,
  sourceMode,
}: {
  payload: Payload
  sourceMode: boolean
}) {
  const report = payload.report as ConfigReport
  const root = payload.root as string | undefined

  const rows = report.items.map((item) => [
    <div className="flex min-w-[180px] flex-col gap-0.5">
      <TruncCell value={item.key} maxW="max-w-[220px]" size="text-[0.75rem]" />
      <LocCell
        file={item.file}
        line={item.line}
        col={item.column}
        root={root}
      />
    </div>,

    item.env_key ? (
      <TruncCell value={item.env_key} maxW="max-w-[140px]" />
    ) : (
      <span className="text-xs text-muted-foreground">—</span>
    ),

    item.default_value != null ? (
      <TruncCell value={item.default_value} maxW="max-w-[120px]" />
    ) : (
      <span className="font-mono text-[0.7rem] text-muted-foreground">
        null
      </span>
    ),

    item.env_value != null ? (
      <TruncCell value={item.env_value} maxW="max-w-[120px]" />
    ) : (
      <span className="text-xs text-muted-foreground">—</span>
    ),

    <SrcCell source={item.source} root={root} />,
  ])

  return (
    <DataSection
      title={sourceMode ? "Config Sources" : "Effective Config"}
      count={report.item_count}
      columns={[
        { key: "key", label: "Config Key" },
        { key: "env", label: "Env Key" },
        { key: "default", label: "Default" },
        { key: "value", label: "Env Value" },
        { key: "source", label: "Registered Via" },
      ]}
      rows={rows}
    />
  )
}
