"use client"

import { Badge } from "@/components/ui/badge"
import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { Payload, ProviderReport } from "@/lib/types"

export function ProvidersView({ payload }: { payload: Payload }) {
  const report = payload.report as ProviderReport
  const root = payload.root as string | undefined

  const rows = report.providers.map((p) => [
    <div className="flex min-w-[200px] flex-col gap-0.5">
      <TruncCell
        value={p.provider_class}
        maxW="max-w-[240px]"
        size="text-[0.72rem]"
      />
      <LocCell file={p.declared_in} line={p.line} col={p.column} root={root} />
    </div>,

    <span className="text-[0.72rem]">{p.registration_kind}</span>,

    <span className="text-[0.72rem] text-muted-foreground">
      {p.package_name || "—"}
    </span>,

    p.source_file ? (
      <TruncCell value={p.source_file} maxW="max-w-[200px]" muted />
    ) : (
      <span className="text-xs text-muted-foreground">—</span>
    ),

    <div className="flex flex-wrap gap-1">
      <Badge
        variant={p.source_available ? "default" : "destructive"}
        className="h-4 rounded-sm font-mono text-[0.6rem]"
      >
        {p.status}
      </Badge>
      <Badge
        variant={p.source_available ? "secondary" : "destructive"}
        className="h-4 rounded-sm text-[0.6rem]"
      >
        {p.source_available ? "source ✓" : "missing"}
      </Badge>
    </div>,
  ])

  return (
    <DataSection
      title="Service Providers"
      count={report.provider_count}
      columns={[
        { key: "class", label: "Provider Class" },
        { key: "kind", label: "Kind" },
        { key: "package", label: "Package" },
        { key: "file", label: "Source File" },
        { key: "status", label: "Status" },
      ]}
      rows={rows}
    />
  )
}
