use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct EnvHoverInput {
    pub key: String,
    pub value: String,
    pub source: String,
    pub line: usize,
    pub column: usize,
    pub detail: Option<String>,
}

pub fn build(input: EnvHoverInput) -> DocBundle {
    let doc = MarkdownDoc::new()
        .title(&input.key)
        .blank()
        .separator()
        .blank()
        .field("Value", input.value)
        .blank()
        .field("Source", format!("{}:{}:{}", input.source, input.line, input.column));

    DocBundle::new(input.key, doc).with_detail(input.detail.unwrap_or_default())
}
