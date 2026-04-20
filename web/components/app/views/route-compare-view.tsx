"use client"

import { Badge } from "@/components/ui/badge"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { ComparedRoute, Payload, RouteComparison } from "@/lib/types"
import { IconInfoCircle } from "@tabler/icons-react"

const METHOD_COLORS: Record<string, string> = {
  GET:    "bg-blue-100   text-blue-800   dark:bg-blue-900/40   dark:text-blue-300   border-blue-200   dark:border-blue-800",
  POST:   "bg-green-100  text-green-800  dark:bg-green-900/40  dark:text-green-300  border-green-200  dark:border-green-800",
  PUT:    "bg-amber-100  text-amber-800  dark:bg-amber-900/40  dark:text-amber-300  border-amber-200  dark:border-amber-800",
  PATCH:  "bg-orange-100 text-orange-800 dark:bg-orange-900/40 dark:text-orange-300 border-orange-200 dark:border-orange-800",
  DELETE: "bg-red-100    text-red-800    dark:bg-red-900/40    dark:text-red-300    border-red-200    dark:border-red-800",
}

function MethodPill({ method }: { method: string }) {
  const c = METHOD_COLORS[method.toUpperCase()] ?? "bg-muted text-muted-foreground border-border"
  return (
    <span className={`inline-flex items-center rounded border px-1.5 py-px font-mono text-[0.62rem] font-bold leading-none tracking-wide ${c}`}>
      {method}
    </span>
  )
}

// No middleware column here
const CMP_COLS = [
  { key: "route",  label: "Route" },
  { key: "name",   label: "Name" },
  { key: "action", label: "Action" },
  { key: "source", label: "Source" },
]

function cmpRows(routes: ComparedRoute[], root?: string) {
  return routes.map((route) => [
    <div className="flex flex-col gap-1 min-w-[140px]">
      <div className="flex flex-wrap gap-0.5">{route.methods.map((m) => <MethodPill key={m} method={m} />)}</div>
      <TruncCell value={route.uri} maxW="max-w-[200px]" size="text-[0.78rem]" />
    </div>,

    route.name
      ? <TruncCell value={route.name} maxW="max-w-[140px]" />
      : <span className="text-muted-foreground text-xs">—</span>,

    route.action
      ? <TruncCell value={route.action} maxW="max-w-[180px]" />
      : <span className="text-muted-foreground text-xs">—</span>,

    route.source
      ? (() => {
          // source might be "file:line:col" from the analyzer
          const parts = route.source.match(/^(.+):(\d+):(\d+)$/)
          if (parts) return <LocCell file={parts[1]} line={Number(parts[2])} col={Number(parts[3])} root={root} />
          return <TruncCell value={route.source} maxW="max-w-[200px]" muted />
        })()
      : <span className="text-muted-foreground text-xs">—</span>,
  ])
}

export function RouteCompareView({ payload }: { payload: Payload }) {
  const c    = payload.comparison as RouteComparison
  const root = payload.root as string | undefined

  const stats = [
    { label: "Runtime",        value: c.runtime_count,       color: "" },
    { label: "Analyzer",       value: c.analyzer_count,      color: "" },
    { label: "Matched",        value: c.matched_count,       color: "text-green-600 dark:text-green-400" },
    { label: "Runtime Only",   value: c.runtime_only_count,  color: "text-amber-600 dark:text-amber-400" },
    { label: "Analyzer Only",  value: c.analyzer_only_count, color: "text-amber-600 dark:text-amber-400" },
    { label: "Status",         value: c.runnable ? "OK" : "No PHP", color: c.runnable ? "text-green-600 dark:text-green-400" : "text-muted-foreground" },
  ]

  return (
    <div className="flex flex-col gap-4">
      <Alert>
        <IconInfoCircle className="size-4" />
        <AlertDescription className="text-xs">
          {c.note}
          {c.artisan_path && <code className="ml-2 font-mono text-[0.68rem]">{c.artisan_path}</code>}
        </AlertDescription>
      </Alert>

      <div className="grid grid-cols-3 gap-2 sm:grid-cols-6">
        {stats.map(({ label, value, color }) => (
          <div key={label} className="rounded-lg border bg-card px-3 py-2">
            <p className="text-[0.6rem] font-medium uppercase tracking-wider text-muted-foreground">{label}</p>
            <p className={`mt-0.5 font-mono text-base font-semibold tabular-nums ${color}`}>{value}</p>
          </div>
        ))}
      </div>

      {c.runtime_only.length > 0 && (
        <DataSection title="Runtime Only — Missing From Analyzer" count={c.runtime_only_count} columns={CMP_COLS} rows={cmpRows(c.runtime_only, root)} />
      )}
      {c.matched.length > 0 && (
        <DataSection title="Matched Routes" count={c.matched_count} columns={CMP_COLS} rows={cmpRows(c.matched, root)} />
      )}
      {c.analyzer_only.length > 0 && (
        <DataSection title="Analyzer Only" count={c.analyzer_only_count} columns={CMP_COLS} rows={cmpRows(c.analyzer_only, root)} />
      )}
    </div>
  )
}
