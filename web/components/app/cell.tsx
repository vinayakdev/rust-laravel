"use client"

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

/** Strip the project root prefix to show a relative path. */
export function relPath(full: string, root?: string): string {
  if (!root) return full
  const r = root.endsWith("/") ? root : root + "/"
  return full.startsWith(r) ? full.slice(r.length) : full
}

type TruncProps = {
  value: string
  maxW?: string
  mono?: boolean
  muted?: boolean
  size?: string
}

/**
 * Truncated text cell. On hover shows full value in a tooltip.
 */
export function TruncCell({
  value,
  maxW = "max-w-[220px]",
  mono = true,
  muted = false,
  size = "text-[0.72rem]",
}: TruncProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
          className={[
            "block truncate cursor-default",
            maxW,
            size,
            mono ? "font-mono" : "",
            muted ? "text-muted-foreground" : "",
          ].join(" ")}
        >
          {value}
        </span>
      </TooltipTrigger>
      <TooltipContent
        side="bottom"
        className="max-w-[600px] break-all font-mono text-xs"
      >
        {value}
      </TooltipContent>
    </Tooltip>
  )
}

/** loc = "file:line:col" — strips root prefix and shows full on hover. */
export function LocCell({ file, line, col, root }: { file: string; line: number; col: number; root?: string }) {
  const rel  = relPath(file, root)
  const full = `${file}:${line}:${col}`
  const disp = `${rel}:${line}:${col}`
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <code className="block max-w-[240px] truncate cursor-default font-mono text-[0.68rem] text-muted-foreground">
          {disp}
        </code>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="max-w-[600px] break-all font-mono text-xs">
        {full}
      </TooltipContent>
    </Tooltip>
  )
}
