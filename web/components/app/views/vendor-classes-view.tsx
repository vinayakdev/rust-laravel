"use client"

import { useState, useMemo } from "react"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Card, CardContent } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { Alert, AlertDescription } from "@/components/ui/alert"
import {
  IconAlertTriangle,
  IconArrowLeft,
  IconChevronRight,
  IconSearch,
} from "@tabler/icons-react"
import type {
  Payload,
  VendorClass,
  VendorClassDetail,
  VendorClassReport,
  VendorProperty,
} from "@/lib/types"

export function VendorClassesView({ payload }: { payload: Payload }) {
  const report = payload.report as VendorClassReport
  const [search, setSearch] = useState("")
  const [selected, setSelected] = useState<VendorClass | null>(null)
  const [detail, setDetail] = useState<VendorClassDetail | null>(null)
  const [loadState, setLoadState] = useState<"idle" | "loading" | "error">(
    "idle"
  )
  const [error, setError] = useState<string | null>(null)

  const filtered = useMemo(() => {
    const q = search.toLowerCase()
    if (!q) return report.classes
    return report.classes.filter((c) => c.fqn.toLowerCase().includes(q))
  }, [search, report.classes])

  async function openClass(cls: VendorClass) {
    setSelected(cls)
    setDetail(null)
    setLoadState("loading")
    setError(null)
    try {
      const params = new URLSearchParams({
        project: payload.root,
        class: cls.fqn,
      })
      const res = await fetch(`/api/vendor-class?${params}`)
      if (!res.ok) {
        const data = await res.json()
        setError(data.error ?? "Request failed")
        setLoadState("error")
        return
      }
      const data: VendorClassDetail = await res.json()
      setDetail(data)
      setLoadState("idle")
    } catch (err) {
      setError(err instanceof Error ? err.message : "Request failed")
      setLoadState("error")
    }
  }

  if (selected) {
    return (
      <DetailPanel
        cls={selected}
        detail={detail}
        loadState={loadState}
        error={error}
        onBack={() => {
          setSelected(null)
          setDetail(null)
          setLoadState("idle")
        }}
      />
    )
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <h2 className="text-base font-semibold">Vendor Classes</h2>
        <Badge variant="secondary" className="font-mono text-xs">
          {filtered.length.toLocaleString()} /{" "}
          {report.class_count.toLocaleString()}
        </Badge>
      </div>

      <div className="relative">
        <IconSearch className="absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          placeholder="Search by class name or namespace…"
          className="h-8 pl-8 font-mono text-xs"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          autoFocus
        />
      </div>

      <div className="flex flex-col divide-y rounded-md border">
        {filtered.length === 0 && (
          <div className="py-10 text-center text-sm text-muted-foreground">
            No classes match &ldquo;{search}&rdquo;
          </div>
        )}
        {filtered.slice(0, 500).map((cls) => (
          <ClassRow key={cls.fqn} cls={cls} onClick={() => openClass(cls)} />
        ))}
        {filtered.length > 500 && (
          <div className="px-4 py-2 text-center text-xs text-muted-foreground">
            Showing first 500 — refine your search to narrow results
          </div>
        )}
      </div>
    </div>
  )
}

function ClassRow({
  cls,
  onClick,
}: {
  cls: VendorClass
  onClick: () => void
}) {
  const parts = cls.fqn.split("\\")
  const name = parts.pop() ?? cls.fqn
  const ns = parts.join("\\")

  return (
    <button
      onClick={onClick}
      className="flex w-full items-center justify-between px-3 py-2 text-left transition-colors hover:bg-muted/50"
    >
      <div className="min-w-0">
        <span className="block font-mono text-xs font-semibold">{name}</span>
        {ns && (
          <span className="block truncate font-mono text-[0.65rem] text-muted-foreground">
            {ns}
          </span>
        )}
      </div>
      <IconChevronRight className="ml-2 size-3.5 shrink-0 text-muted-foreground" />
    </button>
  )
}

