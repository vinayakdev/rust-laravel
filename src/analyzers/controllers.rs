use bumpalo::Bump;
use php_parser::ast::{ClassMember, Name, Stmt, UseKind};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::lsp::overrides::FileOverrides;
use crate::php::ast::{byte_offset_to_line_col, span_text, strip_root};
use crate::php::psr4::{Psr4Mapping, collect_psr4_mappings};
use crate::project::LaravelProject;
use crate::types::{
    ControllerEntry, ControllerMethodEntry, ControllerReport, RouteControllerTarget,
};

#[derive(Clone)]
struct MethodDef {
    name: String,
    line: usize,
    visibility: String,
    is_static: bool,
}

#[derive(Clone)]
struct TypeDef {
    fqn: String,
    short_name: String,
    namespace: String,
    file: PathBuf,
    line: usize,
    end_line: usize,
    kind: TypeKind,
    extends: Option<String>,
    traits: Vec<String>,
    methods: Vec<MethodDef>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeKind {
    Class,
    Trait,
}

#[derive(Clone)]
struct FlattenedMethod {
    name: String,
    declared_in: PathBuf,
    line: usize,
    visibility: String,
    is_static: bool,
    source_kind: String,
    source_name: String,
}

pub fn analyze(project: &LaravelProject) -> Result<ControllerReport, String> {
    analyze_with_overrides(project, &FileOverrides::default())
}

pub fn analyze_with_overrides(
    project: &LaravelProject,
    overrides: &FileOverrides,
) -> Result<ControllerReport, String> {
    let mappings = collect_psr4_mappings(&project.root)?;
    let defs = collect_type_defs(project, &mappings, overrides);

    let mut controllers = defs
        .values()
        .filter(|def| def.kind == TypeKind::Class)
        .filter(|def| looks_like_controller(def, &defs))
        .map(|def| build_controller_entry(project, def, &defs))
        .collect::<Vec<_>>();

    controllers.sort_by(|left, right| left.fqn.cmp(&right.fqn));

    Ok(ControllerReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        controller_count: controllers.len(),
        controllers,
    })
}

pub fn resolve_route_target(
    report: &ControllerReport,
    action: &str,
) -> Option<RouteControllerTarget> {
    let (controller_name, method_name) = if let Some((controller, method)) = action.split_once('@')
    {
        (controller.trim(), method.trim())
    } else if action.trim().ends_with("Controller") {
        (action.trim(), "__invoke")
    } else {
        return None;
    };

    let candidates = controller_candidates(report, controller_name);
    if candidates.is_empty() {
        if !controller_name.ends_with("Controller") {
            return None;
        }
        return Some(RouteControllerTarget {
            controller: controller_name.to_string(),
            method: method_name.to_string(),
            declared_in: None,
            method_declared_in: None,
            method_line: None,
            accessible_from_route: false,
            status: "missing-controller".to_string(),
            source_kind: None,
            notes: vec!["Controller class was not found in scanned PSR-4 sources.".to_string()],
        });
    }

    if candidates.len() > 1 {
        return Some(RouteControllerTarget {
            controller: controller_name.to_string(),
            method: method_name.to_string(),
            declared_in: None,
            method_declared_in: None,
            method_line: None,
            accessible_from_route: false,
            status: "ambiguous-controller".to_string(),
            source_kind: None,
            notes: vec![format!(
                "Multiple controller candidates matched `{controller_name}`: {}",
                candidates
                    .iter()
                    .map(|controller| controller.fqn.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )],
        });
    }

    let controller = candidates[0];
    let method = controller
        .methods
        .iter()
        .find(|item| item.name == method_name);

    match method {
        Some(method) if method.accessible_from_route => Some(RouteControllerTarget {
            controller: controller.fqn.clone(),
            method: method_name.to_string(),
            declared_in: Some(controller.file.clone()),
            method_declared_in: Some(method.declared_in.clone()),
            method_line: Some(method.line),
            accessible_from_route: true,
            status: "ok".to_string(),
            source_kind: Some(method.source_kind.clone()),
            notes: vec![method.accessibility.clone()],
        }),
        Some(method) => Some(RouteControllerTarget {
            controller: controller.fqn.clone(),
            method: method_name.to_string(),
            declared_in: Some(controller.file.clone()),
            method_declared_in: Some(method.declared_in.clone()),
            method_line: Some(method.line),
            accessible_from_route: false,
            status: "not-route-callable".to_string(),
            source_kind: Some(method.source_kind.clone()),
            notes: vec![method.accessibility.clone()],
        }),
        None => Some(RouteControllerTarget {
            controller: controller.fqn.clone(),
            method: method_name.to_string(),
            declared_in: Some(controller.file.clone()),
            method_declared_in: None,
            method_line: None,
            accessible_from_route: false,
            status: "missing-method".to_string(),
            source_kind: None,
            notes: vec![format!(
                "Method `{method_name}` was not found on `{}` or its scanned traits/parents.",
                controller.fqn
            )],
        }),
    }
}

fn controller_candidates<'a>(
    report: &'a ControllerReport,
    controller_name: &str,
) -> Vec<&'a ControllerEntry> {
    let normalized = controller_name.trim_start_matches('\\');
    let short_name = normalized.rsplit('\\').next().unwrap_or(normalized);

    let exact_fqn = report
        .controllers
        .iter()
        .filter(|controller| controller.fqn == normalized)
        .collect::<Vec<_>>();
    if !exact_fqn.is_empty() {
        return exact_fqn;
    }

    let exact_short = report
        .controllers
        .iter()
        .filter(|controller| controller.class_name == short_name)
        .collect::<Vec<_>>();
    if !exact_short.is_empty() {
        return exact_short;
    }

    report
        .controllers
        .iter()
        .filter(|controller| controller.fqn.ends_with(&format!("\\{normalized}")))
        .collect()
}

