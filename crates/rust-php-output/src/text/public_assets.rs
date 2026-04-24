use comfy_table::Table;

use crate::text::{header, join_or_dash, new_table, wrap_cell};
use crate::types::PublicAssetReport;

pub fn print_public_asset_report(report: &PublicAssetReport) {
    println!("{}", render_public_asset_report(report));
}

pub fn render_public_asset_report(report: &PublicAssetReport) -> String {
    let mut table = new_table();
    table.set_header(vec![
        header("Asset Path"),
        header("File"),
        header("Ext"),
        header("Size"),
        header("Usages"),
        header("Used In"),
    ]);

    for asset in &report.assets {
        let used_in = if asset.usages.is_empty() {
            Vec::new()
        } else {
            asset
                .usages
                .iter()
                .map(|usage| {
                    format!(
                        "{}:{}:{} ({})",
                        usage.file.display(),
                        usage.line,
                        usage.column,
                        usage.helper
                    )
                })
                .collect::<Vec<_>>()
        };

        table.add_row(vec![
            wrap_cell(&asset.asset_path, 40),
            wrap_cell(&asset.file.display().to_string(), 42),
            wrap_cell(asset.extension.as_deref().unwrap_or("-"), 10),
            wrap_cell(&format!("{} B", asset.size_bytes), 12),
            wrap_cell(&asset.usages.len().to_string(), 8),
            wrap_cell(&join_or_dash(&used_in), 64),
        ]);
    }

    summarize(report, table)
}

fn summarize(report: &PublicAssetReport, table: Table) -> String {
    format!(
        "{}\n\nPublic files: {}\nMatched asset usages: {}",
        table, report.file_count, report.usage_count
    )
}
