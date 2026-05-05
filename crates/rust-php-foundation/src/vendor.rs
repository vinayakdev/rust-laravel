use crate::php::ast::span_text;
use bumpalo::Bump;
use php_parser::ast::{ClassMember, Name, Stmt, Type, UseKind};
use php_parser::lexer::Lexer;
use php_parser::lexer::token::Token;
use php_parser::parser::Parser;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub fn load_classmap(project_root: &Path) -> HashMap<String, PathBuf> {
    let vendor_dir = project_root.join("vendor");
    let base_dir = project_root;
    let classmap_path = vendor_dir.join("composer/autoload_classmap.php");

    let Ok(content) = fs::read_to_string(&classmap_path) else {
        return HashMap::new();
    };

    let vendor_str = vendor_dir.display().to_string();
    let base_str = base_dir.display().to_string();
    let mut map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        let Some(fqn) = extract_quoted(line) else {
            continue;
        };
        let full_path = if line.contains("$vendorDir") {
            extract_path(line, "$vendorDir . '").map(|p| format!("{vendor_str}{p}"))
        } else if line.contains("$baseDir") {
            extract_path(line, "$baseDir . '").map(|p| format!("{base_str}{p}"))
        } else {
            None
        };
        if let Some(path) = full_path {
            map.insert(fqn, PathBuf::from(path));
        }
    }

    map
}

#[derive(Debug)]
pub struct VendorClassIndex {
    classmap: HashMap<String, PathBuf>,
    parsed_cache: Mutex<HashMap<String, CachedVendorClass>>,
}

#[derive(Clone, Debug)]
struct CachedVendorClass {
    stamp: FileStamp,
    parsed: ParsedVendorClass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl VendorClassIndex {
    pub fn load(project_root: &Path) -> Self {
        Self::from_classmap(load_classmap(project_root))
    }

    pub fn from_classmap(classmap: HashMap<String, PathBuf>) -> Self {
        Self {
            classmap,
            parsed_cache: Mutex::new(HashMap::new()),
        }
    }

    pub fn classmap(&self) -> &HashMap<String, PathBuf> {
        &self.classmap
    }

    pub fn class_path(&self, fqn: &str) -> Option<&Path> {
        self.classmap.get(fqn).map(PathBuf::as_path)
    }

    pub fn collect_chainable_methods(&self, fqn: &str) -> Vec<String> {
        collect_chainable_methods(fqn, &self.classmap)
    }

    pub fn collect_chainable_methods_with_source(&self, fqn: &str) -> Vec<(String, String)> {
        collect_chainable_methods_with_source(fqn, &self.classmap)
    }

    pub fn collect_class_properties(&self, fqn: &str) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut properties = Vec::new();
        self.collect_properties_recursive(fqn, &mut visited, &mut properties);
        properties.dedup();
        properties
    }

    pub fn collect_class_properties_with_source(&self, fqn: &str) -> Vec<(String, String)> {
        let mut visited = HashSet::new();
        let mut properties = Vec::new();
        self.collect_properties_with_source_recursive(fqn, &mut visited, &mut properties);
        properties
    }

    pub fn collect_class_property_stubs_with_source(
        &self,
        fqn: &str,
    ) -> Vec<(String, String, String)> {
        let mut visited = HashSet::new();
        let mut out = Vec::new();
        self.collect_property_stubs_recursive(fqn, &mut visited, &mut out);
        out
    }

    pub fn warm_common_laravel_classes(&self) {
        const COMMON_CLASSES: &[&str] = &[
            "Illuminate\\Database\\Eloquent\\Model",
            "Illuminate\\Foundation\\Auth\\User",
            "Illuminate\\Database\\Eloquent\\Relations\\Pivot",
            "Illuminate\\Database\\Eloquent\\Relations\\MorphPivot",
        ];

        for class in COMMON_CLASSES {
            let _ = self.collect_class_property_stubs_with_source(class);
            let _ = self.collect_chainable_methods(class);
        }
    }

