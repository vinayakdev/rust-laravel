use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct AssetHoverInput {
    pub asset_path: String,
    pub file_name: Option<String>,
    pub file_uri: Option<String>,
    pub size_display: Option<String>,
    pub extension: Option<String>,
    pub usages: usize,
    pub status: AssetStatus,
    pub completion_detail: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetStatus {
    Resolved,
    Missing,
    Unresolved,
}

pub fn build(input: AssetHoverInput) -> DocBundle {
    let asset_path = input.asset_path.clone();
    let title = input
        .file_name
        .clone()
        .unwrap_or_else(|| asset_path.clone());
    let size_display = input.size_display.clone();
    let file_uri = input.file_uri.clone();
    let extension = input.extension.clone();
    let usages = input.usages;
    let completion_detail = input.completion_detail;

    let hover = match input.status {
        AssetStatus::Resolved => {
            let mut doc = MarkdownDoc::new()
                .title(title.clone())
                .blank()
                .separator()
                .blank();

            if let Some(size_display) = size_display.clone() {
                doc = doc.field("Size", size_display).blank();
            }

            if let Some(file_uri) = file_uri.clone() {
                doc = doc.link_field("File", asset_path.clone(), file_uri);
            } else {
                doc = doc.line(format!("File: `{asset_path}`"));
            }

            doc
        }
        AssetStatus::Missing => MarkdownDoc::new()
            .title(&asset_path)
            .blank()
            .field("Status", "missing"),
        AssetStatus::Unresolved => MarkdownDoc::new()
            .title(&asset_path)
            .blank()
            .field("Status", "unresolved"),
    };

    let completion = {
        let mut doc = MarkdownDoc::new().title(title).blank().separator().blank();
        if let Some(file_uri) = file_uri.as_deref() {
            doc = doc
                .link_field("Path", format!("public/{asset_path}"), file_uri)
                .blank();
        } else {
            doc = doc.field("Path", format!("public/{asset_path}")).blank();
        }

        if let Some(extension) = extension {
            doc = doc.field("Extension", format!(".{extension}")).blank();
        }

        if let Some(size_display) = size_display {
            doc = doc.field("Size", size_display);
        }

        if usages > 0 {
            doc = doc.blank().field("Usages", usages.to_string());
        }

        doc
    };

    DocBundle::new(asset_path, hover)
        .with_detail(completion_detail)
        .with_completion(completion)
}
