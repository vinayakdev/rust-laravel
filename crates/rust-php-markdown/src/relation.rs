use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

pub struct RelationHoverInput {
    pub method: String,
    pub relation_type: String,
    pub related_model: String,
    pub model_uri: Option<String>,
}

pub fn build(input: RelationHoverInput) -> DocBundle {
    let mut doc = MarkdownDoc::new()
        .title(&input.method)
        .blank()
        .separator()
        .blank()
        .field("Type", &input.relation_type)
        .blank();

    if let Some(uri) = input.model_uri.as_deref() {
        doc = doc.link_field("Model", &input.related_model, uri);
    } else {
        doc = doc.field("Model", &input.related_model);
    }

    DocBundle::new(input.method, doc)
}