    fn parse_vendor_class(&self, class_fqn: &str) -> Option<ParsedVendorClass> {
        let path = self.classmap.get(class_fqn)?;
        let stamp = file_stamp(path)?;

        if let Some(parsed) = self.cached_vendor_class(class_fqn, &stamp) {
            return Some(parsed);
        }

        let source = fs::read(path).ok()?;
        let parsed = parse_vendor_class_from_source(class_fqn, &source)?;
        self.store_vendor_class(class_fqn, stamp, parsed.clone());
        Some(parsed)
    }

    fn cached_vendor_class(&self, class_fqn: &str, stamp: &FileStamp) -> Option<ParsedVendorClass> {
        let cache = self.parsed_cache.lock().ok()?;
        let cached = cache.get(class_fqn)?;
        if cached.stamp == *stamp {
            Some(cached.parsed.clone())
        } else {
            None
        }
    }

    fn store_vendor_class(&self, class_fqn: &str, stamp: FileStamp, parsed: ParsedVendorClass) {
        let Ok(mut cache) = self.parsed_cache.lock() else {
            return;
        };
        cache.insert(class_fqn.to_string(), CachedVendorClass { stamp, parsed });
    }

    fn collect_properties_with_source_recursive(
        &self,
        class_fqn: &str,
        visited: &mut HashSet<String>,
        properties: &mut Vec<(String, String)>,
    ) {
        if !visited.insert(class_fqn.to_string()) {
            return;
        }
        let short = class_fqn
            .split('\\')
            .last()
            .unwrap_or(class_fqn)
            .to_string();
        let Some(parsed) = self.parse_vendor_class(class_fqn) else {
            return;
        };

        for property in parsed.properties {
            properties.push((property.name, short.clone()));
        }
        for trait_fqn in parsed.used_traits {
            self.collect_properties_with_source_recursive(&trait_fqn, visited, properties);
        }
        if let Some(parent_fqn) = parsed.parent_fqn {
            self.collect_properties_with_source_recursive(&parent_fqn, visited, properties);
        }
    }

    fn collect_properties_recursive(
        &self,
        class_fqn: &str,
        visited: &mut HashSet<String>,
        properties: &mut Vec<String>,
    ) {
        if !visited.insert(class_fqn.to_string()) {
            return;
        }
        let Some(parsed) = self.parse_vendor_class(class_fqn) else {
            return;
        };

        for property in parsed.properties {
            properties.push(property.name);
        }
        for fqn in parsed.used_traits {
            self.collect_properties_recursive(&fqn, visited, properties);
        }
        if let Some(fqn) = parsed.parent_fqn {
            self.collect_properties_recursive(&fqn, visited, properties);
        }
    }

    fn collect_property_stubs_recursive(
        &self,
        class_fqn: &str,
        visited: &mut HashSet<String>,
        out: &mut Vec<(String, String, String)>,
    ) {
        if !visited.insert(class_fqn.to_string()) {
            return;
        }
        let short = class_fqn
            .split('\\')
            .last()
            .unwrap_or(class_fqn)
            .to_string();
        let Some(parsed) = self.parse_vendor_class(class_fqn) else {
            return;
        };

        for property in parsed.properties {
            out.push((property.name, property.stub, short.clone()));
        }
        for fqn in parsed.used_traits {
            self.collect_property_stubs_recursive(&fqn, visited, out);
        }
        if let Some(fqn) = parsed.parent_fqn {
            self.collect_property_stubs_recursive(&fqn, visited, out);
        }
    }
}

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileStamp {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

pub fn collect_chainable_methods(fqn: &str, classmap: &HashMap<String, PathBuf>) -> Vec<String> {
    let mut visited = HashSet::new();
    let mut methods = Vec::new();
    collect_methods_recursive(fqn, classmap, &mut visited, &mut methods);
    methods.dedup();
    methods
}

pub fn collect_chainable_methods_with_source(
    fqn: &str,
    classmap: &HashMap<String, PathBuf>,
) -> Vec<(String, String)> {
    let mut visited = HashSet::new();
    let mut methods = Vec::new();
    collect_methods_with_source_recursive(fqn, classmap, &mut visited, &mut methods);
    methods
}

fn collect_methods_with_source_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    methods: &mut Vec<(String, String)>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else {
        return;
    };
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);
    let short = class_fqn
        .split('\\')
        .last()
        .unwrap_or(class_fqn)
        .to_string();

    for name in parse_chainable_methods(&source) {
        methods.push((name, short.clone()));
    }

    for trait_short in parse_used_traits(&source) {
        let trait_fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_methods_with_source_recursive(&trait_fqn, classmap, visited, methods);
    }

    if let Some(parent_short) = parse_extends(&source) {
        let parent_fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_methods_with_source_recursive(&parent_fqn, classmap, visited, methods);
    }
}

