use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct EnvHoverInput {
    pub key: String,
    pub value: String,
    pub source: String,
    pub source_uri: Option<String>,
    pub line: usize,
    pub column: usize,
    pub detail: Option<String>,
}

pub fn build(input: EnvHoverInput) -> DocBundle {
    let mut doc = MarkdownDoc::new()
        .title(&input.key)
        .blank()
        .separator()
        .blank()
        .field("Value", input.value)
        .blank();

    let source_text = format!("{}:{}:{}", input.source, input.line, input.column);
    if let Some(source_uri) = input.source_uri.as_deref() {
        doc = doc.link_field("Source", source_text, source_uri);
    } else {
        doc = doc.field("Source", source_text);
    }

    DocBundle::new(input.key, doc).with_detail(input.detail.unwrap_or_default())
}
