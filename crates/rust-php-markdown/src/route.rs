use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct RouteHoverInput {
    pub name: String,
    pub methods: Vec<String>,
    pub uri: String,
    pub action: Option<String>,
    pub resolved_middleware: Vec<String>,
    pub parameter_patterns: Vec<(String, String)>,
    pub source: String,
    pub source_uri: Option<String>,
    pub line: usize,
    pub column: usize,
    pub detail: Option<String>,
}

pub fn build(input: RouteHoverInput) -> DocBundle {
    let mut doc = MarkdownDoc::new()
        .title(&input.name)
        .blank()
        .separator()
        .blank()
        .field("Methods", input.methods.join(", "))
        .blank()
        .field("Uri", input.uri);

    if let Some(action) = input.action.as_deref() {
        doc = doc.field("Action", action).blank();
    }
    if !input.resolved_middleware.is_empty() {
        doc = doc
            .field("Middleware", input.resolved_middleware.join(", "))
            .blank();
    }
    if !input.parameter_patterns.is_empty() {
        let patterns = input
            .parameter_patterns
            .iter()
            .map(|(name, pattern)| format!("{name}={pattern}"))
            .collect::<Vec<_>>()
            .join(", ");
        doc = doc.field("Parameter patterns", patterns).blank();
    }
    if let Some(source_uri) = input.source_uri.as_deref() {
        doc = doc.link_field(
            "Source",
            format!("{}:{}:{}", input.source, input.line, input.column),
            source_uri,
        );
    } else {
        doc = doc.field("Source", format!("{}:{}:{}", input.source, input.line, input.column));
    }

    DocBundle::new(input.name, doc).with_detail(input.detail.unwrap_or_default())
}
