use bumpalo::Bump;
use php_parser::ast::{ClassMember, Expr, ExprId, Stmt, UseKind};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use php_parser::span::LineInfo;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::analyzers::providers;
use crate::analyzers::routes;
use crate::php::ast::{
    byte_offset_to_line_col, expr_name, expr_to_path, expr_to_string, expr_to_string_list,
    span_text, strip_root,
};
use crate::php::psr4::{collect_psr4_mappings, resolve_class_file, resolve_namespace_dir};
use crate::php::walk::walk_stmts;
use crate::project::LaravelProject;
use crate::types::{
    BladeComponentEntry, LivewireComponentEntry, MissingViewEntry, ProviderEntry, ViewEntry,
    ViewReport, ViewSource, ViewUsage, ViewVariable,
};

pub fn analyze(project: &LaravelProject) -> Result<ViewReport, String> {
    let mappings = collect_psr4_mappings(&project.root)?;
    let provider_report = providers::analyze(project)?;
    let mut view_namespaces = default_view_namespaces(project);

    let mut views = collect_app_views(project)?;
    let mut blade_components =
        collect_default_blade_components(project, &mappings, &view_namespaces)?;
    let mut livewire_components =
        collect_conventional_livewire_components(project, &mappings, &view_namespaces)?;

    let registrations =
        collect_provider_registrations(project, &provider_report.providers, &mappings)?;
    view_namespaces.extend(registrations.view_namespaces);

    views.extend(registrations.views);
    blade_components.extend(registrations.blade_components);
    livewire_components.extend(registrations.livewire_components);

    let usage_map = collect_view_usages(project)?;
    let missing_views = apply_view_usages(&mut views, usage_map, project, &view_namespaces);

    dedup_views(&mut views);
    dedup_blade_components(&mut blade_components);
    dedup_livewire_components(&mut livewire_components);

    views.sort_by(|l, r| l.name.cmp(&r.name).then(l.file.cmp(&r.file)));
    blade_components.sort_by(|l, r| l.component.cmp(&r.component).then(l.kind.cmp(&r.kind)));
    livewire_components.sort_by(|l, r| l.component.cmp(&r.component).then(l.kind.cmp(&r.kind)));

    Ok(ViewReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        view_count: views.len(),
        blade_component_count: blade_components.len(),
        livewire_component_count: livewire_components.len(),
        missing_view_count: missing_views.len(),
        views,
        blade_components,
        livewire_components,
        missing_views,
    })
}

struct RegistrationSets {
    views: Vec<ViewEntry>,
    blade_components: Vec<BladeComponentEntry>,
    livewire_components: Vec<LivewireComponentEntry>,
    view_namespaces: HashMap<String, PathBuf>,
}

fn collect_app_views(project: &LaravelProject) -> Result<Vec<ViewEntry>, String> {
    let views_root = project.root.join("resources/views");
    if !views_root.is_dir() {
        return Ok(Vec::new());
    }

    let files = collect_blade_files(&views_root);
    Ok(files
        .into_iter()
        .map(|file| ViewEntry {
            name: blade_view_name(&views_root, &file, None),
            file: strip_root(&project.root, &file),
            kind: if file.starts_with(views_root.join("components")) {
                "anonymous-component-view".to_string()
            } else {
                "app-view".to_string()
            },
            props: parse_blade_props(&file),
            variables: Vec::new(),
            usages: Vec::new(),
            source: ViewSource {
                declared_in: strip_root(&project.root, &file),
                line: 1,
                column: 1,
                provider_class: None,
            },
        })
        .collect())
}

fn collect_default_blade_components(
    project: &LaravelProject,
    mappings: &[crate::php::psr4::Psr4Mapping],
    view_namespaces: &HashMap<String, PathBuf>,
) -> Result<Vec<BladeComponentEntry>, String> {
    let mut entries = Vec::new();
    let class_root = project.root.join("app/View/Components");
    if class_root.is_dir() {
        for file in collect_php_files(&class_root) {
            let class_name = class_name_for_path(&project.root, &file, mappings);
            let view_name = derive_class_component_view_name(&class_root, &file);
            entries.push(BladeComponentEntry {
                component: derive_component_alias(&class_root, &file, None),
                kind: "blade-class-auto".to_string(),
                class_name,
                class_file: Some(strip_root(&project.root, &file)),
                view_name: Some(view_name.clone()),
                view_file: resolve_view_file(project, view_namespaces, &view_name),
                props: parse_class_component_props(&file),
                source: ViewSource {
                    declared_in: strip_root(&project.root, &file),
                    line: 1,
                    column: 1,
                    provider_class: None,
                },
            });
        }
    }

    let anon_root = project.root.join("resources/views/components");
    if anon_root.is_dir() {
        for file in collect_blade_files(&anon_root) {
            let name = derive_component_alias(&anon_root, &file, None);
            entries.push(BladeComponentEntry {
                component: name,
                kind: "blade-anonymous-auto".to_string(),
                class_name: None,
                class_file: None,
                view_name: Some(blade_view_name(
                    &project.root.join("resources/views"),
                    &file,
                    None,
                )),
                view_file: Some(strip_root(&project.root, &file)),
                props: parse_blade_props(&file),
                source: ViewSource {
                    declared_in: strip_root(&project.root, &file),
                    line: 1,
                    column: 1,
                    provider_class: None,
                },
            });
        }
    }

    Ok(entries)
}

fn collect_conventional_livewire_components(
    project: &LaravelProject,
    mappings: &[crate::php::psr4::Psr4Mapping],
    view_namespaces: &HashMap<String, PathBuf>,
) -> Result<Vec<LivewireComponentEntry>, String> {
    let mut entries = Vec::new();

    for root in [
        project.root.join("app/Http/Livewire"),
        project.root.join("app/Livewire"),
    ] {
        if !root.is_dir() {
            continue;
        }

        for file in collect_php_files(&root) {
            let component = derive_component_alias(&root, &file, None);
            let class_name = class_name_for_path(&project.root, &file, mappings);
            let view_name = Some(format!("livewire.{component}"));
            let view_file = resolve_view_file(
                project,
                view_namespaces,
                view_name.as_deref().unwrap_or_default(),
            );
            entries.push(LivewireComponentEntry {
                component,
                kind: "livewire-class-auto".to_string(),
                class_name,
                class_file: Some(strip_root(&project.root, &file)),
                view_name,
                view_file,
                state: parse_livewire_state(&file),
                source: ViewSource {
                    declared_in: strip_root(&project.root, &file),
                    line: 1,
                    column: 1,
                    provider_class: None,
                },
            });
        }
    }

    Ok(entries)
}

