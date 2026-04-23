"use client"

import { useCallback, useEffect, useRef, useState } from "react"
import { Card, CardContent } from "@/components/ui/card"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Separator } from "@/components/ui/separator"
import { Skeleton } from "@/components/ui/skeleton"
import { Alert, AlertDescription } from "@/components/ui/alert"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import { DebugBar } from "@/components/app/debug-bar"
import { RoutesView } from "@/components/app/views/routes-view"
import { RouteCompareView } from "@/components/app/views/route-compare-view"
import { MiddlewareView } from "@/components/app/views/middleware-view"
import { ConfigView } from "@/components/app/views/config-view"
import { ControllersView } from "@/components/app/views/controllers-view"
import { ProvidersView } from "@/components/app/views/providers-view"
import { ViewsView } from "@/components/app/views/views-view"
import { ModelsView } from "@/components/app/views/models-view"
import { MigrationsView } from "@/components/app/views/migrations-view"
import { DashboardView } from "@/components/app/views/dashboard-view"
import type { CommandId, Payload, Project } from "@/lib/types"
import {
  IconAlertTriangle,
  IconBox,
  IconBracketsContain,
  IconBrandLaravel,
  IconDatabase,
  IconEye,
  IconGitCompare,
  IconLoader2,
  IconRefresh,
  IconRoute,
  IconSettings,
  IconShieldLock,
  IconTable,
  IconTimeline,
} from "@tabler/icons-react"
import { cn } from "@/lib/utils"

type CommandDef = {
  id: CommandId
  label: string
  description: string
  icon: React.ComponentType<{ className?: string }>
  group: string
}

const COMMANDS: CommandDef[] = [
  {
    id: "dashboard",
    label: "Dashboard",
    description: "Autocomplete counts by feature",
    icon: IconTimeline,
    group: "Overview",
  },
  {
    id: "route:list",
    label: "Routes",
    description: "Resolved route table with middleware and patterns",
    icon: IconRoute,
    group: "Routing",
  },
  {
    id: "route:compare",
    label: "Route Compare",
    description: "Compare analyzer vs artisan runtime output",
    icon: IconGitCompare,
    group: "Routing",
  },
  {
    id: "route:sources",
    label: "Route Sources",
    description: "Route registration origin attribution",
    icon: IconTimeline,
    group: "Routing",
  },
  {
    id: "middleware:list",
    label: "Middleware",
    description: "Aliases, groups, and route parameter patterns",
    icon: IconShieldLock,
    group: "Routing",
  },
  {
    id: "config:list",
    label: "Config",
    description: "Collected config values and env defaults",
    icon: IconSettings,
    group: "Application",
  },
  {
    id: "config:sources",
    label: "Config Sources",
    description: "Config keys with provider and origin",
    icon: IconSettings,
    group: "Application",
  },
  {
    id: "controller:list",
    label: "Controllers",
    description:
      "Controller methods, traits, inheritance, and route callability",
    icon: IconBracketsContain,
    group: "Application",
  },
  {
    id: "provider:list",
    label: "Providers",
    description: "Service provider registration inventory",
    icon: IconBox,
    group: "Application",
  },
  {
    id: "view:list",
    label: "Views",
    description: "Blade views, components, and Livewire",
    icon: IconEye,
    group: "Application",
  },
  {
    id: "model:list",
    label: "Models",
    description: "Eloquent model inventory",
    icon: IconDatabase,
    group: "Data",
  },
  {
    id: "migration:list",
    label: "Migrations",
    description: "Database migration files and status",
    icon: IconTable,
    group: "Data",
  },
]

const GROUPS = ["Overview", "Routing", "Application", "Data"]
type LoadState = "idle" | "loading" | "error" | "done"

function isCommandId(value: string | null): value is CommandId {
  return COMMANDS.some((command) => command.id === value)
}

function initialCommand(): CommandId {
  if (typeof window === "undefined") return "dashboard"
  const value = new URLSearchParams(window.location.search).get("command")
  return isCommandId(value) ? value : "dashboard"
}

function initialProject(): string {
  if (typeof window === "undefined") return ""
  return new URLSearchParams(window.location.search).get("project") ?? ""
}

function compactProjectPath(root: string): string {
  const parts = root.split("/").filter(Boolean)
  const markerIndex = parts.findIndex(
    (part) => part === "laravel-example" || part === "test"
  )
  if (markerIndex >= 0) return parts.slice(markerIndex).join("/")
  if (parts.length <= 2) return root
  return parts.slice(-2).join("/")
}

