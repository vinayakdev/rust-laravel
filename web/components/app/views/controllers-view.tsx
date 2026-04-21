"use client"

import { Badge } from "@/components/ui/badge"
import { DataSection } from "@/components/app/data-section"
import { LocCell, TruncCell } from "@/components/app/cell"
import type { ControllerReport, Payload } from "@/lib/types"

function MethodBadge({
  label,
  callable,
}: {
  label: string
  callable: boolean
}) {
  return (
    <Badge
      variant={callable ? "default" : "secondary"}
      className="h-5 rounded-sm font-mono text-[0.6rem]"
    >
      {label}
    </Badge>
  )
}

export function ControllersView({ payload }: { payload: Payload }) {
  const report = payload.report as ControllerReport
  const root = payload.root as string | undefined

  const rows = report.controllers.map((controller) => {
    const callable = controller.methods.filter((method) => method.accessible_from_route)
    const blocked = controller.methods.filter((method) => !method.accessible_from_route)

    return [
      <div className="flex min-w-[220px] flex-col gap-1">
        <TruncCell value={controller.fqn} maxW="max-w-[260px]" />
        <LocCell file={controller.file} line={controller.line} col={1} root={root} />
      </div>,

      <div className="flex min-w-[180px] flex-col gap-1">
        {controller.extends ? (
          <TruncCell value={controller.extends} maxW="max-w-[220px]" muted />
        ) : (
          <span className="text-xs text-muted-foreground">No parent</span>
        )}
        <div className="flex flex-wrap gap-1">
          {controller.traits.length ? (
            controller.traits.map((traitName) => (
              <Badge key={traitName} variant="outline" className="h-4 rounded-sm text-[0.6rem]">
                {traitName.split("\\").pop()}
              </Badge>
            ))
          ) : (
            <span className="text-xs text-muted-foreground">No traits</span>
          )}
        </div>
      </div>,

      <div className="flex min-w-[220px] flex-wrap gap-1">
        {callable.length ? (
          callable.map((method) => (
            <MethodBadge key={`${controller.fqn}:${method.name}`} label={method.name} callable />
          ))
        ) : (
          <span className="text-xs text-muted-foreground">No public route-callable methods</span>
        )}
      </div>,

      <div className="flex min-w-[280px] flex-wrap gap-1">
        {blocked.length ? (
          blocked.map((method) => (
            <MethodBadge
              key={`${controller.fqn}:${method.name}:blocked`}
              label={`${method.name} (${method.accessibility})`}
              callable={false}
            />
          ))
        ) : (
          <span className="text-xs text-muted-foreground">No blocked methods</span>
        )}
      </div>,
    ]
  })

  return (
    <DataSection
      title="Controllers"
      count={report.controller_count}
      columns={[
        { key: "controller", label: "Controller" },
        { key: "shape", label: "Extends / Traits" },
        { key: "callable", label: "Route Callable" },
        { key: "blocked", label: "Not Route Callable" },
      ]}
      rows={rows}
      note="Route-callable means a public non-static method. Constructors, non-public methods, static methods, and magic methods other than __invoke are flagged."
    />
  )
}
