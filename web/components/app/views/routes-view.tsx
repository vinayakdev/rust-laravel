"use client"

import { Badge } from "@/components/ui/badge"
import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { Payload, RoutesReport } from "@/lib/types"

const METHOD_COLORS: Record<string, string> = {
  GET:    "bg-blue-100   text-blue-800   dark:bg-blue-900/40   dark:text-blue-300   border-blue-200   dark:border-blue-800",
  POST:   "bg-green-100  text-green-800  dark:bg-green-900/40  dark:text-green-300  border-green-200  dark:border-green-800",
  PUT:    "bg-amber-100  text-amber-800  dark:bg-amber-900/40  dark:text-amber-300  border-amber-200  dark:border-amber-800",
  PATCH:  "bg-orange-100 text-orange-800 dark:bg-orange-900/40 dark:text-orange-300 border-orange-200 dark:border-orange-800",
  DELETE: "bg-red-100    text-red-800    dark:bg-red-900/40    dark:text-red-300    border-red-200    dark:border-red-800",
  ANY:    "bg-purple-100 text-purple-800 dark:bg-purple-900/40 dark:text-purple-300 border-purple-200 dark:border-purple-800",
}

function MethodPill({ method }: { method: string }) {
  const c = METHOD_COLORS[method.toUpperCase()] ?? "bg-muted text-muted-foreground border-border"
  return (
    <span className={`inline-flex items-center rounded border px-1.5 py-px font-mono text-[0.62rem] font-bold leading-none tracking-wide ${c}`}>
      {method}
    </span>
  )
}

const COLS = [
  { key: "route",      label: "Route" },
  { key: "location",   label: "Location" },
  { key: "name",       label: "Name" },
  { key: "action",     label: "Action" },
  { key: "middleware", label: "Middleware" },
  { key: "patterns",   label: "Patterns" },
  { key: "registered", label: "Registered Via" },
]

export function RoutesView({ payload, sourceMode }: { payload: Payload; sourceMode: boolean }) {
  const report = payload.report as RoutesReport
  const root   = payload.root as string | undefined

  const rows = report.routes.map((route) => {
    const mw       = route.resolved_middleware.length ? route.resolved_middleware : route.middleware
    const patterns = Object.entries(route.parameter_patterns ?? {})

    return [
      /* Route */
      <div className="flex flex-col gap-1 min-w-[140px]">
        <div className="flex flex-wrap gap-0.5">
          {route.methods.map((m) => <MethodPill key={m} method={m} />)}
        </div>
        <TruncCell value={route.uri} maxW="max-w-[200px]" size="text-[0.78rem]" />
      </div>,

      /* Location */
      <LocCell file={route.file} line={route.line} col={route.column} root={root} />,

      /* Name */
      route.name
        ? <TruncCell value={route.name} maxW="max-w-[140px]" />
        : <span className="text-muted-foreground text-xs">—</span>,

      /* Action */
      route.action
        ? <TruncCell value={route.action} maxW="max-w-[180px]" />
        : <span className="text-muted-foreground text-xs">—</span>,

      /* Middleware */
      mw.length ? (
        <div className="flex flex-wrap gap-0.5 min-w-[120px]">
          {mw.map((m, i) => (
            <Badge key={i} variant="outline" className="h-4 rounded-sm font-mono text-[0.6rem]">{m}</Badge>
          ))}
        </div>
      ) : <span className="text-muted-foreground text-xs">—</span>,

      /* Patterns */
      patterns.length ? (
        <div className="flex flex-wrap gap-0.5">
          {patterns.map(([k, v]) => (
            <Badge key={k} variant="secondary" className="h-4 rounded-sm font-mono text-[0.6rem]">{k}={v}</Badge>
          ))}
        </div>
      ) : <span className="text-muted-foreground text-xs">—</span>,

      /* Registered Via */
      <div className="flex flex-col gap-0.5 min-w-[140px]">
        <span className="text-[0.72rem] font-medium">{route.registration.kind}</span>
        <LocCell file={route.registration.declared_in} line={route.registration.line} col={route.registration.column} root={root} />
        {route.registration.provider_class && (
          <TruncCell value={route.registration.provider_class} maxW="max-w-[180px]" muted />
        )}
      </div>,
    ]
  })

  return (
    <DataSection
      title={sourceMode ? "Registered Route Sources" : "Effective Routes"}
      count={report.route_count}
      columns={COLS}
      rows={rows}
    />
  )
}
