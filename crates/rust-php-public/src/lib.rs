pub mod types;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use rust_php_foundation::php::ast::{byte_offset_to_line_col, strip_root};
use rust_php_foundation::project::LaravelProject;

use crate::types::{PublicAssetEntry, PublicAssetReport, PublicAssetUsage};

pub fn analyze(project: &LaravelProject) -> Result<PublicAssetReport, String> {
    let public_root = project.root.join("public");
    let mut assets = collect_public_assets(project, &public_root)?;
    let usages = collect_asset_usages(project)?;
    let mut usage_by_asset = usages.into_iter().fold(
        BTreeMap::<String, Vec<PublicAssetUsage>>::new(),
        |mut acc, usage| {
            acc.entry(usage.asset_path.clone()).or_default().push(usage);
            acc
        },
    );

    for asset in &mut assets {
        asset.usages = usage_by_asset.remove(&asset.asset_path).unwrap_or_default();
        asset.usages.sort_by(|left, right| {
            left.file
                .cmp(&right.file)
                .then(left.line.cmp(&right.line))
                .then(left.column.cmp(&right.column))
        });
    }

    let usage_count = assets.iter().map(|asset| asset.usages.len()).sum();

    Ok(PublicAssetReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        file_count: assets.len(),
        usage_count,
        assets,
    })
}

fn collect_public_assets(
    project: &LaravelProject,
    public_root: &Path,
) -> Result<Vec<PublicAssetEntry>, String> {
    if !public_root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    collect_files_recursive(public_root, &mut files, |path| path.is_file())?;

    let mut assets = files
        .into_iter()
        .filter_map(|path| build_public_asset_entry(project, &path).ok())
        .collect::<Vec<_>>();

    assets.sort_by(|left, right| left.asset_path.cmp(&right.asset_path));
    Ok(assets)
}

fn build_public_asset_entry(
    project: &LaravelProject,
    path: &Path,
) -> Result<PublicAssetEntry, String> {
    let file = strip_root(&project.root, path);
    let asset_path = strip_root(&project.root.join("public"), path)
        .to_string_lossy()
        .replace('\\', "/");
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;

    Ok(PublicAssetEntry {
        file,
        asset_path,
        extension: path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase()),
        size_bytes: metadata.len(),
        usages: Vec::new(),
    })
}

fn collect_asset_usages(project: &LaravelProject) -> Result<Vec<PublicAssetUsage>, String> {
    let mut files = Vec::new();
    collect_files_recursive(&project.root, &mut files, |path| is_source_file(path))?;

    let mut usages = Vec::new();
    for path in files {
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let relative = strip_root(&project.root, &path);
        let source_kind = source_kind(&relative).to_string();

        usages.extend(find_asset_usages(&source).into_iter().filter_map(|usage| {
            let normalized = normalize_asset_path(&usage.raw_reference)?;
            Some(PublicAssetUsage {
                helper: usage.helper,
                source_kind: source_kind.clone(),
                file: relative.clone(),
                line: usage.line,
                column: usage.column,
                raw_reference: usage.raw_reference,
                asset_path: normalized,
            })
        }));
    }

    usages.sort_by(|left, right| {
        left.asset_path
            .cmp(&right.asset_path)
            .then(left.file.cmp(&right.file))
            .then(left.line.cmp(&right.line))
            .then(left.column.cmp(&right.column))
    });
    Ok(usages)
}

fn collect_files_recursive(
    root: &Path,
    out: &mut Vec<PathBuf>,
    include: impl Fn(&Path) -> bool + Copy,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();

        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_files_recursive(&path, out, include)?;
            continue;
        }

        if include(&path) {
            out.push(path);
        }
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|value| value.to_str()),
        Some(".git" | ".next" | "node_modules" | "public" | "storage" | "vendor")
    )
}

fn is_source_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };

    file_name.ends_with(".blade.php")
        || path.extension().and_then(|value| value.to_str()) == Some("php")
}

fn source_kind(path: &Path) -> &'static str {
    if path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.ends_with(".blade.php"))
    {
        "blade"
    } else {
        "php"
    }
}

#[derive(Debug)]
struct RawUsage {
    helper: String,
    raw_reference: String,
    line: usize,
    column: usize,
}