fn collect_provider_registrations(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    mappings: &[crate::php::psr4::Psr4Mapping],
) -> Result<RegistrationSets, String> {
    let mut views = Vec::new();
    let mut blade_components = Vec::new();
    let mut livewire_components = Vec::new();
    let mut view_namespaces = HashMap::new();
    let mut seen_sources = BTreeSet::new();

    for provider in providers {
        let Some(relative_source_file) = provider.source_file.as_ref() else {
            continue;
        };
        if !provider.source_available {
            continue;
        }
        if !seen_sources.insert((
            provider.provider_class.clone(),
            relative_source_file.clone(),
        )) {
            continue;
        }

        let provider_file = project.root.join(relative_source_file);
        let source = fs::read(&provider_file)
            .map_err(|e| format!("failed to read {}: {e}", provider_file.display()))?;
        let extracted = extract_provider_view_data(
            project,
            provider,
            relative_source_file,
            &provider_file,
            &source,
            mappings,
        );
        views.extend(extracted.views);
        blade_components.extend(extracted.blade_components);
        livewire_components.extend(extracted.livewire_components);
        view_namespaces.extend(extracted.view_namespaces);
    }

    Ok(RegistrationSets {
        views,
        blade_components,
        livewire_components,
        view_namespaces,
    })
}

fn extract_provider_view_data(
    project: &LaravelProject,
    provider: &ProviderEntry,
    declared_in: &Path,
    provider_file: &Path,
    source: &[u8],
    mappings: &[crate::php::psr4::Psr4Mapping],
) -> RegistrationSets {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return RegistrationSets {
            views: Vec::new(),
            blade_components: Vec::new(),
            livewire_components: Vec::new(),
            view_namespaces: HashMap::new(),
        };
    }
    let imports = build_import_map(program.statements, source);

    let mut views = Vec::new();
    let mut blade_components = Vec::new();
    let mut livewire_components = Vec::new();
    let mut view_namespaces = HashMap::new();

    walk_stmts(program.statements, true, &mut |expr| {
        visit_provider_expr(
            expr,
            source,
            project,
            provider,
            declared_in,
            provider_file,
            mappings,
            &imports,
            &mut views,
            &mut blade_components,
            &mut livewire_components,
            &mut view_namespaces,
        );
    });

    RegistrationSets {
        views,
        blade_components,
        livewire_components,
        view_namespaces,
    }
}