fn build_controller_entry(
    project: &LaravelProject,
    def: &TypeDef,
    defs: &HashMap<String, TypeDef>,
) -> ControllerEntry {
    let methods = flatten_methods(def, defs)
        .into_iter()
        .map(|method| {
            let accessibility = accessibility_reason(&method);
            let accessible_from_route = accessibility == "public instance method";
            ControllerMethodEntry {
                name: method.name,
                declared_in: method.declared_in,
                line: method.line,
                visibility: method.visibility,
                is_static: method.is_static,
                source_kind: method.source_kind,
                source_name: method.source_name,
                accessible_from_route,
                accessibility,
            }
        })
        .collect::<Vec<_>>();

    let callable_method_count = methods
        .iter()
        .filter(|method| method.accessible_from_route)
        .count();

    ControllerEntry {
        file: strip_root(&project.root, &def.file),
        line: def.line,
        class_end_line: def.end_line,
        class_name: def.short_name.clone(),
        namespace: def.namespace.clone(),
        fqn: def.fqn.clone(),
        extends: def.extends.clone(),
        traits: def.traits.clone(),
        method_count: methods.len(),
        callable_method_count,
        methods,
    }
}

fn accessibility_reason(method: &FlattenedMethod) -> String {
    if method.name == "__construct" {
        return "constructors are not route actions".to_string();
    }
    if method.name.starts_with("__") && method.name != "__invoke" {
        return "magic methods are not route actions".to_string();
    }
    if method.visibility != "public" {
        return format!("{} methods are not callable from routes", method.visibility);
    }
    if method.is_static {
        return "static methods are not callable as controller actions".to_string();
    }
    "public instance method".to_string()
}

fn flatten_methods(def: &TypeDef, defs: &HashMap<String, TypeDef>) -> Vec<FlattenedMethod> {
    let mut methods = Vec::new();
    let mut seen_names = HashSet::new();
    let mut seen_classes = HashSet::new();
    let mut seen_traits = HashSet::new();

    collect_class_methods(
        def,
        defs,
        false,
        &mut seen_names,
        &mut seen_classes,
        &mut seen_traits,
        &mut methods,
    );

    methods.sort_by(|left, right| left.name.cmp(&right.name).then(left.line.cmp(&right.line)));
    methods
}