fn find_asset_usages(source: &str) -> Vec<RawUsage> {
    let bytes = source.as_bytes();
    let mut usages = Vec::new();
    let mut index = 0usize;

    while index < bytes.len() {
        let Some((helper, helper_len)) = match_helper_at(bytes, index) else {
            index += 1;
            continue;
        };

        if !has_word_boundary_before(bytes, index) {
            index += 1;
            continue;
        }

        let mut cursor = index + helper_len;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'(') {
            index += 1;
            continue;
        }

        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }

        let Some(&quote) = bytes.get(cursor) else {
            index += 1;
            continue;
        };
        if quote != b'\'' && quote != b'"' {
            index += 1;
            continue;
        }

        let literal_start = cursor + 1;
        let mut literal_end = literal_start;
        while let Some(&current) = bytes.get(literal_end) {
            if current == quote && !is_escaped(bytes, literal_end) {
                break;
            }
            literal_end += 1;
        }

        if bytes.get(literal_end) != Some(&quote) {
            index += 1;
            continue;
        }

        let raw_reference = source[literal_start..literal_end].to_string();
        let (line, column) = byte_offset_to_line_col(bytes, literal_start);

        usages.push(RawUsage {
            helper: helper.to_string(),
            raw_reference,
            line,
            column,
        });

        index = literal_end + 1;
    }

    usages
}

fn match_helper_at(bytes: &[u8], index: usize) -> Option<(&'static str, usize)> {
    if bytes[index..].starts_with(b"secure_asset") {
        return Some(("secure_asset", "secure_asset".len()));
    }
    if bytes[index..].starts_with(b"asset") {
        return Some(("asset", "asset".len()));
    }
    None
}

fn has_word_boundary_before(bytes: &[u8], index: usize) -> bool {
    if index == 0 {
        return true;
    }

    !matches!(
        bytes[index - 1],
        b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
    )
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let mut slash_count = 0usize;
    let mut cursor = index;

    while cursor > 0 {
        cursor -= 1;
        if bytes[cursor] != b'\\' {
            break;
        }
        slash_count += 1;
    }

    slash_count % 2 == 1
}

fn normalize_asset_path(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains("://") || trimmed.starts_with("//") {
        return None;
    }

    let candidate = trimmed
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_start_matches('/');
    if candidate.is_empty() {
        return None;
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(candidate).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => normalized.push(".."),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }

    let normalized = normalized.to_string_lossy().replace('\\', "/");
    (!normalized.is_empty()).then_some(normalized)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rust_php_foundation::project;

    use super::{analyze, find_asset_usages, normalize_asset_path};

    #[test]
    fn finds_asset_and_secure_asset_references() {
        let source = r#"
<?php
return secure_asset('assets/images/virtual/landing.png');
{{ asset("/assets/icons/logo.svg") }}
"#;

        let usages = find_asset_usages(source);
        assert_eq!(usages.len(), 2);
        assert_eq!(usages[0].helper, "secure_asset");
        assert_eq!(usages[0].raw_reference, "assets/images/virtual/landing.png");
        assert_eq!(usages[1].helper, "asset");
        assert_eq!(usages[1].raw_reference, "/assets/icons/logo.svg");
    }

    #[test]
    fn normalizes_asset_paths() {
        assert_eq!(
            normalize_asset_path("/assets/images/virtual/landing.png?v=1"),
            Some("assets/images/virtual/landing.png".to_string())
        );
        assert_eq!(
            normalize_asset_path("https://cdn.example.com/file.png"),
            None
        );
    }

    #[test]
    fn builds_public_asset_report_with_usages() {
        let root = unique_temp_project_root();
        fs::create_dir_all(root.join("config")).expect("config dir");
        fs::create_dir_all(root.join("routes")).expect("routes dir");
        fs::create_dir_all(root.join("public/assets/images/virtual")).expect("public dir");
        fs::create_dir_all(root.join("resources/views")).expect("views dir");

        fs::write(root.join("composer.json"), "{\"autoload\":{}}").expect("composer");
        fs::write(root.join("config/app.php"), "<?php return [];").expect("config");
        fs::write(root.join("routes/web.php"), "<?php").expect("routes");
        fs::write(root.join("public/assets/images/virtual/landing.png"), "png")
            .expect("public asset");
        fs::write(
            root.join("resources/views/welcome.blade.php"),
            "{{ secure_asset('assets/images/virtual/landing.png') }}",
        )
        .expect("blade view");

        let project = project::from_root(&root).expect("project");
        let report = analyze(&project).expect("report");

        assert_eq!(report.file_count, 1);
        assert_eq!(report.usage_count, 1);
        assert_eq!(
            report.assets[0].asset_path,
            "assets/images/virtual/landing.png"
        );
        assert_eq!(report.assets[0].usages[0].helper, "secure_asset");
        assert!(
            report.assets[0].usages[0]
                .file
                .to_string_lossy()
                .ends_with("resources/views/welcome.blade.php")
        );
    }

    fn unique_temp_project_root() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("rust-php-public-{nanos}"))
    }
}
