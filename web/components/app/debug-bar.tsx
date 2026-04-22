"use client"

import type { DebugInfo } from "@/lib/types"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import {
  IconArrowsUpDown,
  IconClock,
  IconFiles,
  IconServer,
} from "@tabler/icons-react"

function formatKb(v: number | null | undefined): string | null {
  if (v == null || !Number.isFinite(v)) return null
  const a = Math.abs(v)
  if (a >= 1024 * 1024) return `${(a / 1024 / 1024).toFixed(2)} GB`
  if (a >= 1024) return `${(a / 1024).toFixed(1)} MB`
  return `${v} KB`
}

type Stat = {
  icon: React.ComponentType<{ className?: string }>
  label: string
  value: string
}

export function DebugBar({ debug }: { debug?: DebugInfo }) {
  if (!debug) return null

  const stats: Stat[] = [
    {
      icon: IconFiles,
      label: "Files parsed",
      value: String(debug.parsed_file_count ?? 0),
    },
    {
      icon: IconClock,
      label: "Analysis time",
      value: `${debug.duration_ms ?? 0} ms`,
    },
  ]
  if (debug.rss_after_kb != null)
    stats.push({
      icon: IconServer,
      label: "RSS after",
      value: formatKb(debug.rss_after_kb) ?? `${debug.rss_after_kb} KB`,
    })
  if (debug.rss_before_kb != null && debug.rss_after_kb != null) {
    const d = debug.rss_after_kb - debug.rss_before_kb
    stats.push({
      icon: IconArrowsUpDown,
      label: "RSS delta",
      value: `${d > 0 ? "+" : ""}${formatKb(d) ?? `${d} KB`}`,
    })
  }

  return (
    <div className="flex items-center gap-3">
      {stats.map(({ icon: Icon, label, value }) => (
        <Tooltip key={label}>
          <TooltipTrigger asChild>
            <div className="flex cursor-default items-center gap-1 text-muted-foreground">
              <Icon className="size-3 shrink-0" />
              <span className="font-mono text-[0.7rem] tabular-nums">
                {value}
              </span>
            </div>
          </TooltipTrigger>
          <TooltipContent side="bottom" className="text-xs">
            {label}
          </TooltipContent>
        </Tooltip>
      ))}
    </div>
  )
}
