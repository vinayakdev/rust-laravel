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
    pub line: usize,
    pub column: usize,
    pub registration_kind: String,
    pub registration_declared_in: String,
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
    doc = doc.field("Source", format!("{}:{}:{}", input.source, input.line, input.column));
    doc = doc.blank().field(
        "Registration",
        format!(
            "{} from {}",
            input.registration_kind, input.registration_declared_in
        ),
    );

    DocBundle::new(input.name, doc).with_detail(input.detail.unwrap_or_default())
}
