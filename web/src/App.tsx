import { useEffect, useMemo, useState } from "react";
import { FolderTree, Gauge, LayoutPanelLeft, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";

type CommandId =
  | "route:list"
  | "route:compare"
  | "route:sources"
  | "middleware:list"
  | "config:list"
  | "config:sources"
  | "provider:list"
  | "view:list";

type Project = {
  id: string;
  name: string;
  root: string;
};

type DebugInfo = {
  duration_ms?: number;
  parsed_file_count?: number;
  rss_before_kb?: number | null;
  rss_after_kb?: number | null;
};

type Payload = {
  project: string;
  root: string;
  command: CommandId;
  debug?: DebugInfo;
  report?: any;
  comparison?: any;
  error?: string;
};

const COMMANDS: { id: CommandId; label: string; description: string }[] = [
  { id: "route:list", label: "Routes", description: "Resolved route table with middleware and patterns." },
  { id: "route:compare", label: "Route Compare", description: "Compare Rust analyzer routes against artisan runtime output." },
  { id: "route:sources", label: "Route Sources", description: "Same route inventory, focused on registration origin." },
  { id: "middleware:list", label: "Middleware", description: "Aliases, groups, and route parameter patterns." },
  { id: "config:list", label: "Config", description: "Collected config values and linked env defaults." },
  { id: "config:sources", label: "Config Sources", description: "Config keys with provider and registration origin." },
  { id: "provider:list", label: "Providers", description: "Discovered Laravel service providers and source resolution." },
  { id: "view:list", label: "Views & Components", description: "Blade views, Blade components, and Livewire components." },
];

function readUrlState() {
  const params = new URLSearchParams(window.location.search);
  return {
    project: params.get("project") ?? "",
    command: (params.get("command") as CommandId | null) ?? "route:list",
  };
}

function writeUrlState(project: string, command: CommandId, replace = false) {
  const url = new URL(window.location.href);
  if (project) {
    url.searchParams.set("project", project);
  } else {
    url.searchParams.delete("project");
  }
  url.searchParams.set("command", command);
  const method = replace ? "replaceState" : "pushState";
  window.history[method](null, "", url);
}

function formatKb(value?: number | null) {
  if (value == null || Number.isNaN(value)) return null;
  const absolute = Math.abs(value);
  if (absolute >= 1024 * 1024) return `${value < 0 ? "-" : ""}${(absolute / (1024 * 1024)).toFixed(2)} GB`;
  if (absolute >= 1024) return `${value < 0 ? "-" : ""}${(absolute / 1024).toFixed(1)} MB`;
  return `${value} KB`;
}

function emptyValue(value?: string | null) {
  return value && value.length > 0 ? value : "-";
}

function VariableBadges({ items }: { items?: Array<{ name: string; default_value?: string | null }> }) {
  if (!items || items.length === 0) {
    return <span className="text-sm text-muted-foreground">-</span>;
  }

  return (
    <div className="flex flex-wrap gap-2">
      {items.map((item, index) => (
        <Badge key={`${item.name}-${index}`} variant="secondary">
          {item.default_value == null ? item.name : `${item.name}=${item.default_value}`}
        </Badge>
      ))}
    </div>
  );
}

function SummaryChip({ label, value }: { label: string; value: string | number }) {
  return <div className="rounded-full border border-border bg-white/80 px-4 py-2 text-sm font-semibold">{label}: {value}</div>;
}

function DebugBar({ debug }: { debug?: DebugInfo }) {
  const delta =
    debug?.rss_before_kb != null && debug?.rss_after_kb != null
      ? debug.rss_after_kb - debug.rss_before_kb
      : null;

  const items = [
    debug?.parsed_file_count != null ? `Files ${debug.parsed_file_count}` : null,
    debug?.duration_ms != null ? `Time ${debug.duration_ms} ms` : null,
    debug?.rss_after_kb != null ? `RSS ${formatKb(debug.rss_after_kb)}` : null,
    delta != null ? `Delta ${delta > 0 ? "+" : ""}${formatKb(delta)}` : null,
  ].filter(Boolean) as string[];

  return (
    <div className="flex min-h-10 flex-wrap items-center justify-end gap-2">
      {items.map((item) => (
        <div
          key={item}
          className="rounded-full border border-primary/15 bg-primary/8 px-3 py-1.5 text-xs font-semibold text-primary"
        >
          {item}
        </div>
      ))}
    </div>
  );
}

function SectionTable({
  title,
  count,
  columns,
  children,
  note,
}: {
  title: string;
  count: number;
  columns: string[];
  children: React.ReactNode;
  note?: string;
}) {
  return (
    <div className="overflow-hidden rounded-[24px] border border-border bg-white/70">
      <div className="flex items-center justify-between gap-4 border-b border-border px-5 py-4">
        <h3 className="font-serif text-xl font-semibold">{title}</h3>
        <Badge variant="secondary">{count}</Badge>
      </div>
      <div className="overflow-auto">
        <table className="min-w-full border-collapse text-sm">
          <thead>
            <tr className="bg-muted/70 text-left text-xs uppercase tracking-[0.16em] text-muted-foreground">
              {columns.map((column) => (
                <th key={column} className="px-4 py-3 font-semibold">
                  {column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>{children}</tbody>
        </table>
      </div>
      {note ? <div className="px-5 py-4 text-sm text-muted-foreground">{note}</div> : null}
    </div>
  );
}

function CellStack({ top, bottom, mono = false }: { top: string; bottom?: string; mono?: boolean }) {
  return (
    <div className="space-y-1">
      <div className={cn("font-medium", mono && "font-mono text-[13px]")}>{top}</div>
      {bottom ? <div className={cn("text-xs text-muted-foreground", mono && "font-mono")}>{bottom}</div> : null}
    </div>
  );
}

function RouteTable({ payload, sourceMode }: { payload: Payload; sourceMode: boolean }) {
  const report = payload.report;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap gap-3">
        <SummaryChip label="Project" value={payload.project} />
        <SummaryChip label="Routes" value={report.route_count} />
        <SummaryChip label="Mode" value={sourceMode ? "Source Attribution" : "Route Table"} />
      </div>
      <SectionTable
        title={sourceMode ? "Registered Route Sources" : "Effective Routes"}
        count={report.route_count}
        columns={["Route", "Location", "Name", "Action", "Middleware", "Patterns", "Registered Via"]}
      >
        {report.routes.map((route: any, index: number) => {
          const middleware = route.resolved_middleware?.length ? route.resolved_middleware : route.middleware;
          const patterns = Object.entries(route.parameter_patterns || {});
          return (
            <tr key={`${route.file}-${route.line}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
              <td className="px-4 py-3">
                <CellStack top={route.methods.join("|")} bottom={route.uri} mono />
              </td>
              <td className="px-4 py-3">
                <CellStack top={`${route.file}:${route.line}:${route.column}`} mono />
              </td>
              <td className="px-4 py-3">{emptyValue(route.name)}</td>
              <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(route.action)}</td>
              <td className="px-4 py-3">
                <div className="flex flex-wrap gap-2">
                  {middleware?.length ? middleware.map((item: string) => <Badge key={item}>{item}</Badge>) : <span className="text-muted-foreground">-</span>}
                </div>
              </td>
              <td className="px-4 py-3">
                <div className="flex flex-wrap gap-2">
                  {patterns.length
                    ? patterns.map(([key, value]) => <Badge key={key}>{`${key}=${value}`}</Badge>)
                    : <span className="text-muted-foreground">-</span>}
                </div>
              </td>
              <td className="px-4 py-3">
                <CellStack
                  top={route.registration.kind}
                  bottom={`${route.registration.declared_in}:${route.registration.line}:${route.registration.column}`}
                />
                {route.registration.provider_class ? (
                  <div className="mt-1 text-xs text-muted-foreground">{route.registration.provider_class}</div>
                ) : null}
              </td>
            </tr>
          );
        })}
      </SectionTable>
    </div>
  );
}

function CompareTable({ title, rows }: { title: string; rows: any[] }) {
  return (
    <SectionTable title={title} count={rows.length} columns={["Route", "Name", "Action", "Middleware", "Source"]}>
      {rows.map((route, index) => (
        <tr key={`${route.key}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
          <td className="px-4 py-3">
            <CellStack top={route.methods.join("|")} bottom={route.uri} mono />
          </td>
          <td className="px-4 py-3">{emptyValue(route.name)}</td>
          <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(route.action)}</td>
          <td className="px-4 py-3">
            <div className="flex flex-wrap gap-2">
              {route.middleware?.length ? route.middleware.map((item: string) => <Badge key={item}>{item}</Badge>) : <span className="text-muted-foreground">-</span>}
            </div>
          </td>
          <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(route.source)}</td>
        </tr>
      ))}
    </SectionTable>
  );
}

function RouteCompare({ payload }: { payload: Payload }) {
  const comparison = payload.comparison;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap gap-3">
        <SummaryChip label="Project" value={payload.project} />
        <SummaryChip label="Runtime Routes" value={comparison.runtime_count} />
        <SummaryChip label="Analyzer Routes" value={comparison.analyzer_count} />
        <SummaryChip label="Matched" value={comparison.matched_count} />
        <SummaryChip label="Missed by Rust" value={comparison.runtime_only_count} />
        <SummaryChip label="Analyzer Only" value={comparison.analyzer_only_count} />
      </div>
      <div className="rounded-[24px] border border-border bg-white/70 p-5">
        <div className="mb-3 flex items-center justify-between gap-4">
          <h3 className="font-serif text-xl font-semibold">Comparison Notes</h3>
          <Badge
            variant={comparison.runnable ? "default" : "secondary"}
            className={comparison.runnable ? "bg-primary/12 text-primary hover:bg-primary/20" : ""}
          >
            {comparison.runnable ? "runtime available" : "runtime unavailable"}
          </Badge>
        </div>
        <p className="m-0 text-sm leading-6 text-muted-foreground">{comparison.note}</p>
        {comparison.artisan_path ? <p className="mt-3 font-mono text-xs text-muted-foreground">{comparison.artisan_path}</p> : null}
      </div>
      <CompareTable title="Runtime Only: Missing From Rust" rows={comparison.runtime_only} />
      <CompareTable title="Matched Routes" rows={comparison.matched} />
      <CompareTable title="Analyzer Only" rows={comparison.analyzer_only} />
    </div>
  );
}

function ConfigTable({ payload, sourceMode }: { payload: Payload; sourceMode: boolean }) {
  const report = payload.report;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap gap-3">
        <SummaryChip label="Project" value={payload.project} />
        <SummaryChip label="Items" value={report.item_count} />
        <SummaryChip label="Mode" value={sourceMode ? "Source Attribution" : "Config Table"} />
      </div>
      <SectionTable
        title={sourceMode ? "Config Sources" : "Effective Config"}
        count={report.item_count}
        columns={["Config Item", "Env Key", "Default", "Env Value", "Registered Via"]}
        note={!sourceMode ? "Values are shown directly in the browser; there is no terminal color legend in the web view." : undefined}
      >
        {report.items.map((item: any, index: number) => (
          <tr key={`${item.file}-${item.key}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3">
              <CellStack top={item.key} bottom={`${item.file}:${item.line}:${item.column}`} mono />
            </td>
            <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(item.env_key)}</td>
            <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(item.default_value)}</td>
            <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(item.env_value)}</td>
            <td className="px-4 py-3">
              <CellStack
                top={item.source.kind}
                bottom={`${item.source.declared_in}:${item.source.line}:${item.source.column}`}
              />
              {item.source.provider_class ? <div className="mt-1 text-xs text-muted-foreground">{item.source.provider_class}</div> : null}
            </td>
          </tr>
        ))}
      </SectionTable>
    </div>
  );
}

function ProviderTable({ payload }: { payload: Payload }) {
  const report = payload.report;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap gap-3">
        <SummaryChip label="Project" value={payload.project} />
        <SummaryChip label="Providers" value={report.provider_count} />
      </div>
      <SectionTable title="Providers" count={report.provider_count} columns={["Provider", "Registration Kind", "Package", "Source File", "Status"]}>
        {report.providers.map((provider: any, index: number) => (
          <tr key={`${provider.provider_class}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3">
              <CellStack top={provider.provider_class} bottom={`${provider.declared_in}:${provider.line}:${provider.column}`} />
            </td>
            <td className="px-4 py-3">{provider.registration_kind}</td>
            <td className="px-4 py-3">{emptyValue(provider.package_name)}</td>
            <td className="px-4 py-3 font-mono text-[13px]">{emptyValue(provider.source_file)}</td>
            <td className="px-4 py-3">
              <div className="flex flex-wrap gap-2">
                <Badge
                  variant={provider.source_available ? "default" : "secondary"}
                  className={provider.source_available ? "bg-primary/12 text-primary hover:bg-primary/20" : ""}
                >
                  {provider.status}
                </Badge>
                <Badge
                  variant={provider.source_available ? "default" : "secondary"}
                  className={provider.source_available ? "bg-primary/12 text-primary hover:bg-primary/20" : ""}
                >
                  {provider.source_available ? "source available" : "source missing"}
                </Badge>
              </div>
            </td>
          </tr>
        ))}
      </SectionTable>
    </div>
  );
}

function MiddlewareTable({ payload }: { payload: Payload }) {
  const report = payload.report;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap gap-3">
        <SummaryChip label="Project" value={payload.project} />
        <SummaryChip label="Aliases" value={report.alias_count} />
        <SummaryChip label="Groups" value={report.group_count} />
        <SummaryChip label="Patterns" value={report.pattern_count} />
      </div>
      <SectionTable title="Middleware Aliases" count={report.alias_count} columns={["Alias", "Target", "Declared In"]}>
        {report.aliases.map((alias: any, index: number) => (
          <tr key={`${alias.name}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3 font-medium">{alias.name}</td>
            <td className="px-4 py-3 font-mono text-[13px]">{alias.target}</td>
            <td className="px-4 py-3">
              <CellStack top={alias.source.provider_class} bottom={`${alias.source.declared_in}:${alias.source.line}:${alias.source.column}`} />
            </td>
          </tr>
        ))}
      </SectionTable>
      <SectionTable title="Middleware Groups" count={report.group_count} columns={["Group", "Members", "Declared In"]}>
        {report.groups.map((group: any, index: number) => (
          <tr key={`${group.name}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3 font-medium">{group.name}</td>
            <td className="px-4 py-3">
              <div className="flex flex-wrap gap-2">
                {group.members?.length ? group.members.map((member: string) => <Badge key={member}>{member}</Badge>) : <span className="text-muted-foreground">-</span>}
              </div>
            </td>
            <td className="px-4 py-3">
              <CellStack top={group.source.provider_class} bottom={`${group.source.declared_in}:${group.source.line}:${group.source.column}`} />
            </td>
          </tr>
        ))}
      </SectionTable>
      <SectionTable title="Route Patterns" count={report.pattern_count} columns={["Parameter", "Pattern", "Declared In"]}>
        {report.patterns.map((pattern: any, index: number) => (
          <tr key={`${pattern.parameter}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3 font-medium">{pattern.parameter}</td>
            <td className="px-4 py-3 font-mono text-[13px]">{pattern.pattern}</td>
            <td className="px-4 py-3">
              <CellStack top={pattern.source.provider_class} bottom={`${pattern.source.declared_in}:${pattern.source.line}:${pattern.source.column}`} />
            </td>
          </tr>
        ))}
      </SectionTable>
    </div>
  );
}

function ViewTable({ payload }: { payload: Payload }) {
  const report = payload.report;
  return (
    <div className="space-y-5">
      <div className="flex flex-wrap gap-3">
        <SummaryChip label="Project" value={payload.project} />
        <SummaryChip label="Views" value={report.view_count} />
        <SummaryChip label="Blade Components" value={report.blade_component_count} />
        <SummaryChip label="Livewire Components" value={report.livewire_component_count} />
      </div>
      <SectionTable title="Views" count={report.view_count} columns={["View", "Kind", "Blade Props", "Passed Variables", "Declared In"]}>
        {report.views.map((view: any, index: number) => (
          <tr key={`${view.name}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3">
              <CellStack top={view.name} bottom={view.file} mono />
            </td>
            <td className="px-4 py-3">{view.kind}</td>
            <td className="px-4 py-3"><VariableBadges items={view.props} /></td>
            <td className="px-4 py-3"><VariableBadges items={view.variables} /></td>
            <td className="px-4 py-3">
              <CellStack top={`${view.source.declared_in}:${view.source.line}:${view.source.column}`} mono />
              {view.source.provider_class ? <div className="mt-1 text-xs text-muted-foreground">{view.source.provider_class}</div> : null}
            </td>
          </tr>
        ))}
      </SectionTable>
      <SectionTable title="Blade Components" count={report.blade_component_count} columns={["Component", "Class", "View", "Props", "Declared In"]}>
        {report.blade_components.map((component: any, index: number) => (
          <tr key={`${component.component}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3">
              <CellStack top={component.component} bottom={component.kind} />
            </td>
            <td className="px-4 py-3">
              <CellStack top={emptyValue(component.class_name)} bottom={component.class_file ?? undefined} mono />
            </td>
            <td className="px-4 py-3">
              <CellStack top={emptyValue(component.view_name)} bottom={component.view_file ?? undefined} mono />
            </td>
            <td className="px-4 py-3"><VariableBadges items={component.props} /></td>
            <td className="px-4 py-3">
              <CellStack top={`${component.source.declared_in}:${component.source.line}:${component.source.column}`} mono />
              {component.source.provider_class ? <div className="mt-1 text-xs text-muted-foreground">{component.source.provider_class}</div> : null}
            </td>
          </tr>
        ))}
      </SectionTable>
      <SectionTable title="Livewire Components" count={report.livewire_component_count} columns={["Component", "Class", "View", "Public State", "Declared In"]}>
        {report.livewire_components.map((component: any, index: number) => (
          <tr key={`${component.component}-${index}`} className="border-b border-border/70 align-top hover:bg-primary/4">
            <td className="px-4 py-3">
              <CellStack top={component.component} bottom={component.kind} />
            </td>
            <td className="px-4 py-3">
              <CellStack top={emptyValue(component.class_name)} bottom={component.class_file ?? undefined} mono />
            </td>
            <td className="px-4 py-3">
              <CellStack top={emptyValue(component.view_name)} bottom={component.view_file ?? undefined} mono />
            </td>
            <td className="px-4 py-3"><VariableBadges items={component.state} /></td>
            <td className="px-4 py-3">
              <CellStack top={`${component.source.declared_in}:${component.source.line}:${component.source.column}`} mono />
              {component.source.provider_class ? <div className="mt-1 text-xs text-muted-foreground">{component.source.provider_class}</div> : null}
            </td>
          </tr>
        ))}
      </SectionTable>
    </div>
  );
}

function ReportView({ payload }: { payload: Payload }) {
  switch (payload.command) {
    case "route:list":
      return <RouteTable payload={payload} sourceMode={false} />;
    case "route:compare":
      return <RouteCompare payload={payload} />;
    case "route:sources":
      return <RouteTable payload={payload} sourceMode />;
    case "config:list":
      return <ConfigTable payload={payload} sourceMode={false} />;
    case "config:sources":
      return <ConfigTable payload={payload} sourceMode />;
    case "provider:list":
      return <ProviderTable payload={payload} />;
    case "middleware:list":
      return <MiddlewareTable payload={payload} />;
    case "view:list":
      return <ViewTable payload={payload} />;
    default:
      return <div className="text-sm text-muted-foreground">No renderer for {payload.command}</div>;
  }
}

export default function App() {
  const initial = useMemo(readUrlState, []);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProject, setSelectedProject] = useState(initial.project);
  const [selectedCommand, setSelectedCommand] = useState<CommandId>(initial.command);
  const [payload, setPayload] = useState<Payload | null>(null);
  const [status, setStatus] = useState("Loading");
  const [error, setError] = useState<string | null>(null);

  const commandMeta = COMMANDS.find((command) => command.id === selectedCommand) ?? COMMANDS[0];
  const selectedProjectMeta = projects.find((project) => project.id === selectedProject) ?? null;

  useEffect(() => {
    const onPopState = () => {
      const next = readUrlState();
      setSelectedProject(next.project);
      setSelectedCommand(next.command);
    };

    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);

  async function loadProjects() {
    setStatus("Loading projects");
    const response = await fetch("/api/projects");
    const data = (await response.json()) as Project[];
    setProjects(data);

    let nextProject = selectedProject;
    if (!nextProject || !data.some((project) => project.id === nextProject)) {
      nextProject = data[0]?.id ?? "";
      setSelectedProject(nextProject);
    }

    writeUrlState(nextProject, selectedCommand, true);
  }

  async function loadReport(projectId: string, commandId: CommandId) {
    if (!projectId) {
      setPayload(null);
      setError("No project selected.");
      setStatus("No project");
      return;
    }

    setStatus("Running analyzer");
    setError(null);

    const params = new URLSearchParams({ project: projectId, command: commandId });
    const response = await fetch(`/api/report?${params.toString()}`);
    const data = (await response.json()) as Payload;

    if (!response.ok) {
      setPayload(null);
      setError(data.error ?? "Unknown server error.");
      setStatus("Analyzer failed");
      return;
    }

    setPayload(data);
    setStatus("Loaded");
  }

  useEffect(() => {
    loadProjects().catch((loadError: Error) => {
      setError(loadError.message);
      setStatus("Boot failed");
    });
  }, []);

  useEffect(() => {
    if (!selectedProject) return;
    writeUrlState(selectedProject, selectedCommand, true);
    loadReport(selectedProject, selectedCommand).catch((loadError: Error) => {
      setError(loadError.message);
      setStatus("Load failed");
    });
  }, [selectedProject, selectedCommand]);

  return (
    <div className="min-h-screen p-5">
      <div className="grid min-h-[calc(100vh-2.5rem)] gap-5 lg:grid-cols-[290px_minmax(0,1fr)]">
        <Card className="flex flex-col">
          <CardHeader className="pb-5">
            <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">
              Laravel Static Debugger
            </p>
            <CardTitle className="text-[2.3rem]">rust-php</CardTitle>
            <CardDescription>
              The sidebar drives the analyzer state. Project and command selection stay in the URL, so refresh no longer resets the page.
            </CardDescription>
          </CardHeader>
          <CardContent className="flex flex-1 flex-col gap-5">
            <div className="space-y-3">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2 text-sm font-semibold text-muted-foreground">
                  <FolderTree className="h-4 w-4" />
                  Project
                </div>
                <Button size="sm" variant="ghost" onClick={() => loadProjects().catch((loadError: Error) => {
                  setError(loadError.message);
                  setStatus("Refresh failed");
                })}>
                  <RefreshCw className="h-4 w-4" />
                  Refresh
                </Button>
              </div>
              <Select value={selectedProject} onValueChange={setSelectedProject}>
                <SelectTrigger>
                  <SelectValue placeholder="Select a Laravel project" />
                </SelectTrigger>
                <SelectContent>
                  {projects.map((project) => (
                    <SelectItem key={project.id} value={project.id}>
                      {project.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Separator />
            <div className="flex min-h-0 flex-1 flex-col gap-3">
              <div className="flex items-center gap-2 text-sm font-semibold text-muted-foreground">
                <LayoutPanelLeft className="h-4 w-4" />
                Parameters
              </div>
              <ScrollArea className="min-h-0 flex-1">
                <div className="space-y-3 pr-3">
                  {COMMANDS.map((command) => (
                    <button
                      key={command.id}
                      type="button"
                      onClick={() => setSelectedCommand(command.id)}
                      className={cn(
                        "w-full rounded-[22px] border p-4 text-left transition-all",
                        selectedCommand === command.id
                          ? "border-primary bg-primary text-primary-foreground shadow-lg shadow-primary/15"
                          : "border-border bg-white/65 hover:border-primary/30 hover:bg-white"
                      )}
                    >
                      <div className="font-semibold">{command.label}</div>
                      <div className={cn("mt-2 text-sm leading-6", selectedCommand === command.id ? "text-primary-foreground/80" : "text-muted-foreground")}>
                        {command.description}
                      </div>
                    </button>
                  ))}
                </div>
              </ScrollArea>
            </div>
          </CardContent>
        </Card>

        <div className="grid min-h-0 grid-rows-[auto_minmax(0,1fr)] gap-5">
          <Card className="rounded-[999px] px-6 py-5">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div className="min-w-0">
                <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">Selected Project</p>
                <h1 className="mt-2 font-serif text-[2.25rem] font-semibold leading-none tracking-tight">
                  {selectedProjectMeta?.name ?? "None"}
                </h1>
                <p className="mt-3 break-all text-sm leading-6 text-muted-foreground">
                  {selectedProjectMeta?.root ?? ""}
                </p>
              </div>
              <div className="space-y-3 lg:text-right">
                <div className="flex items-center justify-start gap-2 lg:justify-end">
                  <Gauge className="h-4 w-4 text-muted-foreground" />
                  <span className="text-sm font-semibold text-muted-foreground">Debug Metrics</span>
                </div>
                <DebugBar debug={payload?.debug} />
                <div className="inline-flex rounded-full bg-primary/10 px-4 py-2 text-sm font-semibold text-primary">
                  {status}
                </div>
              </div>
            </div>
          </Card>

          <Card className="min-h-0">
            <CardHeader className="pb-4">
              <div className="flex flex-col gap-3 lg:flex-row lg:items-end lg:justify-between">
                <div>
                  <p className="text-xs font-semibold uppercase tracking-[0.22em] text-muted-foreground">Active Analyzer</p>
                  <CardTitle className="mt-2">{commandMeta.label}</CardTitle>
                  <CardDescription className="mt-2">{commandMeta.description}</CardDescription>
                </div>
                <Badge variant="secondary">
                  {selectedProject ? `?project=${selectedProjectMeta?.name ?? selectedProject}&command=${selectedCommand}` : selectedCommand}
                </Badge>
              </div>
            </CardHeader>
            <CardContent className="min-h-0">
              <div className="h-full rounded-[24px] border border-border bg-[linear-gradient(180deg,rgba(255,252,247,0.95),rgba(244,238,227,0.98))] p-5">
                <ScrollArea className="h-[calc(100vh-16.5rem)] pr-4">
                  {error ? (
                    <div className="grid min-h-[18rem] place-items-center text-center text-sm text-muted-foreground">
                      {error}
                    </div>
                  ) : payload ? (
                    <ReportView payload={payload} />
                  ) : (
                    <div className="grid min-h-[18rem] place-items-center text-center text-sm text-muted-foreground">
                      Loading report…
                    </div>
                  )}
                </ScrollArea>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}
