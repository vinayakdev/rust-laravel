"use client"

import { useDeferredValue, useMemo, useState } from "react"

import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import type { Payload, PublicAssetEntry, PublicAssetReport } from "@/lib/types"

function formatBytes(value: number): string {
  if (!Number.isFinite(value)) return "—"
  if (value >= 1024 * 1024) return `${(value / 1024 / 1024).toFixed(2)} MB`
  if (value >= 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${value} B`
}

function fuzzyScore(candidate: string, query: string): number | null {
  if (!query) return 1
  if (candidate === query) return 10_000
  if (candidate.startsWith(query)) return 9_000 - candidate.length
  if (candidate.includes(query)) return 7_000 - candidate.length

  let queryIndex = 0
  let score = 0
  let lastMatch: number | null = null

  for (let index = 0; index < candidate.length; index += 1) {
    if (queryIndex >= query.length) break
    const ch = candidate[index]
    if (ch.toLowerCase() !== query[queryIndex].toLowerCase()) continue

    score += 10
    if (index === 0) {
      score += 20
    } else {
      const previous = candidate[index - 1]
      if (". _-/:@\\".includes(previous)) score += 18
    }
    if (lastMatch != null && index === lastMatch + 1) score += 14

    lastMatch = index
    queryIndex += 1
  }

  if (queryIndex !== query.length) return null
  return score - candidate.length
}

function assetSearchScore(asset: PublicAssetEntry, query: string): number | null {
  const haystacks = [
    asset.asset_path,
    asset.file,
    asset.extension ?? "",
    ...asset.usages.flatMap((usage) => [
      usage.raw_reference,
      usage.helper,
      usage.source_kind,
      usage.file,
      `${usage.file}:${usage.line}:${usage.column}`,
    ]),
  ]

  let best: number | null = null
  for (const haystack of haystacks) {
    const score = fuzzyScore(haystack.toLowerCase(), query)
    if (score == null) continue
    best = best == null ? score : Math.max(best, score)
  }

  return best
}

function UsageCell({
  asset,
  root,
}: {
  asset: PublicAssetEntry
  root?: string
}) {
  if (asset.usages.length === 0) {
    return <span className="text-xs text-muted-foreground">Unreferenced</span>
  }

  return (
    <div className="flex min-w-[260px] flex-col gap-1">
      {asset.usages.map((usage, index) => (
        <div key={`${usage.file}:${usage.line}:${usage.column}:${index}`} className="flex flex-col gap-0.5">
          <div className="flex flex-wrap gap-1">
            <Badge
              variant="outline"
              className="h-4 rounded-sm font-mono text-[0.6rem]"
            >
              {usage.helper}
            </Badge>
            <Badge
              variant="secondary"
              className="h-4 rounded-sm font-mono text-[0.6rem]"
            >
              {usage.source_kind}
            </Badge>
          </div>
          <LocCell
            file={usage.file}
            line={usage.line}
            col={usage.column}
            root={root}
          />
          <TruncCell value={usage.raw_reference} maxW="max-w-[260px]" muted />
        </div>
      ))}
    </div>
  )
}

export function PublicFilesView({ payload }: { payload: Payload }) {
  const report = payload.report as PublicAssetReport
  const root = payload.root as string | undefined
  const [query, setQuery] = useState("")
  const deferredQuery = useDeferredValue(query.trim().toLowerCase())

  const filteredAssets = useMemo(() => {
    if (!deferredQuery) return report.assets

    return report.assets
      .map((asset) => ({
        asset,
        score: assetSearchScore(asset, deferredQuery),
      }))
      .filter((entry) => entry.score != null)
      .sort((left, right) => {
        return (
          (right.score ?? 0) - (left.score ?? 0) ||
          right.asset.usages.length - left.asset.usages.length ||
          left.asset.asset_path.localeCompare(right.asset.asset_path)
        )
      })
      .map((entry) => entry.asset)
  }, [deferredQuery, report.assets])

  const filteredUsageCount = filteredAssets.reduce(
    (total, asset) => total + asset.usages.length,
    0
  )

  const rows = filteredAssets.map((asset) => [
    <div key="asset" className="flex min-w-[180px] flex-col gap-0.5">
      <TruncCell
        value={asset.asset_path}
        maxW="max-w-[240px]"
        size="text-[0.75rem]"
      />
      <TruncCell value={asset.file} maxW="max-w-[240px]" muted />
    </div>,
    asset.extension ? (
      <Badge
        key="ext"
        variant="secondary"
        className="h-4 w-fit rounded-sm font-mono text-[0.6rem]"
      >
        {asset.extension}
      </Badge>
    ) : (
      <span key="ext" className="text-xs text-muted-foreground">
        —
      </span>
    ),
    <span key="size" className="font-mono text-[0.78rem] tabular-nums">
      {formatBytes(asset.size_bytes)}
    </span>,
    <span key="usage-count" className="font-mono text-[0.78rem] tabular-nums">
      {asset.usages.length}
    </span>,
    <UsageCell key="used-in" asset={asset} root={root} />,
  ])

  return (
    <div className="flex flex-col gap-4">
      <div className="grid gap-4 md:grid-cols-3">
        <div className="rounded-xl border bg-card p-4">
          <p className="text-[0.65rem] tracking-wider text-muted-foreground uppercase">
            Indexed Files
          </p>
          <p className="mt-2 font-mono text-3xl font-semibold tabular-nums">
            {report.file_count}
          </p>
        </div>
        <div className="rounded-xl border bg-card p-4">
          <p className="text-[0.65rem] tracking-wider text-muted-foreground uppercase">
            Matched Usages
          </p>
          <p className="mt-2 font-mono text-3xl font-semibold tabular-nums">
            {report.usage_count}
          </p>
        </div>
        <div className="rounded-xl border bg-card p-4">
          <p className="text-[0.65rem] tracking-wider text-muted-foreground uppercase">
            Visible Results
          </p>
          <p className="mt-2 font-mono text-3xl font-semibold tabular-nums">
            {filteredAssets.length}
          </p>
        </div>
      </div>

      <div className="rounded-xl border bg-card p-4">
        <p className="text-sm font-semibold tracking-tight">Asset Search</p>
        <p className="mt-1 text-xs text-muted-foreground">
          Fuzzy match by asset path, public file, helper name, or PHP/Blade source location.
        </p>
        <Input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Search landing png, secure_asset, resources/views..."
          className="mt-3 max-w-xl"
        />
        <p className="mt-2 text-[0.7rem] text-muted-foreground">
          Showing {filteredAssets.length} files and {filteredUsageCount} matched references.
        </p>
      </div>

      <DataSection
        title="Public Files"
        count={filteredAssets.length}
        columns={[
          { key: "asset", label: "Asset" },
          { key: "ext", label: "Ext" },
          { key: "size", label: "Size" },
          { key: "usage-count", label: "Usages" },
          { key: "used-in", label: "Referenced In" },
        ]}
        rows={rows}
        note="References are matched from string literals passed to asset(...) and secure_asset(...) inside .php and .blade.php files."
      />
    </div>
  )
}