export default function Page() {
  const [projects, setProjects] = useState<Project[]>([])
  const [selectedProject, setSelectedProject] = useState<string>(initialProject)
  const [selectedCommand, setSelectedCommand] =
    useState<CommandId>(initialCommand)
  const [payload, setPayload] = useState<Payload | null>(null)
  const [loadState, setLoadState] = useState<LoadState>("idle")
  const [error, setError] = useState<string | null>(null)
  const abortRef = useRef<AbortController | null>(null)

  const loadProjects = useCallback(async () => {
    try {
      const res = await fetch("/api/projects")
      const data: Project[] = await res.json()
      setProjects(data)
      if (data.length > 0) {
        setSelectedProject((current) => {
          if (current && data.some((project) => project.id === current))
            return current
          return data[0].id
        })
      }
      return data
    } catch {
      return []
    }
  }, [])

  const loadReport = useCallback(
    async (projectId: string, command: CommandId) => {
      if (!projectId) return
      abortRef.current?.abort()
      const ctrl = new AbortController()
      abortRef.current = ctrl
      setLoadState("loading")
      setError(null)
      setPayload(null)
      try {
        const params = new URLSearchParams({ project: projectId, command })
        const res = await fetch(`/api/report?${params}`, {
          signal: ctrl.signal,
        })
        const data: Payload = await res.json()
        if (!res.ok) {
          setError(data.error ?? "Unknown error")
          setLoadState("error")
          return
        }
        setPayload(data)
        setLoadState("done")
      } catch (err: unknown) {
        if (err instanceof Error && err.name === "AbortError") return
        setError(err instanceof Error ? err.message : "Request failed")
        setLoadState("error")
      }
    },
    []
  )

  useEffect(() => {
    loadProjects()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  useEffect(() => {
    if (selectedProject) loadReport(selectedProject, selectedCommand)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedProject, selectedCommand])

  useEffect(() => {
    if (typeof window === "undefined" || !selectedProject) return
    const params = new URLSearchParams(window.location.search)
    params.set("project", selectedProject)
    params.set("command", selectedCommand)
    const query = params.toString()
    const nextUrl = query
      ? `${window.location.pathname}?${query}`
      : window.location.pathname
    window.history.replaceState(null, "", nextUrl)
  }, [selectedProject, selectedCommand])

  const activeProject = projects.find((p) => p.id === selectedProject)
  const activeCommand = COMMANDS.find((c) => c.id === selectedCommand)
  const activeProjectPath = activeProject
    ? compactProjectPath(activeProject.root)
    : ""

  return (
    // h-screen + overflow-hidden pins the shell to viewport
    <SidebarProvider style={{ height: "100svh", overflow: "hidden" }}>
      <Sidebar variant="inset">
        <SidebarHeader className="border-b px-4 py-3">
          <div className="flex items-center gap-3">
            <div className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground">
              <IconBrandLaravel className="size-4" />
            </div>
            <div className="min-w-0">
              <p className="text-sm leading-none font-semibold">rust-php</p>
              <p className="mt-0.5 text-[0.65rem] text-muted-foreground">
                Laravel Static Analyzer
              </p>
            </div>
          </div>
        </SidebarHeader>

        <div className="border-b px-3 py-3">
          <p className="mb-1.5 text-[0.65rem] font-medium tracking-wider text-muted-foreground uppercase">
            Project
          </p>
          <Select value={selectedProject} onValueChange={setSelectedProject}>
            <SelectTrigger className="h-8 w-full text-xs">
              <SelectValue placeholder="Select project…" />
            </SelectTrigger>
            <SelectContent>
              <SelectGroup>
                {projects.map((p) => (
                  <SelectItem key={p.id} value={p.id} className="text-xs">
                    {p.name}
                  </SelectItem>
                ))}
              </SelectGroup>
            </SelectContent>
          </Select>
          <button
            onClick={() => loadProjects()}
            className="mt-2 flex items-center gap-1.5 text-[0.65rem] text-muted-foreground transition-colors hover:text-foreground"
          >
            <IconRefresh className="size-3" /> Refresh projects
          </button>
        </div>

        <SidebarContent>
          {GROUPS.map((group) => (
            <SidebarGroup key={group}>
              <SidebarGroupLabel>{group}</SidebarGroupLabel>
              <SidebarGroupContent>
                <SidebarMenu>
                  {COMMANDS.filter((c) => c.group === group).map((cmd) => (
                    <SidebarMenuItem key={cmd.id}>
                      <SidebarMenuButton
                        isActive={selectedCommand === cmd.id}
                        onClick={() => setSelectedCommand(cmd.id)}
                        tooltip={cmd.description}
                        className="cursor-pointer"
                      >
                        <cmd.icon />
                        <span>{cmd.label}</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  ))}
                </SidebarMenu>
              </SidebarGroupContent>
            </SidebarGroup>
          ))}
        </SidebarContent>

        <SidebarFooter className="border-t px-3 py-2">
          {activeProject && (
            <div className="min-w-0">
              <p className="text-[0.6rem] tracking-wider text-muted-foreground uppercase">
                Active path
              </p>
              <code className="mt-0.5 block truncate font-mono text-[0.65rem] text-muted-foreground">
                {activeProjectPath}
              </code>
            </div>
          )}
        </SidebarFooter>
      </Sidebar>

      {/* SidebarInset: minHeight overridden so it doesn't grow past viewport */}
      <SidebarInset
        style={{ minHeight: 0, overflow: "hidden" }}
        className="flex flex-col"
      >
        {/* Fixed toolbar */}
        <header className="flex h-11 shrink-0 items-center justify-between gap-4 border-b bg-background px-4">
          <div className="flex items-center gap-2.5">
            <SidebarTrigger className="-ml-1" />
            <Separator orientation="vertical" className="h-4" />
            <div>
              <p className="text-[0.6rem] leading-none tracking-widest text-muted-foreground uppercase">
                Project
              </p>
              <p className="mt-0.5 text-sm leading-none font-semibold">
                {activeProject?.name ?? "—"}
              </p>
            </div>
            {activeProject && (
              <>
                <Separator
                  orientation="vertical"
                  className="hidden h-4 sm:block"
                />
                <div className="hidden min-w-0 sm:block">
                  <p className="text-[0.6rem] leading-none tracking-widest text-muted-foreground uppercase">
                    Path
                  </p>
                  <code className="mt-0.5 block max-w-[320px] truncate font-mono text-[0.65rem] text-muted-foreground">
                    {activeProjectPath}
                  </code>
                </div>
              </>
            )}
            {activeCommand && (
              <>
                <Separator orientation="vertical" className="h-4" />
                <div>
                  <p className="text-[0.6rem] leading-none tracking-widest text-muted-foreground uppercase">
                    Analyzer
                  </p>
                  <p className="mt-0.5 text-sm leading-none font-semibold">
                    {activeCommand.label}
                  </p>
                </div>
              </>
            )}
          </div>

          <div className="flex shrink-0 items-center gap-3">
            {payload?.debug && <DebugBar debug={payload.debug} />}
            <Separator orientation="vertical" className="h-4" />
            <span
              className={cn(
                "flex items-center gap-1 text-xs font-medium",
                loadState === "error"
                  ? "text-destructive"
                  : "text-muted-foreground"
              )}
            >
              {loadState === "loading" && (
                <IconLoader2 className="size-3 animate-spin" />
              )}
              {loadState === "loading"
                ? "Running…"
                : loadState === "error"
                  ? "Error"
                  : loadState === "done"
                    ? "Ready"
                    : "Idle"}
            </span>
          </div>
        </header>

        {/* Scrollable content area */}
        <div className="flex-1 overflow-y-auto p-4">
          {loadState === "loading" && (
            <div className="flex flex-col gap-2">
              <Skeleton className="h-8 w-48" />
              <Skeleton className="h-px w-full" />
              <Skeleton className="h-7 w-full" />
              <Skeleton className="h-7 w-full" />
              <Skeleton className="h-7 w-3/4" />
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
          {loadState === "idle" && (
            <Card>
              <CardContent className="py-16 text-center text-sm text-muted-foreground">
                Select a project and an analyzer from the sidebar.
              </CardContent>
            </Card>
          )}
          {loadState === "done" && payload && (
            <ReportView
              key={`${payload.project}:${payload.command}`}
              payload={payload}
            />
          )}
        </div>
      </SidebarInset>
    </SidebarProvider>
  )
}

function ReportView({ payload }: { payload: Payload }) {
  switch (payload.command) {
    case "dashboard":
      return <DashboardView payload={payload} />
    case "route:list":
      return <RoutesView payload={payload} sourceMode={false} />
    case "route:sources":
      return <RoutesView payload={payload} sourceMode={true} />
    case "route:compare":
      return <RouteCompareView payload={payload} />
    case "middleware:list":
      return <MiddlewareView payload={payload} />
    case "config:list":
      return <ConfigView payload={payload} sourceMode={false} />
    case "config:sources":
      return <ConfigView payload={payload} sourceMode={true} />
    case "controller:list":
      return <ControllersView payload={payload} />
    case "provider:list":
      return <ProvidersView payload={payload} />
    case "view:list":
      return <ViewsView payload={payload} />
    case "model:list":
      return <ModelsView payload={payload} />
    case "migration:list":
      return <MigrationsView payload={payload} />
    default:
      return (
        <Card>
          <CardContent className="py-8 text-center font-mono text-xs text-muted-foreground">
            No renderer for: {payload.command}
          </CardContent>
        </Card>
      )
  }
}
