"use client"

import { DataSection } from "@/components/app/data-section"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import type { DashboardReport, Payload } from "@/lib/types"

function formatKb(v: number | null | undefined): string {
  if (v == null || !Number.isFinite(v)) return "—"
  const abs = Math.abs(v)
  if (abs >= 1024 * 1024) return `${(v / 1024 / 1024).toFixed(2)} GB`
  if (abs >= 1024) return `${(v / 1024).toFixed(1)} MB`
  return `${v} KB`
}

function SummaryCard({
  label,
  value,
}: {
  label: string
  value: number
}) {
  return (
    <Card size="sm">
      <CardHeader className="border-b">
        <CardTitle>{label}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="font-mono text-3xl font-semibold tabular-nums">
          {value}
        </div>
      </CardContent>
    </Card>
  )
}

export function DashboardView({ payload }: { payload: Payload }) {
  const report = payload.report as DashboardReport

  const rows = report.features.map((feature) => [
    <span className="text-sm font-medium">{feature.label}</span>,
    <span className="font-mono text-[0.78rem] tabular-nums">
      {feature.files_scanned}
    </span>,
    <span className="font-mono text-[0.78rem] tabular-nums">
      {feature.items_found}
    </span>,
    <span className="font-mono text-[0.78rem] tabular-nums">
      {feature.autocomplete_suggestions}
    </span>,
    <span className="font-mono text-[0.78rem] tabular-nums">
      {feature.scan_time_ms} ms
    </span>,
    <span className="font-mono text-[0.78rem] tabular-nums">
      {feature.rss_delta_kb != null
        ? `${feature.rss_delta_kb > 0 ? "+" : ""}${formatKb(feature.rss_delta_kb)}`
        : "—"}
    </span>,
  ])

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-4 md:grid-cols-3">
        <SummaryCard
          label="Total Files Scanned"
          value={report.summary.total_files_scanned}
        />
        <SummaryCard
          label="Total Items Found"
          value={report.summary.total_items_found}
        />
        <SummaryCard
          label="Total Suggestions"
          value={report.summary.total_autocomplete_suggestions}
        />
      </div>

      <DataSection
        title="Autocomplete Coverage By Feature"
        count={report.summary.feature_count}
        columns={[
          { key: "feature", label: "Feature" },
          { key: "files", label: "Files Scanned" },
          { key: "items", label: "Items Found" },
          { key: "suggestions", label: "Autocomplete Suggestions" },
          { key: "time", label: "Scan Time" },
          { key: "ram", label: "RAM Delta" },
        ]}
        rows={rows}
      />
    </div>
  )
}