function DetailPanel({
  cls,
  detail,
  loadState,
  error,
  onBack,
}: {
  cls: VendorClass
  detail: VendorClassDetail | null
  loadState: "idle" | "loading" | "error"
  error: string | null
  onBack: () => void
}) {
  const [search, setSearch] = useState("")

  const filteredMethods = useMemo(() => {
    if (!detail) return []
    const q = search.toLowerCase()
    if (!q) return detail.methods
    return detail.methods.filter(
      (m) =>
        m.name.toLowerCase().includes(q) ||
        m.source.toLowerCase().includes(q)
    )
  }, [detail, search])

  const groupedMethods = useMemo(() => {
    const map = new Map<string, string[]>()
    for (const m of filteredMethods) {
      const list = map.get(m.source) ?? []
      list.push(m.name)
      map.set(m.source, list)
    }
    return map
  }, [filteredMethods])

  const filteredProperties = useMemo(() => {
    if (!detail) return []
    const q = search.toLowerCase()
    if (!q) return detail.properties
    return detail.properties.filter(
      (p) =>
        p.name.toLowerCase().includes(q) ||
        p.source.toLowerCase().includes(q)
    )
  }, [detail, search])

  const groupedProperties = useMemo(() => {
    const map = new Map<string, string[]>()
    for (const p of filteredProperties) {
      const list = map.get(p.source) ?? []
      list.push(p.name)
      map.set(p.source, list)
    }
    return map
  }, [filteredProperties])

  const parts = cls.fqn.split("\\")
  const className = parts.pop() ?? cls.fqn
  const ns = parts.join("\\")

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <button
          onClick={onBack}
          className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          <IconArrowLeft className="size-3.5" />
          Back
        </button>
        <span className="text-muted-foreground">/</span>
        <span className="font-mono text-sm font-semibold">{className}</span>
        {ns && (
          <span className="truncate font-mono text-xs text-muted-foreground">
            {ns}
          </span>
        )}
      </div>

      <code className="block truncate rounded bg-muted px-2 py-1 font-mono text-[0.65rem] text-muted-foreground">
        {cls.file}
      </code>

      {loadState === "loading" && (
        <div className="flex flex-col gap-2">
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-6 w-3/4" />
          <Skeleton className="h-6 w-1/2" />
        </div>
      )}

      {loadState === "error" && (
        <Alert variant="destructive">
          <IconAlertTriangle className="size-4" />
          <AlertDescription className="font-mono text-xs">
            {error}
          </AlertDescription>
        </Alert>
      )}

      {detail && (
        <>
          <div className="relative">
            <IconSearch className="absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              placeholder="Filter methods, properties, or source…"
              className="h-8 pl-8 font-mono text-xs"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              autoFocus
            />
          </div>

          {detail.properties.length > 0 && (
            <>
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-semibold">Properties</h3>
                <Badge variant="secondary" className="font-mono text-xs">
                  {filteredProperties.length} / {detail.properties.length}
                </Badge>
              </div>

              <div className="flex flex-col gap-3">
                {groupedProperties.size === 0 && search && (
                  <Card>
                    <CardContent className="py-4 text-center text-sm text-muted-foreground">
                      No properties match &ldquo;{search}&rdquo;
                    </CardContent>
                  </Card>
                )}
                {Array.from(groupedProperties.entries()).map(([source, props]) => (
                  <div key={source} className="rounded-md border">
                    <div className="border-b bg-muted/40 px-3 py-1.5">
                      <span className="font-mono text-[0.65rem] font-semibold text-muted-foreground">
                        {source}
                      </span>
                      <Badge
                        variant="outline"
                        className="ml-2 h-4 rounded-sm font-mono text-[0.6rem]"
                      >
                        {props.length}
                      </Badge>
                    </div>
                    <div className="flex flex-wrap gap-1.5 p-2">
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
            </>
          )}

          {detail.methods.length > 0 && (
            <>
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-semibold">Chainable Methods</h3>
                <Badge variant="secondary" className="font-mono text-xs">
                  {filteredMethods.length} / {detail.methods.length}
                </Badge>
              </div>

              <div className="flex flex-col gap-3">
                {groupedMethods.size === 0 && search && (
                  <Card>
                    <CardContent className="py-4 text-center text-sm text-muted-foreground">
                      No methods match &ldquo;{search}&rdquo;
                    </CardContent>
                  </Card>
                )}
                {Array.from(groupedMethods.entries()).map(([source, methods]) => (
                  <div key={source} className="rounded-md border">
                    <div className="border-b bg-muted/40 px-3 py-1.5">
                      <span className="font-mono text-[0.65rem] font-semibold text-muted-foreground">
                        {source}
                      </span>
                      <Badge
                        variant="outline"
                        className="ml-2 h-4 rounded-sm font-mono text-[0.6rem]"
                      >
                        {methods.length}
                      </Badge>
                    </div>
                    <div className="flex flex-wrap gap-1.5 p-2">
                      {methods.map((name) => (
                        <Badge
                          key={name}
                          variant="secondary"
                          className="h-5 rounded-sm font-mono text-[0.65rem]"
                        >
                          -{`>`}{name}()
                        </Badge>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}

          {detail.properties.length === 0 && detail.methods.length === 0 && (
            <Card>
              <CardContent className="py-8 text-center text-sm text-muted-foreground">
                No properties or chainable methods found
              </CardContent>
            </Card>
          )}
        </>
      )}
    </div>
  )
}
