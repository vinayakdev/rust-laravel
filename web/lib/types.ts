export type Project = {
  id: string
  name: string
  root: string
}

export type DebugInfo = {
  duration_ms: number
  parsed_file_count: number
  rss_before_kb?: number | null
  rss_after_kb?: number | null
}

export type RegistrationSource = {
  kind: string
  declared_in: string
  line: number
  column: number
  provider_class?: string
}

export type RouteEntry = {
  methods: string[]
  uri: string
  name?: string
  action?: string
  file: string
  line: number
  column: number
  middleware: string[]
  resolved_middleware: string[]
  parameter_patterns: Record<string, string>
  registration: RegistrationSource
}

export type RoutesReport = {
  route_count: number
  routes: RouteEntry[]
}

export type MiddlewareAlias = {
  name: string
  target: string
  source: RegistrationSource
}

export type MiddlewareGroup = {
  name: string
  members: string[]
  source: RegistrationSource
}

export type MiddlewarePattern = {
  parameter: string
  pattern: string
  source: RegistrationSource
}

export type MiddlewareReport = {
  alias_count: number
  group_count: number
  pattern_count: number
  aliases: MiddlewareAlias[]
  groups: MiddlewareGroup[]
  patterns: MiddlewarePattern[]
}

export type ConfigItem = {
  key: string
  env_key?: string
  default_value?: string
  env_value?: string
  file: string
  line: number
  column: number
  source: RegistrationSource
}

export type ConfigReport = {
  item_count: number
  items: ConfigItem[]
}

export type ProviderEntry = {
  provider_class: string
  declared_in: string
  line: number
  column: number
  registration_kind: string
  package_name?: string
  source_file?: string
  status: string
  source_available: boolean
}

export type ProviderReport = {
  provider_count: number
  providers: ProviderEntry[]
}

export type ViewVariable = {
  name: string
  default_value?: string
}

export type ViewEntry = {
  name: string
  file: string
  kind: string
  props: ViewVariable[]
  variables: ViewVariable[]
  source: RegistrationSource
}

export type BladeComponent = {
  component: string
  kind: string
  class_name?: string
  class_file?: string
  view_name?: string
  view_file?: string
  props: ViewVariable[]
  source: RegistrationSource
}

export type LivewireComponent = {
  component: string
  kind: string
  class_name?: string
  class_file?: string
  view_name?: string
  view_file?: string
  state: ViewVariable[]
  source: RegistrationSource
}

export type ViewReport = {
  view_count: number
  blade_component_count: number
  livewire_component_count: number
  views: ViewEntry[]
  blade_components: BladeComponent[]
  livewire_components: LivewireComponent[]
}

export type ComparedRoute = {
  key: string
  methods: string[]
  uri: string
  name?: string
  action?: string
  source?: string
  middleware: string[]
}

export type RouteComparison = {
  runtime_count: number
  analyzer_count: number
  matched_count: number
  runtime_only_count: number
  analyzer_only_count: number
  runnable: boolean
  artisan_path?: string
  note: string
  matched: ComparedRoute[]
  runtime_only: ComparedRoute[]
  analyzer_only: ComparedRoute[]
}

export type ModelEntry = {
  file: string
  line: number
  class_name: string
  namespace: string
  table: string
  table_inferred: boolean
  primary_key: string
  key_type: string
  incrementing: boolean
  timestamps: boolean
  soft_deletes: boolean
  connection?: string
  fillable: string[]
  guarded: string[]
  hidden: string[]
  casts: Record<string, string>
  appends: string[]
  with: string[]
  traits: string[]
  relations: RelationEntry[]
  scopes: string[]
  accessors: string[]
  mutators: string[]
  columns: ColumnEntry[]
}

export type ModelReport = {
  model_count: number
  models: ModelEntry[]
}

export type RelationEntry = {
  method: string
  relation_type: string
  related_model: string
  related_model_file?: string
  foreign_key?: string
  local_key?: string
  pivot_table?: string
  line: number
}

export type ColumnEntry = {
  name: string
  column_type: string
  nullable: boolean
  default?: string
  unique: boolean
  unsigned: boolean
  primary: boolean
  enum_values: string[]
  comment?: string
  references?: string
  on_table?: string
}

export type IndexEntry = {
  columns: string[]
  index_type: string
}

export type MigrationEntry = {
  file: string
  timestamp: string
  class_name: string
  table: string
  operation: string
  columns: ColumnEntry[]
  indexes: IndexEntry[]
  dropped_columns: string[]
}

export type MigrationReport = {
  migration_count: number
  migrations: MigrationEntry[]
}

export type CommandId =
  | "route:list"
  | "route:compare"
  | "route:sources"
  | "middleware:list"
  | "config:list"
  | "config:sources"
  | "provider:list"
  | "view:list"
  | "model:list"
  | "migration:list"

export type Payload = {
  project: string
  root: string
  command: CommandId
  debug?: DebugInfo
  report?: RoutesReport | MiddlewareReport | ConfigReport | ProviderReport | ViewReport | ModelReport | MigrationReport
  comparison?: RouteComparison
  error?: string
}
