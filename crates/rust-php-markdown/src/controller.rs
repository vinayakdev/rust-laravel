use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct ControllerHoverInput {
    pub label: String,
    pub fqn: String,
    pub callable_methods: usize,
    pub total_methods: usize,
    pub source: String,
    pub line: usize,
    pub extends: Option<String>,
    pub traits: Vec<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ControllerMethodHoverInput {
    pub label: String,
    pub controller_fqn: String,
    pub route_callable: bool,
    pub visibility: String,
    pub source_kind: String,
    pub notes: String,
    pub source: String,
    pub line: usize,
    pub detail: Option<String>,
}

pub fn build(input: ControllerHoverInput) -> DocBundle {
    let mut doc = MarkdownDoc::new()
        .title(&input.fqn)
        .blank()
        .separator()
        .blank()
        .field("Callable methods", input.callable_methods.to_string())
        .blank()
        .field("Total methods", input.total_methods.to_string())
        .blank()
        .field("Source", format!("{}:{}", input.source, input.line));

    if let Some(parent) = input.extends.as_deref() {
        doc = doc.field("Extends", parent).blank();
    }
    if !input.traits.is_empty() {
        doc = doc.field("Traits", input.traits.join(", "));
    }

    DocBundle::new(input.label, doc).with_detail(input.detail.unwrap_or(input.fqn))
}

pub fn build_method(input: ControllerMethodHoverInput) -> DocBundle {
    let doc = MarkdownDoc::new()
        .title(&input.label)
        .blank()
        .separator()
        .blank()
        .field("Controller", input.controller_fqn)
        .blank()
        .field("Route callable", input.route_callable.to_string())
        .blank()
        .field("Visibility", input.visibility)
        .blank()
        .field("Source kind", input.source_kind)
        .blank()
        .field("Notes", input.notes)
        .blank()
        .field("Source", format!("{}:{}", input.source, input.line));

    DocBundle::new(input.label, doc).with_detail(input.detail.unwrap_or_default())
}