fn collect_class_methods(
    def: &TypeDef,
    defs: &HashMap<String, TypeDef>,
    inherited: bool,
    seen_names: &mut HashSet<String>,
    seen_classes: &mut HashSet<String>,
    seen_traits: &mut HashSet<String>,
    methods: &mut Vec<FlattenedMethod>,
) {
    if !seen_classes.insert(def.fqn.clone()) {
        return;
    }

    for method in &def.methods {
        if inherited && method.visibility == "private" {
            continue;
        }
        if seen_names.insert(method.name.clone()) {
            methods.push(FlattenedMethod {
                name: method.name.clone(),
                declared_in: def.file.clone(),
                line: method.line,
                visibility: method.visibility.clone(),
                is_static: method.is_static,
                source_kind: if inherited {
                    "parent".to_string()
                } else {
                    "class".to_string()
                },
                source_name: def.fqn.clone(),
            });
        }
    }

    for trait_name in &def.traits {
        collect_trait_methods(
            trait_name,
            defs,
            inherited,
            seen_names,
            seen_traits,
            methods,
        );
    }

    if let Some(parent) = def.extends.as_ref().and_then(|name| defs.get(name)) {
        collect_class_methods(
            parent,
            defs,
            true,
            seen_names,
            seen_classes,
            seen_traits,
            methods,
        );
    }
}

fn collect_trait_methods(
    trait_name: &str,
    defs: &HashMap<String, TypeDef>,
    inherited: bool,
    seen_names: &mut HashSet<String>,
    seen_traits: &mut HashSet<String>,
    methods: &mut Vec<FlattenedMethod>,
) {
    let Some(def) = defs.get(trait_name) else {
        return;
    };
    if def.kind != TypeKind::Trait || !seen_traits.insert(def.fqn.clone()) {
        return;
    }

    for method in &def.methods {
        if seen_names.insert(method.name.clone()) {
            methods.push(FlattenedMethod {
                name: method.name.clone(),
                declared_in: def.file.clone(),
                line: method.line,
                visibility: method.visibility.clone(),
                is_static: method.is_static,
                source_kind: if inherited {
                    "parent-trait".to_string()
                } else {
                    "trait".to_string()
                },
                source_name: def.fqn.clone(),
            });
        }
    }

    for nested_trait in &def.traits {
        collect_trait_methods(
            nested_trait,
            defs,
            inherited,
            seen_names,
            seen_traits,
            methods,
        );
    }
}

fn looks_like_controller(def: &TypeDef, defs: &HashMap<String, TypeDef>) -> bool {
    if def.file.to_string_lossy().contains("/Http/Controllers/") {
        return true;
    }
    if def.short_name.ends_with("Controller") {
        return true;
    }

    let mut cursor = def.extends.as_deref();
    while let Some(parent_name) = cursor {
        if parent_name.ends_with("Controller") {
            return true;
        }
        cursor = defs
            .get(parent_name)
            .and_then(|parent| parent.extends.as_deref());
    }

    false
}

fn collect_type_defs(
    project: &LaravelProject,
    mappings: &[Psr4Mapping],
    overrides: &FileOverrides,
) -> HashMap<String, TypeDef> {
    let mut files = Vec::new();
    let mut seen_dirs = BTreeSet::new();

    for mapping in mappings {
        let dir = &mapping.base_dir;
        if !dir.is_dir() {
            continue;
        }
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        if !seen_dirs.insert(canonical) {
            continue;
        }
        collect_php_files_recursive(dir, &mut files);
    }

    let mut defs = HashMap::new();
    for file in files {
        for def in parse_file_defs(project, &file, overrides) {
            defs.insert(def.fqn.clone(), def);
        }
    }
    defs
}

fn collect_php_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_php_files_recursive(&path, out);
        } else if path.extension().and_then(|value| value.to_str()) == Some("php") {
            out.push(path);
        }
    }
}

