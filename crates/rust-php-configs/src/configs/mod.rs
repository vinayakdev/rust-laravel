mod collector;
mod extractor;

use collector::collect_registered_config_files;
use extractor::find_config_items;

use crate::types::ConfigReport;
use rust_php_foundation::overrides::FileOverrides;
use rust_php_foundation::php::env::load_env_map_with;
use rust_php_foundation::project::LaravelProject;
use rust_php_foundation::types::ProviderEntry;

pub fn analyze(
    project: &LaravelProject,
    providers: &[ProviderEntry],
    overrides: &FileOverrides,
) -> Result<ConfigReport, String> {
    let env = load_env_map_with(&project.root, |path| overrides.get_string(path))?;
    let config_files = collect_registered_config_files(project, providers, overrides)?;
    let mut items = Vec::new();

    for registered in config_files {
        let source = overrides.get_string(&registered.file).map_or_else(
            || {
                std::fs::read_to_string(&registered.file)
                    .map_err(|e| format!("failed to read {}: {e}", registered.file.display()))
            },
            Ok,
        )?;

        items.extend(find_config_items(
            &project.root,
            &registered.file,
            &source,
            &registered.namespace,
            &env,
            &registered.source,
        ));
    }

    items.sort_by(|l, r| {
        l.file
            .cmp(&r.file)
            .then(l.line.cmp(&r.line))
            .then(l.column.cmp(&r.column))
            .then(l.key.cmp(&r.key))
            .then(l.source.declared_in.cmp(&r.source.declared_in))
    });

    Ok(ConfigReport {
        project_name: project.name.clone(),
        project_root: project.root.clone(),
        item_count: items.len(),
        items,
    })
}
