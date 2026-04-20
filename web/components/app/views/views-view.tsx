"use client"

import { Badge } from "@/components/ui/badge"
import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { Payload, RegistrationSource, ViewReport, ViewVariable } from "@/lib/types"

function VarBadges({ vars }: { vars: ViewVariable[] }) {
  if (!vars?.length) return <span className="text-muted-foreground text-xs">—</span>
  return (
    <div className="flex flex-wrap gap-0.5">
      {vars.map((v, i) => (
        <Badge key={i} variant="outline" className="h-4 rounded-sm font-mono text-[0.6rem]">
          {v.default_value != null ? `${v.name}=${v.default_value}` : v.name}
        </Badge>
      ))}
    </div>
  )
}

function SrcCell({ source, root }: { source: RegistrationSource; root?: string }) {
  return (
    <div className="flex flex-col gap-0.5">
      <LocCell file={source.declared_in} line={source.line} col={source.column} root={root} />
      {source.provider_class && <TruncCell value={source.provider_class} maxW="max-w-[180px]" muted />}
    </div>
  )
}

export function ViewsView({ payload }: { payload: Payload }) {
  const report = payload.report as ViewReport
  const root   = payload.root as string | undefined

  const viewRows = report.views.map((v) => [
    <div className="flex flex-col gap-0.5 min-w-[160px]">
      <TruncCell value={v.name} maxW="max-w-[200px]" size="text-[0.75rem]" />
      <TruncCell value={v.file} maxW="max-w-[200px]" muted />
    </div>,
    <Badge variant="secondary" className="h-4 rounded-sm font-mono text-[0.6rem]">{v.kind}</Badge>,
    <VarBadges vars={v.props} />,
    <VarBadges vars={v.variables} />,
    <SrcCell source={v.source} root={root} />,
  ])

  const bladeRows = report.blade_components.map((c) => [
    <div className="flex flex-col gap-0.5 min-w-[140px]">
      <TruncCell value={c.component} maxW="max-w-[180px]" size="text-[0.75rem]" />
      <Badge variant="outline" className="w-fit h-4 rounded-sm text-[0.6rem]">{c.kind}</Badge>
    </div>,
    c.class_name ? (
      <div className="flex flex-col gap-0.5">
        <TruncCell value={c.class_name} maxW="max-w-[180px]" size="text-[0.72rem]" />
        {c.class_file && <TruncCell value={c.class_file} maxW="max-w-[180px]" muted />}
      </div>
    ) : <span className="text-muted-foreground text-xs">—</span>,
    c.view_name || c.view_file ? (
      <div className="flex flex-col gap-0.5">
        {c.view_name && <TruncCell value={c.view_name} maxW="max-w-[160px]" size="text-[0.72rem]" />}
        {c.view_file && <TruncCell value={c.view_file} maxW="max-w-[160px]" muted />}
      </div>
    ) : <span className="text-muted-foreground text-xs">—</span>,
    <VarBadges vars={c.props} />,
    <SrcCell source={c.source} root={root} />,
  ])

  const livewireRows = report.livewire_components.map((c) => [
    <div className="flex flex-col gap-0.5 min-w-[140px]">
      <TruncCell value={c.component} maxW="max-w-[180px]" size="text-[0.75rem]" />
      <Badge variant="outline" className="w-fit h-4 rounded-sm text-[0.6rem]">{c.kind}</Badge>
    </div>,
    c.class_name ? (
      <div className="flex flex-col gap-0.5">
        <TruncCell value={c.class_name} maxW="max-w-[180px]" size="text-[0.72rem]" />
        {c.class_file && <TruncCell value={c.class_file} maxW="max-w-[180px]" muted />}
      </div>
    ) : <span className="text-muted-foreground text-xs">—</span>,
    c.view_name || c.view_file ? (
      <div className="flex flex-col gap-0.5">
        {c.view_name && <TruncCell value={c.view_name} maxW="max-w-[160px]" size="text-[0.72rem]" />}
        {c.view_file && <TruncCell value={c.view_file} maxW="max-w-[160px]" muted />}
      </div>
    ) : <span className="text-muted-foreground text-xs">—</span>,
    <VarBadges vars={c.state} />,
    <SrcCell source={c.source} root={root} />,
  ])

  return (
    <div className="flex flex-col gap-4">
      <DataSection
        title="Views"
        count={report.view_count}
        columns={[
          { key: "name",   label: "View Name" },
          { key: "kind",   label: "Kind" },
          { key: "props",  label: "Blade Props" },
          { key: "vars",   label: "Variables" },
          { key: "source", label: "Declared In" },
        ]}
        rows={viewRows}
      />
      <DataSection
        title="Blade Components"
        count={report.blade_component_count}
        columns={[
          { key: "component", label: "Component" },
          { key: "class",     label: "Class" },
          { key: "view",      label: "View" },
          { key: "props",     label: "Props" },
          { key: "source",    label: "Declared In" },
        ]}
        rows={bladeRows}
      />
      <DataSection
        title="Livewire Components"
        count={report.livewire_component_count}
        columns={[
          { key: "component", label: "Component" },
          { key: "class",     label: "Class" },
          { key: "view",      label: "View" },
          { key: "state",     label: "Public State" },
          { key: "source",    label: "Declared In" },
        ]}
        rows={livewireRows}
      />
    </div>
  )
}