#[allow(clippy::too_many_arguments)]
fn visit_provider_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    provider: &ProviderEntry,
    declared_in: &Path,
    provider_file: &Path,
    mappings: &[crate::php::psr4::Psr4Mapping],
    imports: &HashMap<String, String>,
    views: &mut Vec<ViewEntry>,
    blade_components: &mut Vec<BladeComponentEntry>,
    livewire_components: &mut Vec<LivewireComponentEntry>,
    view_namespaces: &mut HashMap<String, PathBuf>,
) {
    match expr {
        Expr::MethodCall { method, args, .. } => {
            let method_name = expr_name(method, source).unwrap_or_default();
            let source_ref = source_ref(expr, source, provider, declared_in);

            match method_name.as_str() {
                "loadViewsFrom" if args.len() >= 2 => {
                    if let (Some(path), Some(namespace)) = (
                        args.first().and_then(|a| {
                            expr_to_path(a.value, source, &project.root, provider_file)
                        }),
                        args.get(1).and_then(|a| expr_to_string(a.value, source)),
                    ) {
                        view_namespaces
                            .entry(namespace.clone())
                            .or_insert_with(|| path.clone());
                        for file in collect_blade_files(&path) {
                            views.push(ViewEntry {
                                name: blade_view_name(&path, &file, Some(&namespace)),
                                file: strip_root(&project.root, &file),
                                kind: "provider-view-namespace".to_string(),
                                props: parse_blade_props(&file),
                                variables: Vec::new(),
                                usages: Vec::new(),
                                source: source_ref.clone(),
                            });
                        }

                        let components_dir = path.join("components");
                        if components_dir.is_dir() {
                            for file in collect_blade_files(&components_dir) {
                                blade_components.push(BladeComponentEntry {
                                    component: derive_component_alias(
                                        &components_dir,
                                        &file,
                                        Some(&namespace),
                                    ),
                                    kind: "blade-anonymous-package".to_string(),
                                    class_name: None,
                                    class_file: None,
                                    view_name: Some(blade_view_name(
                                        &path,
                                        &file,
                                        Some(&namespace),
                                    )),
                                    view_file: Some(strip_root(&project.root, &file)),
                                    props: parse_blade_props(&file),
                                    source: source_ref.clone(),
                                });
                            }
                        }
                    }
                }
                "loadViewComponentsAs" if args.len() >= 2 => {
                    if let Some(prefix) = args.first().and_then(|a| expr_to_string(a.value, source))
                    {
                        for class_name in args
                            .get(1)
                            .map(|a| expr_to_string_list(a.value, source))
                            .unwrap_or_default()
                        {
                            blade_components.push(build_blade_component_from_class(
                                project,
                                &format!(
                                    "{}-{}",
                                    prefix,
                                    kebab_case(last_class_segment(&class_name))
                                ),
                                "blade-class-prefix",
                                Some(class_name),
                                &source_ref,
                                mappings,
                                view_namespaces,
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        Expr::StaticCall {
            class,
            method,
            args,
            ..
        } => {
            let class_name = expr_name(class, source).unwrap_or_default();
            let method_name = expr_name(method, source).unwrap_or_default();
            let source_ref = source_ref(expr, source, provider, declared_in);

            match (class_name.as_str(), method_name.as_str()) {
                ("Blade", "component") if args.len() >= 2 => {
                    if let (Some(alias), Some(class_name)) = (
                        args.first().and_then(|a| expr_to_string(a.value, source)),
                        args.get(1)
                            .and_then(|a| expr_to_class_name(a.value, source, imports)),
                    ) {
                        blade_components.push(build_blade_component_from_class(
                            project,
                            &alias,
                            "blade-class-manual",
                            Some(class_name),
                            &source_ref,
                            mappings,
                            view_namespaces,
                        ));
                    }
                }
                ("Blade", "componentNamespace") if args.len() >= 2 => {
                    if let (Some(class_namespace), Some(prefix)) = (
                        args.first().and_then(|a| expr_to_string(a.value, source)),
                        args.get(1).and_then(|a| expr_to_string(a.value, source)),
                    ) {
                        if let Some(dir) = resolve_namespace_dir(&class_namespace, mappings) {
                            for file in collect_php_files(&dir) {
                                let class_name =
                                    class_name_for_path(&project.root, &file, mappings);
                                blade_components.push(build_blade_component_from_class(
                                    project,
                                    &derive_component_alias(&dir, &file, Some(&prefix)),
                                    "blade-class-namespace",
                                    class_name,
                                    &source_ref,
                                    mappings,
                                    view_namespaces,
                                ));
                            }
                        }
                    }
                }
                ("Blade", "anonymousComponentPath") if !args.is_empty() => {
                    if let Some(path) = args
                        .first()
                        .and_then(|a| expr_to_path(a.value, source, &project.root, provider_file))
                    {
                        let prefix = args.get(1).and_then(|a| expr_to_string(a.value, source));
                        for file in collect_blade_files(&path) {
                            blade_components.push(BladeComponentEntry {
                                component: derive_component_alias(&path, &file, prefix.as_deref()),
                                kind: "blade-anonymous-registered".to_string(),
                                class_name: None,
                                class_file: None,
                                view_name: None,
                                view_file: Some(strip_root(&project.root, &file)),
                                props: parse_blade_props(&file),
                                source: source_ref.clone(),
                            });
                        }
                    }
                }
                ("Livewire", "component") if args.len() >= 2 => {
                    if let (Some(alias), Some(class_name)) = (
                        args.first().and_then(|a| expr_to_string(a.value, source)),
                        args.get(1)
                            .and_then(|a| expr_to_class_name(a.value, source, imports)),
                    ) {
                        livewire_components.push(build_livewire_component_from_class(
                            project,
                            &alias,
                            "livewire-class-manual",
                            Some(class_name),
                            &source_ref,
                            mappings,
                            view_namespaces,
                        ));
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn build_import_map(
    stmts: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
) -> HashMap<String, String> {
    let mut imports = HashMap::new();
    for stmt in stmts {
        let Stmt::Use { uses, kind, .. } = stmt else {
            continue;
        };
        if *kind != UseKind::Normal {
            continue;
        }
        for item in *uses {
            let fqn = item
                .name
                .parts
                .iter()
                .map(|part| crate::php::ast::span_text(part.span, source))
                .collect::<String>()
                .trim_start_matches('\\')
                .to_string();
            let key = if let Some(alias_token) = item.alias {
                crate::php::ast::span_text(alias_token.span, source)
            } else {
                last_class_segment(&fqn).to_string()
            };
            imports.insert(key, fqn);
        }
    }
    imports
}

fn expr_to_class_name(
    expr: ExprId<'_>,
    source: &[u8],
    imports: &HashMap<String, String>,
) -> Option<String> {
    let Expr::ClassConstFetch {
        class, constant, ..
    } = expr
    else {
        return expr_to_string(expr, source);
    };
    let constant_name = expr_name(constant, source)?;
    if constant_name != "class" {
        return None;
    }
    let raw = expr_name(class, source)?
        .trim_start_matches('\\')
        .to_string();
    Some(if raw.contains('\\') {
        raw
    } else {
        imports.get(&raw).cloned().unwrap_or(raw)
    })
}

fn source_ref(
    expr: ExprId<'_>,
    source: &[u8],
    provider: &ProviderEntry,
    declared_in: &Path,
) -> ViewSource {
    ViewSource {
        declared_in: declared_in.to_path_buf(),
        line: expr
            .span()
            .line_info(source)
            .map_or(provider.line, |i| i.line),
        column: expr
            .span()
            .line_info(source)
            .map_or(provider.column, |i| i.column),
        provider_class: Some(provider.provider_class.clone()),
    }
}

fn build_blade_component_from_class(
    project: &LaravelProject,
    component: &str,
    kind: &str,
    class_name: Option<String>,
    source: &ViewSource,
    mappings: &[crate::php::psr4::Psr4Mapping],
    view_namespaces: &HashMap<String, PathBuf>,
) -> BladeComponentEntry {
    let class_file = class_name
        .as_ref()
        .and_then(|name| resolve_class_file(name, mappings))
        .map(|file| strip_root(&project.root, &file));
    let props = class_file
        .as_ref()
        .map(|relative| parse_class_component_props(&project.root.join(relative)))
        .unwrap_or_default();
    let view_name = class_name
        .as_ref()
        .and_then(|name| resolve_render_view_name(project, name, mappings));
    let view_file = view_name
        .as_ref()
        .and_then(|name| resolve_view_file(project, view_namespaces, name));

    BladeComponentEntry {
        component: component.to_string(),
        kind: kind.to_string(),
        class_name,
        class_file,
        view_name,
        view_file,
        source: source.clone(),
        props,
    }
}

fn build_livewire_component_from_class(
    project: &LaravelProject,
    component: &str,
    kind: &str,
    class_name: Option<String>,
    source: &ViewSource,
    mappings: &[crate::php::psr4::Psr4Mapping],
    view_namespaces: &HashMap<String, PathBuf>,
) -> LivewireComponentEntry {
    let class_file = class_name
        .as_ref()
        .and_then(|name| resolve_class_file(name, mappings))
        .map(|file| strip_root(&project.root, &file));
    let state = class_file
        .as_ref()
        .map(|relative| parse_livewire_state(&project.root.join(relative)))
        .unwrap_or_default();
    let view_name = class_name
        .as_ref()
        .and_then(|name| resolve_render_view_name(project, name, mappings))
        .or_else(|| Some(format!("livewire.{component}")));
    let view_file = view_name
        .as_ref()
        .and_then(|name| resolve_view_file(project, view_namespaces, name));

    LivewireComponentEntry {
        component: component.to_string(),
        kind: kind.to_string(),
        class_name,
        class_file,
        view_name,
        view_file,
        source: source.clone(),
        state,
    }
}

fn collect_view_usages(
    project: &LaravelProject,
) -> Result<HashMap<String, Vec<ViewUsage>>, String> {
    let mut usage_map: HashMap<String, Vec<ViewUsage>> = HashMap::new();

    for root in [project.root.join("app"), project.root.join("packages")] {
        collect_view_usages_from_root(project, &root, &mut usage_map)?;
    }

    for file in routes::collect_registered_route_paths(project)? {
        for (view_name, usage) in extract_view_usages_from_php(project, &file)? {
            usage_map.entry(view_name).or_default().push(usage);
        }
    }

    Ok(usage_map)
}

fn collect_view_usages_from_root(
    project: &LaravelProject,
    root: &Path,
    usage_map: &mut HashMap<String, Vec<ViewUsage>>,
) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }

    for file in collect_php_files(root) {
        for (view_name, usage) in extract_view_usages_from_php(project, &file)? {
            usage_map.entry(view_name).or_default().push(usage);
        }
    }

    Ok(())
}

fn extract_view_usages_from_php(
    project: &LaravelProject,
    file: &Path,
) -> Result<Vec<(String, ViewUsage)>, String> {
    let source = fs::read(file).map_err(|e| format!("failed to read {}: {e}", file.display()))?;
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return Ok(extract_view_usages_from_php_fallback(
            project, file, &source,
        ));
    }

    let imports = build_import_map(program.statements, &source);
    let mut usages = Vec::new();
    collect_view_usages_from_stmts(
        program.statements,
        &source,
        project,
        file,
        &imports,
        &mut usages,
    );
    Ok(usages)
}

fn extract_view_usages_from_php_fallback(
    project: &LaravelProject,
    file: &Path,
    source: &[u8],
) -> Vec<(String, ViewUsage)> {
    const PATTERNS: [(&str, &str); 5] = [
        ("response()->view(", "response-view"),
        ("View::make(", "view-facade-make"),
        ("View::first(", "view-facade-first"),
        ("Route::view(", "route-view"),
        ("view(", "view-call"),
    ];

    let text = String::from_utf8_lossy(source);
    let mut usages = Vec::new();
    let mut offset = 0usize;

    while offset < text.len() {
        let mut matched = None;
        for (prefix, kind) in PATTERNS {
            if text[offset..].starts_with(prefix) {
                matched = Some((prefix, kind));
                break;
            }
        }

        let Some((prefix, kind)) = matched else {
            offset += 1;
            continue;
        };

        let open_index = offset + prefix.len() - 1;
        let Some(close_index) = find_matching_delimiter(&text, open_index, '(', ')') else {
            offset += prefix.len();
            continue;
        };

        let args_body = &text[open_index + 1..close_index];
        let args = split_top_level(args_body, ',');
        let view_names = match kind {
            "route-view" => args
                .get(1)
                .map(|arg| extract_string_list_from_text(arg))
                .unwrap_or_default(),
            "view-facade-first" => args
                .first()
                .map(|arg| extract_string_list_from_text(arg))
                .unwrap_or_default(),
            _ => args
                .first()
                .and_then(|arg| extract_string_literal(arg))
                .into_iter()
                .collect(),
        };

        if view_names.is_empty() {
            offset = close_index + 1;
            continue;
        }

        let start_index = if kind == "route-view" { 2 } else { 1 };
        let mut variables = args
            .iter()
            .skip(start_index)
            .flat_map(|arg| extract_passed_variables_from_text(arg))
            .collect::<Vec<_>>();

        let mut chain_index = skip_ascii_whitespace(&text, close_index + 1);
        while text[chain_index..].starts_with("->with(") {
            let with_open = chain_index + "->with".len();
            let Some(with_close) = find_matching_delimiter(&text, with_open, '(', ')') else {
                break;
            };
            let with_body = &text[with_open + 1..with_close];
            variables.extend(extract_with_variables_from_text(&split_top_level(
                with_body, ',',
            )));
            chain_index = skip_ascii_whitespace(&text, with_close + 1);
        }

        dedup_variables(&mut variables);
        let (line, column) = byte_offset_to_line_col(source, offset);
        let usage = ViewUsage {
            kind: kind.to_string(),
            source: ViewSource {
                declared_in: strip_root(&project.root, file),
                line,
                column,
                provider_class: None,
            },
            variables,
        };

        for view_name in view_names {
            usages.push((view_name, usage.clone()));
        }

        offset = chain_index.max(close_index + 1);
    }

    usages
}

fn collect_view_usages_from_stmts(
    stmts: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
    project: &LaravelProject,
    file: &Path,
    imports: &HashMap<String, String>,
    out: &mut Vec<(String, ViewUsage)>,
) {
    for stmt in stmts {
        match stmt {
            Stmt::Return {
                expr: Some(expr), ..
            }
            | Stmt::Expression { expr, .. } => collect_view_usages_from_expr(
                *expr,
                source,
                project,
                file,
                stmt.span().line_info(source),
                imports,
                out,
            ),
            Stmt::Block { statements, .. }
            | Stmt::Declare {
                body: statements, ..
            } => {
                collect_view_usages_from_stmts(statements, source, project, file, imports, out);
            }
            Stmt::Namespace {
                body: Some(body), ..
            } => {
                collect_view_usages_from_stmts(body, source, project, file, imports, out);
            }
            Stmt::Class { members, .. }
            | Stmt::Interface { members, .. }
            | Stmt::Trait { members, .. }
            | Stmt::Enum { members, .. } => {
                for member in members.iter().copied() {
                    if let ClassMember::Method { body, .. } = member {
                        collect_view_usages_from_stmts(body, source, project, file, imports, out);
                    }
                }
            }
            Stmt::If {
                then_block,
                else_block,
                ..
            } => {
                collect_view_usages_from_stmts(then_block, source, project, file, imports, out);
                if let Some(else_block) = else_block {
                    collect_view_usages_from_stmts(else_block, source, project, file, imports, out);
                }
            }
            Stmt::While { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::For { body, .. }
            | Stmt::Foreach { body, .. }
            | Stmt::Try { body, .. } => {
                collect_view_usages_from_stmts(body, source, project, file, imports, out);
            }
            _ => {}
        }
    }
}

fn usage_source(project: &LaravelProject, file: &Path, line_info: Option<LineInfo>) -> ViewSource {
    ViewSource {
        declared_in: strip_root(&project.root, file),
        line: line_info.map_or(1, |info| info.line),
        column: line_info.map_or(1, |info| info.column),
        provider_class: None,
    }
}

fn collect_view_usages_from_expr(
    expr: ExprId<'_>,
    source: &[u8],
    project: &LaravelProject,
    file: &Path,
    line_info: Option<LineInfo>,
    imports: &HashMap<String, String>,
    out: &mut Vec<(String, ViewUsage)>,
) {
    let Some(invocation) = extract_view_invocation(expr, source, imports) else {
        return;
    };

    for name in invocation.view_names {
        out.push((
            name,
            ViewUsage {
                kind: invocation.kind.clone(),
                source: usage_source(project, file, line_info),
                variables: invocation.variables.clone(),
            },
        ));
    }
}

struct ViewInvocation {
    kind: String,
    view_names: Vec<String>,
    variables: Vec<ViewVariable>,
}

fn extract_view_invocation(
    expr: ExprId<'_>,
    source: &[u8],
    imports: &HashMap<String, String>,
) -> Option<ViewInvocation> {
    match expr {
        Expr::Call { func, args, .. } => {
            let func_name = expr_name(func, source)?;
            if matches!(func_name.as_str(), "view" | "\\view") {
                return Some(ViewInvocation {
                    kind: "view-call".to_string(),
                    view_names: args
                        .first()
                        .and_then(|arg| expr_to_string(arg.value, source))
                        .into_iter()
                        .collect(),
                    variables: extract_passed_variables_from_args(&args, 1, source),
                });
            }
            None
        }
        Expr::MethodCall {
            target,
            method,
            args,
            ..
        } => {
            let method_name = expr_name(method, source)?;
            match method_name.as_str() {
                "with" => {
                    let mut invocation = extract_view_invocation(target, source, imports)?;
                    invocation
                        .variables
                        .extend(extract_with_variables(&args, source));
                    dedup_variables(&mut invocation.variables);
                    Some(invocation)
                }
                "view" if is_response_helper_call(target, source) => Some(ViewInvocation {
                    kind: "response-view".to_string(),
                    view_names: args
                        .first()
                        .and_then(|arg| expr_to_string(arg.value, source))
                        .into_iter()
                        .collect(),
                    variables: extract_passed_variables_from_args(&args, 1, source),
                }),
                "make" | "first" if is_view_factory_target(target, source) => {
                    extract_view_factory_invocation(&method_name, args, source, "view-factory")
                }
                _ => extract_view_invocation(target, source, imports),
            }
        }
        Expr::StaticCall {
            class,
            method,
            args,
            ..
        } => {
            let class_name = resolve_expr_class_name(class, source, imports);
            let method_name = expr_name(method, source)?;

            if is_route_class(class_name.as_deref()) && method_name == "view" {
                return Some(ViewInvocation {
                    kind: "route-view".to_string(),
                    view_names: args
                        .get(1)
                        .and_then(|arg| expr_to_string(arg.value, source))
                        .into_iter()
                        .collect(),
                    variables: extract_passed_variables_from_args(&args, 2, source),
                });
            }

            if is_view_factory_class(class_name.as_deref()) {
                return extract_view_factory_invocation(&method_name, args, source, "view-facade");
            }

            None
        }
        _ => None,
    }
}

fn extract_view_factory_invocation(
    method_name: &str,
    args: &[php_parser::ast::Arg<'_>],
    source: &[u8],
    kind_prefix: &str,
) -> Option<ViewInvocation> {
    match method_name {
        "make" => Some(ViewInvocation {
            kind: format!("{kind_prefix}-make"),
            view_names: args
                .first()
                .and_then(|arg| expr_to_string(arg.value, source))
                .into_iter()
                .collect(),
            variables: extract_passed_variables_from_args(args, 1, source),
        }),
        "first" => {
            let view_names = args
                .first()
                .map(|arg| expr_to_string_list(arg.value, source))
                .unwrap_or_default();
            if view_names.is_empty() {
                return None;
            }
            Some(ViewInvocation {
                kind: format!("{kind_prefix}-first"),
                view_names,
                variables: extract_passed_variables_from_args(args, 1, source),
            })
        }
        _ => None,
    }
}

fn extract_passed_variables_from_args(
    args: &[php_parser::ast::Arg<'_>],
    start_index: usize,
    source: &[u8],
) -> Vec<ViewVariable> {
    let mut variables = Vec::new();
    for arg in args.iter().skip(start_index) {
        variables.extend(extract_passed_variables(arg.value, source));
    }
    dedup_variables(&mut variables);
    variables
}

fn is_response_helper_call(expr: ExprId<'_>, source: &[u8]) -> bool {
    matches!(expr, Expr::Call { func, .. } if matches!(expr_name(func, source).as_deref(), Some("response" | "\\response")))
}

fn is_view_factory_target(expr: ExprId<'_>, source: &[u8]) -> bool {
    matches!(expr, Expr::Call { func, args, .. } if matches!(expr_name(func, source).as_deref(), Some("view" | "\\view")) && args.is_empty())
}

fn is_view_factory_class(class_name: Option<&str>) -> bool {
    matches!(
        class_name,
        Some(
            "View"
                | "\\View"
                | "Illuminate\\Support\\Facades\\View"
                | "\\Illuminate\\Support\\Facades\\View"
                | "Illuminate\\Contracts\\View\\Factory"
                | "\\Illuminate\\Contracts\\View\\Factory"
        )
    )
}

fn is_route_class(class_name: Option<&str>) -> bool {
    matches!(
        class_name,
        Some(
            "Route"
                | "\\Route"
                | "Illuminate\\Support\\Facades\\Route"
                | "\\Illuminate\\Support\\Facades\\Route"
        )
    )
}

fn resolve_expr_class_name(
    expr: ExprId<'_>,
    source: &[u8],
    imports: &HashMap<String, String>,
) -> Option<String> {
    let raw = expr_name(expr, source)?;
    if raw.contains('\\') || raw.starts_with('\\') {
        return Some(raw);
    }
    Some(imports.get(&raw).cloned().unwrap_or(raw))
}

fn extract_passed_variables(expr: ExprId<'_>, source: &[u8]) -> Vec<ViewVariable> {
    match expr {
        Expr::Array { items, .. } => items
            .iter()
            .filter_map(|item| {
                let key = item
                    .key
                    .and_then(|key| expr_to_string(key, source))
                    .or_else(|| {
                        item.key
                            .map(|key| trim_quotes(&span_text(key.span(), source)).to_string())
                    })?;
                Some(ViewVariable {
                    name: key,
                    default_value: expr_literal_default(item.value, source),
                })
            })
            .collect(),
        Expr::Call { func, args, .. } => {
            let func_name = expr_name(func, source).unwrap_or_default();
            if func_name == "compact" || func_name == "\\compact" {
                return args
                    .iter()
                    .filter_map(|arg| expr_to_string(arg.value, source))
                    .map(|name| ViewVariable {
                        name,
                        default_value: None,
                    })
                    .collect();
            }
            if func_name == "array_merge" || func_name == "\\array_merge" {
                let mut merged = Vec::new();
                for arg in *args {
                    merged.extend(extract_passed_variables(arg.value, source));
                }
                dedup_variables(&mut merged);
                return merged;
            }
            extract_passed_variables_from_text(&span_text(expr.span(), source))
        }
        _ => extract_passed_variables_from_text(&span_text(expr.span(), source)),
    }
}

fn extract_passed_variables_from_text(expr: &str) -> Vec<ViewVariable> {
    let trimmed = strip_wrapping_parens(expr.trim());
    if trimmed.is_empty() {
        return Vec::new();
    }

    let plus_parts = split_top_level(trimmed, '+');
    if plus_parts.len() > 1 {
        let mut merged = plus_parts
            .into_iter()
            .flat_map(|part| extract_passed_variables_from_text(&part))
            .collect::<Vec<_>>();
        dedup_variables(&mut merged);
        return merged;
    }

    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return parse_array_like_variables(&trimmed[1..trimmed.len() - 1]);
    }

    if let Some(body) = extract_function_call_body(trimmed, &["compact", "\\compact"]) {
        return split_top_level(body, ',')
            .into_iter()
            .filter_map(|part| extract_string_literal(&part))
            .map(|name| ViewVariable {
                name,
                default_value: None,
            })
            .collect();
    }

    if let Some(body) = extract_function_call_body(trimmed, &["array_merge", "\\array_merge"]) {
        let mut merged = split_top_level(body, ',')
            .into_iter()
            .flat_map(|part| extract_passed_variables_from_text(&part))
            .collect::<Vec<_>>();
        dedup_variables(&mut merged);
        return merged;
    }

    Vec::new()
}

fn extract_with_variables(args: &[php_parser::ast::Arg<'_>], source: &[u8]) -> Vec<ViewVariable> {
    if args.len() >= 2 {
        if let Some(name) = expr_to_string(args[0].value, source) {
            return vec![ViewVariable {
                name,
                default_value: expr_literal_default(args[1].value, source),
            }];
        }
    }

    let mut variables = extract_passed_variables_from_args(args, 0, source);
    dedup_variables(&mut variables);
    variables
}

fn extract_with_variables_from_text(args: &[String]) -> Vec<ViewVariable> {
    if args.len() >= 2 {
        if let Some(name) = extract_string_literal(&args[0]) {
            return vec![ViewVariable {
                name,
                default_value: literal_default_from_text(&args[1]),
            }];
        }
    }

    let mut variables = args
        .iter()
        .flat_map(|arg| extract_passed_variables_from_text(arg))
        .collect::<Vec<_>>();
    dedup_variables(&mut variables);
    variables
}

fn expr_literal_default(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    let raw = span_text(expr.span(), source).trim().to_string();
    literal_default_from_text(&raw)
}

fn literal_default_from_text(raw: &str) -> Option<String> {
    let raw = raw.trim().to_string();
    if raw.is_empty() || raw.starts_with('$') {
        return None;
    }
    Some(raw)
}

fn apply_view_usages(
    views: &mut [ViewEntry],
    usage_map: HashMap<String, Vec<ViewUsage>>,
    project: &LaravelProject,
    view_namespaces: &HashMap<String, PathBuf>,
) -> Vec<MissingViewEntry> {
    let mut remaining = usage_map;
    for view in views {
        if let Some(mut usages) = remaining.remove(&view.name) {
            for usage in &mut usages {
                dedup_variables(&mut usage.variables);
            }
            let mut variables = usages
                .iter()
                .flat_map(|usage| usage.variables.clone())
                .collect::<Vec<_>>();
            dedup_variables(&mut variables);
            view.variables = variables;
            view.usages = usages;
        }
        dedup_variables(&mut view.props);
    }

    let mut missing_views = remaining
        .into_iter()
        .map(|(name, mut usages)| {
            for usage in &mut usages {
                dedup_variables(&mut usage.variables);
            }
            usages.sort_by(|l, r| {
                l.source
                    .declared_in
                    .cmp(&r.source.declared_in)
                    .then(l.source.line.cmp(&r.source.line))
                    .then(l.source.column.cmp(&r.source.column))
                    .then(l.kind.cmp(&r.kind))
            });
            MissingViewEntry {
                expected_file: expected_view_file(project, view_namespaces, &name),
                name,
                usages,
            }
        })
        .collect::<Vec<_>>();

    missing_views.sort_by(|l, r| l.name.cmp(&r.name));
    missing_views
}

fn parse_blade_props(file: &Path) -> Vec<ViewVariable> {
    let Ok(source) = fs::read_to_string(file) else {
        return Vec::new();
    };
    let Some(start) = source.find("@props(") else {
        return Vec::new();
    };
    let body = extract_balanced_segment(&source[start + "@props".len()..], '(', ')');
    let Some(body) = body else {
        return Vec::new();
    };
    let trimmed = body.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Vec::new();
    }
    parse_array_like_variables(&trimmed[1..trimmed.len() - 1])
}

fn parse_class_component_props(file: &Path) -> Vec<ViewVariable> {
    let Ok(source) = fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut props = parse_public_properties(&source);
    props.extend(parse_constructor_promoted_properties(&source));
    dedup_variables(&mut props);
    props
}

fn parse_livewire_state(file: &Path) -> Vec<ViewVariable> {
    let Ok(source) = fs::read_to_string(file) else {
        return Vec::new();
    };
    let mut props = parse_public_properties(&source);
    dedup_variables(&mut props);
    props
}

fn parse_public_properties(source: &str) -> Vec<ViewVariable> {
    let mut props = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("public ")
            || trimmed.contains(" function ")
            || trimmed.starts_with("public function")
        {
            continue;
        }
        if let Some(dollar) = trimmed.find('$') {
            let after = &trimmed[dollar + 1..];
            let name: String = after
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let default_value = trimmed
                .split_once('=')
                .map(|(_, value)| value.trim().trim_end_matches(';').trim().to_string())
                .filter(|value| !value.is_empty());
            props.push(ViewVariable {
                name,
                default_value,
            });
        }
    }
    props
}

fn parse_constructor_promoted_properties(source: &str) -> Vec<ViewVariable> {
    let Some(start) = source.find("function __construct") else {
        return Vec::new();
    };
    let Some(body) = extract_balanced_segment(&source[start..], '(', ')') else {
        return Vec::new();
    };
    let mut props = Vec::new();
    for param in split_top_level(body, ',') {
        let trimmed = param.trim();
        if !(trimmed.contains("public ")
            || trimmed.starts_with("public")
            || trimmed.contains("public readonly"))
        {
            continue;
        }
        if let Some(dollar) = trimmed.find('$') {
            let after = &trimmed[dollar + 1..];
            let name: String = after
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect();
            if name.is_empty() {
                continue;
            }
            let default_value = trimmed
                .split_once('=')
                .map(|(_, value)| value.trim().to_string())
                .filter(|value| !value.is_empty());
            props.push(ViewVariable {
                name,
                default_value,
            });
        }
    }
    props
}

fn parse_array_like_variables(body: &str) -> Vec<ViewVariable> {
    split_top_level(body, ',')
        .into_iter()
        .filter_map(|entry| {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return None;
            }
            if let Some((name, default)) = trimmed.split_once("=>") {
                Some(ViewVariable {
                    name: trim_quotes(name.trim()).to_string(),
                    default_value: Some(default.trim().to_string()),
                })
            } else {
                Some(ViewVariable {
                    name: trim_quotes(trimmed).to_string(),
                    default_value: None,
                })
            }
        })
        .collect()
}

fn extract_balanced_segment(source: &str, open: char, close: char) -> Option<&str> {
    let start = source.find(open)?;
    let mut depth = 0usize;
    let mut start_index = None;
    for (index, ch) in source.char_indices().skip(start) {
        if ch == open {
            depth += 1;
            if depth == 1 {
                start_index = Some(index + ch.len_utf8());
            }
        } else if ch == close {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                let begin = start_index?;
                return source.get(begin..index);
            }
        }
    }
    None
}

fn find_matching_delimiter(
    source: &str,
    open_index: usize,
    open: char,
    close: char,
) -> Option<usize> {
    if source[open_index..].chars().next()? != open {
        return None;
    }

    let mut depth = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for (relative, ch) in source[open_index..].char_indices() {
        let index = open_index + relative;

        if in_single {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }

        if in_double {
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            _ if ch == open => depth += 1,
            _ if ch == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_top_level(source: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren = 0usize;
    let mut bracket = 0usize;
    let mut brace = 0usize;
    let mut in_single = false;
    let mut in_double = false;
    let mut escape = false;

    for ch in source.chars() {
        if in_single {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '\'' {
                in_single = false;
            }
            continue;
        }
        if in_double {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_double = false;
            }
            continue;
        }

        match ch {
            '\'' => {
                in_single = true;
                current.push(ch);
            }
            '"' => {
                in_double = true;
                current.push(ch);
            }
            '(' => {
                paren += 1;
                current.push(ch);
            }
            ')' => {
                paren = paren.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                bracket += 1;
                current.push(ch);
            }
            ']' => {
                bracket = bracket.saturating_sub(1);
                current.push(ch);
            }
            '{' => {
                brace += 1;
                current.push(ch);
            }
            '}' => {
                brace = brace.saturating_sub(1);
                current.push(ch);
            }
            _ if ch == separator && paren == 0 && bracket == 0 && brace == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

fn strip_wrapping_parens(mut value: &str) -> &str {
    loop {
        let trimmed = value.trim();
        if !trimmed.starts_with('(') {
            return trimmed;
        }
        let Some(end) = find_matching_delimiter(trimmed, 0, '(', ')') else {
            return trimmed;
        };
        if end != trimmed.len() - 1 {
            return trimmed;
        }
        value = &trimmed[1..end];
    }
}

fn extract_function_call_body<'a>(source: &'a str, names: &[&str]) -> Option<&'a str> {
    let trimmed = strip_wrapping_parens(source);
    for name in names {
        let Some(rest) = trimmed.strip_prefix(name) else {
            continue;
        };
        if !rest.starts_with('(') {
            continue;
        }
        let open_index = trimmed.len() - rest.len();
        let close_index = find_matching_delimiter(trimmed, open_index, '(', ')')?;
        if close_index == trimmed.len() - 1 {
            return trimmed.get(open_index + 1..close_index);
        }
    }
    None
}

fn extract_string_literal(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.len() < 2 {
        return None;
    }
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return Some(trim_quotes(trimmed).to_string());
    }
    None
}

fn extract_string_list_from_text(value: &str) -> Vec<String> {
    let trimmed = strip_wrapping_parens(value.trim());
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return split_top_level(&trimmed[1..trimmed.len() - 1], ',')
            .into_iter()
            .filter_map(|entry| extract_string_literal(&entry))
            .collect();
    }

    extract_string_literal(trimmed).into_iter().collect()
}

fn skip_ascii_whitespace(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let ch = source[index..].chars().next().unwrap_or('\0');
        if !ch.is_ascii_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn trim_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn dedup_variables(variables: &mut Vec<ViewVariable>) {
    let mut seen = BTreeSet::new();
    variables.retain(|entry| seen.insert((entry.name.clone(), entry.default_value.clone())));
}

fn resolve_render_view_name(
    project: &LaravelProject,
    class_name: &str,
    mappings: &[crate::php::psr4::Psr4Mapping],
) -> Option<String> {
    let file = resolve_class_file(class_name, mappings)?;
    let source = fs::read(&file).ok()?;
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return None;
    }

    let mut found = None;
    for stmt in program.statements.iter() {
        let Stmt::Class { members, .. } = stmt else {
            continue;
        };
        for member in members.iter().copied() {
            let ClassMember::Method { name, body, .. } = member else {
                continue;
            };
            if crate::php::ast::span_text(name.span, &source) != "render" {
                continue;
            }
            for stmt in body {
                if let Stmt::Return {
                    expr: Some(inner), ..
                } = stmt
                {
                    found = extract_view_name(*inner, &source);
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
    }

    found.or_else(|| {
        let relative = strip_root(&project.root, &file);
        let path = relative.to_string_lossy();
        if let Some(rest) = path.strip_prefix("app/Livewire/") {
            Some(format!(
                "livewire.{}",
                path_to_dot(rest).trim_end_matches(".php")
            ))
        } else if let Some(rest) = path.strip_prefix("app/Http/Livewire/") {
            Some(format!(
                "livewire.{}",
                path_to_dot(rest).trim_end_matches(".php")
            ))
        } else {
            None
        }
    })
}

fn extract_view_name(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::Call { func, args, .. } => {
            let func_name = expr_name(func, source)?;
            if func_name == "view" {
                args.first().and_then(|a| expr_to_string(a.value, source))
            } else {
                None
            }
        }
        Expr::MethodCall { target, method, .. } => {
            let method_name = expr_name(method, source)?;
            if method_name == "with" || method_name == "layout" {
                extract_view_name(target, source)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn default_view_namespaces(project: &LaravelProject) -> HashMap<String, PathBuf> {
    let mut namespaces = HashMap::new();
    let vendor_root = project.root.join("resources/views/vendor");
    if let Ok(entries) = fs::read_dir(&vendor_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            namespaces.insert(name.to_string(), path);
        }
    }
    namespaces
}

fn resolve_view_file(
    project: &LaravelProject,
    view_namespaces: &HashMap<String, PathBuf>,
    view_name: &str,
) -> Option<PathBuf> {
    let file = expected_view_file_absolute(project, view_namespaces, view_name);
    file.is_file().then(|| strip_root(&project.root, &file))
}

fn expected_view_file(
    project: &LaravelProject,
    view_namespaces: &HashMap<String, PathBuf>,
    view_name: &str,
) -> PathBuf {
    strip_root(
        &project.root,
        &expected_view_file_absolute(project, view_namespaces, view_name),
    )
}

fn expected_view_file_absolute(
    project: &LaravelProject,
    view_namespaces: &HashMap<String, PathBuf>,
    view_name: &str,
) -> PathBuf {
    let (namespace, path) = view_name.split_once("::").unwrap_or(("", view_name));
    let relative = PathBuf::from(path.replace('.', "/") + ".blade.php");

    if namespace.is_empty() {
        return project.root.join("resources/views").join(relative);
    }

    let published = project
        .root
        .join("resources/views/vendor")
        .join(namespace)
        .join(&relative);
    if published.is_file() {
        return published;
    }

    if let Some(root) = view_namespaces.get(namespace) {
        return root.join(relative);
    }

    published
}

fn collect_blade_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_blade_files(&path));
            } else if path.to_string_lossy().ends_with(".blade.php") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn collect_php_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(collect_php_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("php") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

fn blade_view_name(root: &Path, file: &Path, namespace: Option<&str>) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file).to_string_lossy();
    let view = relative
        .trim_end_matches(".blade.php")
        .replace('/', ".")
        .replace('\\', ".");
    match namespace {
        Some(prefix) => format!("{prefix}::{view}"),
        None => view,
    }
}

fn derive_component_alias(root: &Path, file: &Path, prefix: Option<&str>) -> String {
    let relative = file.strip_prefix(root).unwrap_or(file).to_string_lossy();
    let stem = relative
        .trim_end_matches(".blade.php")
        .trim_end_matches(".php");
    let alias = path_to_dot(stem);
    match prefix {
        Some(prefix) if prefix.contains("::") => format!("{prefix}.{alias}"),
        Some(prefix) => format!("{prefix}::{alias}"),
        None => alias,
    }
}

fn derive_class_component_view_name(root: &Path, file: &Path) -> String {
    format!(
        "components.{}",
        path_to_dot(
            file.strip_prefix(root)
                .unwrap_or(file)
                .to_string_lossy()
                .trim_end_matches(".php")
        )
    )
}

fn path_to_dot(path: &str) -> String {
    path.split(&['/', '\\'][..])
        .filter(|part| !part.is_empty())
        .map(|part| kebab_case(part))
        .collect::<Vec<_>>()
        .join(".")
}

fn kebab_case(input: &str) -> String {
    let mut output = String::new();
    let mut prev_lower = false;
    for ch in input.chars() {
        if ch == '_' || ch == '-' {
            if !output.ends_with('-') {
                output.push('-');
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() {
            if prev_lower && !output.ends_with('-') {
                output.push('-');
            }
            for lower in ch.to_lowercase() {
                output.push(lower);
            }
            prev_lower = false;
        } else {
            output.push(ch);
            prev_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        }
    }
    output
}

fn last_class_segment(class_name: &str) -> &str {
    class_name.rsplit('\\').next().unwrap_or(class_name)
}

fn class_name_for_path(
    project_root: &Path,
    file: &Path,
    mappings: &[crate::php::psr4::Psr4Mapping],
) -> Option<String> {
    for mapping in mappings {
        if file.starts_with(&mapping.base_dir) {
            let relative = file.strip_prefix(&mapping.base_dir).ok()?;
            let stem = relative
                .to_string_lossy()
                .trim_end_matches(".php")
                .replace('/', "\\");
            return Some(format!(
                "{}{}",
                mapping.prefix.trim_end_matches('\\'),
                if stem.is_empty() {
                    "".to_string()
                } else {
                    format!("\\{stem}")
                }
            ));
        }
    }
    Some(
        strip_root(project_root, file)
            .to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches(".php")
            .to_string(),
    )
}

fn dedup_views(views: &mut Vec<ViewEntry>) {
    let mut seen = BTreeSet::new();
    views.retain(|entry| seen.insert((entry.name.clone(), entry.file.clone(), entry.kind.clone())));
}

fn dedup_blade_components(components: &mut Vec<BladeComponentEntry>) {
    let mut seen = BTreeSet::new();
    components.retain(|entry| {
        seen.insert((
            entry.component.clone(),
            entry.kind.clone(),
            entry.class_name.clone(),
            entry.view_file.clone(),
        ))
    });
}

fn dedup_livewire_components(components: &mut Vec<LivewireComponentEntry>) {
    let mut seen = BTreeSet::new();
    components.retain(|entry| {
        seen.insert((
            entry.component.clone(),
            entry.kind.clone(),
            entry.class_name.clone(),
            entry.view_name.clone(),
        ))
    });
}
