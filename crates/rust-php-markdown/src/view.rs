use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct ViewHoverInput {
    pub name: String,
    pub kind: String,
    pub file: String,
    pub usages: usize,
    pub props: Vec<String>,
    pub detail: Option<String>,
}

pub fn build(input: ViewHoverInput) -> DocBundle {
    let mut doc = MarkdownDoc::new()
        .title(&input.name)
        .blank()
        .separator()
        .blank()
        .field("Kind", input.kind)
        .blank()
        .field("File", input.file);

    if input.usages > 0 {
        doc = doc.field("Usages", input.usages.to_string()).blank();
    }
    if !input.props.is_empty() {
        doc = doc.field("Props", input.props.join(", "));
    }

    DocBundle::new(input.name, doc).with_detail(input.detail.unwrap_or_default())
}