fn collect_methods_recursive(
    class_fqn: &str,
    classmap: &HashMap<String, PathBuf>,
    visited: &mut HashSet<String>,
    methods: &mut Vec<String>,
) {
    if !visited.insert(class_fqn.to_string()) {
        return;
    }
    let Some(path) = classmap.get(class_fqn) else {
        return;
    };
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };

    let namespace = parse_namespace(&source);
    let use_map = parse_use_statements(&source);

    for name in parse_chainable_methods(&source) {
        methods.push(name);
    }

    for trait_short in parse_used_traits(&source) {
        let fqn = resolve_fqn(&trait_short, &namespace, &use_map);
        collect_methods_recursive(&fqn, classmap, visited, methods);
    }

    if let Some(parent_short) = parse_extends(&source) {
        let fqn = resolve_fqn(&parent_short, &namespace, &use_map);
        collect_methods_recursive(&fqn, classmap, visited, methods);
    }
}

fn parse_namespace(source: &str) -> String {
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("namespace ") && t.ends_with(';') {
            return t[10..t.len() - 1].to_string();
        }
    }
    String::new()
}

fn parse_use_statements(source: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("class ")
            || t.starts_with("abstract class ")
            || t.starts_with("trait ")
            || t.starts_with("interface ")
        {
            break;
        }
        if t.starts_with("use ") && t.ends_with(';') && !t.contains('{') {
            let inner = &t[4..t.len() - 1];
            if let Some((fqn, alias)) = inner.split_once(" as ") {
                map.insert(alias.trim().to_string(), fqn.trim().to_string());
            } else {
                let fqn = inner.trim().to_string();
                let short = fqn.split('\\').last().unwrap_or(&fqn).to_string();
                map.insert(short, fqn);
            }
        }
    }
    map
}

fn parse_used_traits(source: &str) -> Vec<String> {
    let mut traits = Vec::new();
    let mut in_body = false;
    let mut depth = 0i32;

    for line in source.lines() {
        let t = line.trim();
        if !in_body {
            if t.contains('{')
                && (t.starts_with("class ")
                    || t.starts_with("abstract class ")
                    || t.starts_with("trait ")
                    || t.starts_with("interface "))
            {
                in_body = true;
                depth = 1;
                continue;
            }
            if t == "{" {
                in_body = true;
                depth = 1;
                continue;
            }
            continue;
        }
        for ch in t.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        if depth == 1 && t.starts_with("use ") && t.ends_with(';') && !t.contains('(') {
            let inner = &t[4..t.len() - 1];
            for part in inner.split(',') {
                let name = part.trim().split('{').next().unwrap_or(part).trim();
                if !name.is_empty() {
                    traits.push(name.to_string());
                }
            }
        }
    }
    traits
}

fn parse_extends(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        if (t.starts_with("class ") || t.starts_with("abstract class ")) && t.contains("extends") {
            let after = t.split("extends").nth(1)?;
            let parent = after
                .split_whitespace()
                .next()?
                .trim_matches(|c| c == '{' || c == ',');
            return Some(parent.to_string());
        }
    }
    None
}

