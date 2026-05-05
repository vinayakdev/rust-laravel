"use client"

import { useState, useMemo } from "react"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import type { ClassPropsReport, ClassWithProperties, Payload } from "@/lib/types"
import { IconSearch } from "@tabler/icons-react"

export function ClassPropsView({ payload }: { payload: Payload }) {
  const report = payload.report as ClassPropsReport
  const [search, setSearch] = useState("")

  const filtered = useMemo(() => {
    const q = search.toLowerCase()
    if (!q) return report.classes
    return report.classes.filter(
      (c) =>
        c.class_fqn.toLowerCase().includes(q) ||
        c.parent_fqn.toLowerCase().includes(q) ||
        c.properties.some((p) => p.name.toLowerCase().includes(q))
    )
  }, [search, report.classes])

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">Class Properties</h2>
        <Badge variant="secondary" className="font-mono text-xs">
          {filtered.length.toLocaleString()} /{" "}
          {report.class_count.toLocaleString()}
        </Badge>
      </div>

      <div className="relative">
        <IconSearch className="absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          placeholder="Search by class, parent, or property name…"
          className="h-8 pl-8 font-mono text-xs"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          autoFocus
        />
      </div>

      <div className="flex flex-col gap-3">
        {filtered.length === 0 && (
          <div className="py-10 text-center text-sm text-muted-foreground">
            No results match &ldquo;{search}&rdquo;
          </div>
        )}
        {filtered.slice(0, 200).map((entry) => (
          <ClassCard key={entry.class_fqn} entry={entry} />
        ))}
        {filtered.length > 200 && (
          <div className="py-2 text-center text-xs text-muted-foreground">
            Showing first 200 — refine your search to narrow results
          </div>
        )}
      </div>
    </div>
  )
}

function ClassCard({ entry }: { entry: ClassWithProperties }) {
  const classParts = entry.class_fqn.split("\\")
  const className = classParts.pop() ?? entry.class_fqn
  const classNs = classParts.join("\\")

  const parentParts = entry.parent_fqn.split("\\")
  const parentName = parentParts.pop() ?? entry.parent_fqn

  const grouped = useMemo(() => {
    const map = new Map<string, string[]>()
    for (const p of entry.properties) {
      const list = map.get(p.source_class) ?? []
      list.push(p.name)
      map.set(p.source_class, list)
    }
    return map
  }, [entry.properties])

  return (
    <div className="rounded-md border">
      <div className="border-b bg-muted/40 px-3 py-2">
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-xs font-semibold">{className}</span>
          {classNs && (
            <span className="font-mono text-[0.65rem] text-muted-foreground truncate">
              {classNs}
            </span>
          )}
          <span className="ml-auto font-mono text-[0.65rem] text-muted-foreground shrink-0">
            extends{" "}
            <span className="text-foreground">{parentName}</span>
          </span>
        </div>
      </div>

      <div className="flex flex-col divide-y">
        {Array.from(grouped.entries()).map(([source, props]) => (
          <div key={source} className="px-3 py-2">
            <div className="mb-1.5 font-mono text-[0.65rem] text-muted-foreground">
              from {source}
            </div>
            <div className="flex flex-wrap gap-1.5">
              {props.map((name) => (
                <Badge
                  key={name}
                  variant="secondary"
                  className="h-5 rounded-sm font-mono text-[0.65rem]"
                >
                  ${name}
                </Badge>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