fn parse_file_defs(
    project: &LaravelProject,
    file: &Path,
    overrides: &FileOverrides,
) -> Vec<TypeDef> {
    let Some(source) = overrides.get_bytes(file).or_else(|| fs::read(file).ok()) else {
        return Vec::new();
    };
    let arena = Bump::new();
    let lexer = Lexer::new(&source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    if !program.errors.is_empty() {
        return Vec::new();
    }

    let mut namespace = String::new();
    let mut imports = HashMap::new();
    let mut defs = Vec::new();

    for stmt in program.statements.iter() {
        match stmt {
            Stmt::Namespace { name, .. } => {
                namespace = name
                    .as_ref()
                    .map(|value| name_to_string(value, &source))
                    .unwrap_or_default();
            }
            Stmt::Use { uses, kind, .. } => {
                if *kind != UseKind::Normal {
                    continue;
                }
                for item in *uses {
                    let fqn = name_to_string(&item.name, &source);
                    let alias = item
                        .alias
                        .map(|value| span_text(value.span, &source))
                        .unwrap_or_else(|| fqn.rsplit('\\').next().unwrap_or(&fqn).to_string());
                    imports.insert(alias, fqn);
                }
            }
            Stmt::Class {
                name,
                extends,
                members,
                span,
                ..
            } => {
                let short_name = span_text(name.span, &source);
                let (line, _) = byte_offset_to_line_col(&source, span.start);
                let (end_line, _) = byte_offset_to_line_col(&source, span.end.saturating_sub(1));
                defs.push(TypeDef {
                    fqn: qualify_name(&namespace, &short_name),
                    short_name,
                    namespace: namespace.clone(),
                    file: strip_root(&project.root, file),
                    line,
                    end_line,
                    kind: TypeKind::Class,
                    extends: extends.as_ref().map(|value| {
                        resolve_name(&name_to_string(value, &source), &namespace, &imports)
                    }),
                    traits: collect_trait_uses(members, &source, &namespace, &imports),
                    methods: collect_methods(members, &source),
                });
            }
            Stmt::Trait {
                name,
                members,
                span,
                ..
            } => {
                let short_name = span_text(name.span, &source);
                let (line, _) = byte_offset_to_line_col(&source, span.start);
                let (end_line, _) = byte_offset_to_line_col(&source, span.end.saturating_sub(1));
                defs.push(TypeDef {
                    fqn: qualify_name(&namespace, &short_name),
                    short_name,
                    namespace: namespace.clone(),
                    file: strip_root(&project.root, file),
                    line,
                    end_line,
                    kind: TypeKind::Trait,
                    extends: None,
                    traits: collect_trait_uses(members, &source, &namespace, &imports),
                    methods: collect_methods(members, &source),
                });
            }
            _ => {}
        }
    }

    defs
}

fn collect_methods(members: &[ClassMember<'_>], source: &[u8]) -> Vec<MethodDef> {
    let mut methods = Vec::new();

    for member in members {
        let ClassMember::Method {
            name,
            modifiers,
            span,
            ..
        } = member
        else {
            continue;
        };
        let (line, _) = byte_offset_to_line_col(source, span.start);
        let modifier_text = modifiers
            .iter()
            .map(|token| span_text(token.span, source))
            .collect::<Vec<_>>();
        let visibility = if modifier_text.iter().any(|modifier| modifier == "private") {
            "private".to_string()
        } else if modifier_text.iter().any(|modifier| modifier == "protected") {
            "protected".to_string()
        } else {
            "public".to_string()
        };
        let is_static = modifier_text.iter().any(|modifier| modifier == "static");

        methods.push(MethodDef {
            name: span_text(name.span, source),
            line,
            visibility,
            is_static,
        });
    }

    methods
}

fn collect_trait_uses(
    members: &[ClassMember<'_>],
    source: &[u8],
    namespace: &str,
    imports: &HashMap<String, String>,
) -> Vec<String> {
    let mut traits = Vec::new();

    for member in members {
        let ClassMember::TraitUse { traits: used, .. } = member else {
            continue;
        };

        for item in used.iter() {
            traits.push(resolve_name(
                &name_to_string(item, source),
                namespace,
                imports,
            ));
        }
    }

    traits
}

fn name_to_string(name: &Name<'_>, source: &[u8]) -> String {
    name.parts
        .iter()
        .map(|part| span_text(part.span, source))
        .collect::<String>()
        .trim_start_matches('\\')
        .to_string()
}

fn qualify_name(namespace: &str, short_name: &str) -> String {
    if namespace.is_empty() {
        short_name.to_string()
    } else {
        format!("{namespace}\\{short_name}")
    }
}

fn resolve_name(raw: &str, namespace: &str, imports: &HashMap<String, String>) -> String {
    let normalized = raw.trim_start_matches('\\');
    if normalized.contains('\\') {
        let head = normalized.split('\\').next().unwrap_or(normalized);
        if let Some(prefix) = imports.get(head) {
            let suffix = normalized.strip_prefix(head).unwrap_or_default();
            return format!("{prefix}{suffix}");
        }
        return normalized.to_string();
    }
    if let Some(imported) = imports.get(normalized) {
        return imported.clone();
    }
    if namespace.is_empty() {
        normalized.to_string()
    } else {
        format!("{namespace}\\{normalized}")
    }
}