fn parse_chainable_methods(source: &str) -> Vec<String> {
    let mut methods = Vec::new();
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("public function ")
            && (t.contains("): static") || t.contains("): self"))
            && !t.contains("abstract ")
        {
            let after = &t["public function ".len()..];
            if let Some(name) = after.split('(').next() {
                let name = name.trim();
                if !name.is_empty() {
                    methods.push(name.to_string());
                }
            }
        }
    }
    methods
}

#[derive(Clone, Debug)]
struct ParsedVendorClass {
    parent_fqn: Option<String>,
    used_traits: Vec<String>,
    properties: Vec<ParsedVendorProperty>,
}

#[derive(Clone, Debug)]
struct ParsedVendorProperty {
    name: String,
    stub: String,
}

fn parse_vendor_class_from_source(class_fqn: &str, source: &[u8]) -> Option<ParsedVendorClass> {
    let arena = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    find_vendor_class_in_stmts(&program.statements, source, class_fqn, "", &HashMap::new())
}

fn find_vendor_class_in_stmts(
    stmts: &[php_parser::ast::StmtId<'_>],
    source: &[u8],
    target_fqn: &str,
    current_namespace: &str,
    inherited_imports: &HashMap<String, String>,
) -> Option<ParsedVendorClass> {
    let mut namespace = current_namespace.to_string();
    let mut imports = inherited_imports.clone();

    for stmt in stmts.iter().copied() {
        match stmt {
            Stmt::Namespace { name, body, .. } => {
                let next_namespace = name
                    .map(|value| name_to_string(&value, source))
                    .unwrap_or_default();
                if let Some(body) = body {
                    if let Some(parsed) = find_vendor_class_in_stmts(
                        body,
                        source,
                        target_fqn,
                        &next_namespace,
                        &HashMap::new(),
                    ) {
                        return Some(parsed);
                    }
                } else {
                    namespace = next_namespace;
                    imports.clear();
                }
            }
            Stmt::Use { uses, kind, .. } if *kind == UseKind::Normal => {
                for item in *uses {
                    let fqn = name_to_string(&item.name, source);
                    let key = if let Some(alias) = item.alias {
                        span_text(alias.span, source)
                    } else {
                        fqn.rsplit('\\').next().unwrap_or(&fqn).to_string()
                    };
                    imports.insert(key, fqn);
                }
            }
            Stmt::Class {
                name,
                extends,
                members,
                ..
            } => {
                if qualify_name(&namespace, &span_text(name.span, source)) == target_fqn {
                    return Some(build_parsed_vendor_class(
                        members,
                        extends.as_ref(),
                        source,
                        &namespace,
                        &imports,
                    ));
                }
            }
            Stmt::Trait { name, members, .. } => {
                if qualify_name(&namespace, &span_text(name.span, source)) == target_fqn {
                    return Some(build_parsed_vendor_class(
                        members, None, source, &namespace, &imports,
                    ));
                }
            }
            _ => {}
        }
    }

    None
}

fn build_parsed_vendor_class(
    members: &[ClassMember<'_>],
    extends: Option<&Name<'_>>,
    source: &[u8],
    namespace: &str,
    imports: &HashMap<String, String>,
) -> ParsedVendorClass {
    let parent_fqn =
        extends.map(|name| resolve_fqn(&name_to_string(name, source), namespace, imports));
    let mut used_traits = Vec::new();
    let mut properties = Vec::new();

    for member in members {
        match member {
            ClassMember::Property {
                modifiers,
                ty,
                entries,
                ..
            } => collect_property_entries(&mut properties, modifiers, *ty, entries, source),
            ClassMember::PropertyHook {
                modifiers,
                ty,
                name,
                ..
            } => collect_property_hook(&mut properties, modifiers, *ty, name, source),
            ClassMember::TraitUse { traits, .. } => {
                for used_trait in *traits {
                    used_traits.push(resolve_fqn(
                        &name_to_string(&used_trait, source),
                        namespace,
                        imports,
                    ));
                }
            }
            _ => {}
        }
    }

    ParsedVendorClass {
        parent_fqn,
        used_traits,
        properties,
    }
}

fn collect_property_entries(
    out: &mut Vec<ParsedVendorProperty>,
    modifiers: &[Token],
    ty: Option<&Type<'_>>,
    entries: &[php_parser::ast::PropertyEntry<'_>],
    source: &[u8],
) {
    let modifiers = modifiers
        .iter()
        .map(|token| span_text(token.span, source))
        .collect::<Vec<_>>();
    let ty = ty.map(|ty| type_to_string(ty, source));

    for entry in entries {
        let name = span_text(entry.name.span, source)
            .trim_start_matches('$')
            .to_string();
        if name.is_empty() || name.starts_with('_') {
            continue;
        }
        out.push(ParsedVendorProperty {
            stub: build_property_stub(&modifiers, ty.as_deref(), &name),
            name,
        });
    }
}

fn collect_property_hook(
    out: &mut Vec<ParsedVendorProperty>,
    modifiers: &[Token],
    ty: Option<&Type<'_>>,
    name: &Token,
    source: &[u8],
) {
    let property_name = span_text(name.span, source)
        .trim_start_matches('$')
        .to_string();
    if property_name.is_empty() || property_name.starts_with('_') {
        return;
    }

    let modifiers = modifiers
        .iter()
        .map(|token| span_text(token.span, source))
        .collect::<Vec<_>>();
    let ty = ty.map(|ty| type_to_string(ty, source));

    out.push(ParsedVendorProperty {
        stub: build_property_stub(&modifiers, ty.as_deref(), &property_name),
        name: property_name,
    });
}

fn build_property_stub(modifiers: &[String], ty: Option<&str>, name: &str) -> String {
    let mut parts = Vec::with_capacity(3);
    if !modifiers.is_empty() {
        parts.push(modifiers.join(" "));
    }
    if let Some(ty) = ty {
        parts.push(ty.to_string());
    }
    parts.push(format!("${name}"));
    parts.join(" ")
}

fn type_to_string(ty: &Type<'_>, source: &[u8]) -> String {
    match ty {
        Type::Simple(token) => span_text(token.span, source),
        Type::Name(name) => name_to_string(name, source),
        Type::Union(types) => types
            .iter()
            .map(|inner| type_to_string(inner, source))
            .collect::<Vec<_>>()
            .join("|"),
        Type::Intersection(types) => types
            .iter()
            .map(|inner| type_to_string(inner, source))
            .collect::<Vec<_>>()
            .join("&"),
        Type::Nullable(inner) => format!("?{}", type_to_string(inner, source)),
    }
}

fn name_to_string(name: &Name<'_>, source: &[u8]) -> String {
    name.parts
        .iter()
        .map(|token| span_text(token.span, source))
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

pub fn collect_class_properties(fqn: &str, classmap: &HashMap<String, PathBuf>) -> Vec<String> {
    VendorClassIndex::from_classmap(classmap.clone()).collect_class_properties(fqn)
}

pub fn collect_class_properties_with_source(
    fqn: &str,
    classmap: &HashMap<String, PathBuf>,
) -> Vec<(String, String)> {
    VendorClassIndex::from_classmap(classmap.clone()).collect_class_properties_with_source(fqn)
}

/// Returns `(name, stub, source_class)` tuples walking the full hierarchy.
/// `stub` is the declaration up to (but not including) `=`, e.g.
/// `"protected static ?string $model"`.
pub fn collect_class_property_stubs_with_source(
    fqn: &str,
    classmap: &HashMap<String, PathBuf>,
) -> Vec<(String, String, String)> {
    VendorClassIndex::from_classmap(classmap.clone()).collect_class_property_stubs_with_source(fqn)
}

pub fn parse_namespace_pub(source: &str) -> String {
    parse_namespace(source)
}

pub fn parse_extends_pub(source: &str) -> Option<String> {
    parse_extends(source)
}

pub fn parse_file_use_statements(source: &str) -> HashMap<String, String> {
    parse_use_statements(source)
}

pub fn resolve_fqn(short: &str, namespace: &str, use_map: &HashMap<String, String>) -> String {
    if short.starts_with('\\') {
        return short[1..].to_string();
    }
    let root = short.split('\\').next().unwrap_or(short);
    if let Some(fqn) = use_map.get(root) {
        let rest: &str = short.splitn(2, '\\').nth(1).unwrap_or("");
        if rest.is_empty() {
            return fqn.clone();
        }
        let base_ns = fqn.rsplitn(2, '\\').last().unwrap_or(fqn.as_str());
        return format!("{base_ns}\\{rest}");
    }
    if namespace.is_empty() {
        short.to_string()
    } else {
        format!("{namespace}\\{short}")
    }
}

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('\'')? + 1;
    let end = s[start..].find('\'')? + start;
    Some(s[start..end].replace("\\\\", "\\"))
}

fn extract_path<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let start = line.find(marker)? + marker.len();
    let end = line[start..].find('\'')?;
    Some(&line[start..start + end])
}

#[cfg(test)]
mod tests {
    use super::{VendorClassIndex, parse_vendor_class_from_source};
    use std::collections::HashMap;
    use std::fs;

    #[test]
    fn parses_multiline_trait_use_blocks() {
        let source = br#"<?php
namespace Illuminate\Database\Eloquent;

abstract class Model
{
    use Concerns\HasAttributes,
        Concerns\GuardsAttributes,
        Concerns\HidesAttributes;
}
"#;

        let parsed =
            parse_vendor_class_from_source("Illuminate\\Database\\Eloquent\\Model", source)
                .unwrap();

        assert_eq!(
            parsed.used_traits,
            vec![
                "Illuminate\\Database\\Eloquent\\Concerns\\HasAttributes",
                "Illuminate\\Database\\Eloquent\\Concerns\\GuardsAttributes",
                "Illuminate\\Database\\Eloquent\\Concerns\\HidesAttributes",
            ]
        );
    }

    #[test]
    fn ignores_method_parameters_when_collecting_properties() {
        let source = br#"<?php
namespace Illuminate\Database\Eloquent\Concerns;

trait GuardsAttributes
{
    protected $fillable = [];
    protected $guarded = ['*'];

    public function fill(array $attributes)
    {
        return $this;
    }
}
"#;

        let parsed = parse_vendor_class_from_source(
            "Illuminate\\Database\\Eloquent\\Concerns\\GuardsAttributes",
            source,
        )
        .unwrap();

        let names = parsed
            .properties
            .iter()
            .map(|property| property.name.as_str())
            .collect::<Vec<_>>();
        let stubs = parsed
            .properties
            .iter()
            .map(|property| property.stub.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["fillable", "guarded"]);
        assert_eq!(stubs, vec!["protected $fillable", "protected $guarded"]);
    }

    #[test]
    fn reparses_cached_vendor_class_when_file_stamp_changes() {
        let dir =
            std::env::temp_dir().join(format!("rust-php-vendor-cache-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Foo.php");
        fs::write(
            &file,
            "<?php\nnamespace App;\nclass Foo\n{\n    protected $first = [];\n}\n",
        )
        .unwrap();

        let index = VendorClassIndex::from_classmap(HashMap::from([(
            "App\\Foo".to_string(),
            file.clone(),
        )]));
        assert_eq!(index.collect_class_properties("App\\Foo"), vec!["first"]);

        fs::write(
            &file,
            "<?php\nnamespace App;\nclass Foo\n{\n    protected $secondProperty = [];\n}\n",
        )
        .unwrap();

        assert_eq!(
            index.collect_class_properties("App\\Foo"),
            vec!["secondProperty"]
        );

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(dir);
    }
}
