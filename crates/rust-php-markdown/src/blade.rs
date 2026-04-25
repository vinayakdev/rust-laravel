use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct BladeComponentHoverInput {
    pub component: String,
    pub class_name: Option<String>,
    pub class_uri: Option<String>,
    pub view_file: Option<String>,
    pub view_file_uri: Option<String>,
    pub props: Vec<String>,
    pub detail: Option<String>,
}

pub fn build(input: BladeComponentHoverInput) -> DocBundle {
    let title = format!("x-{}", input.component);
    let mut doc = MarkdownDoc::new()
        .title(&title)
        .blank()
        .separator()
        .blank();

    if let Some(class) = input.class_name.as_deref() {
        if let Some(uri) = input.class_uri.as_deref() {
            doc = doc.link_field("Class", class, uri).blank();
        } else {
            doc = doc.field("Class", class).blank();
        }
    }

    if let Some(file) = input.view_file.as_deref() {
        if let Some(uri) = input.view_file_uri.as_deref() {
            doc = doc.link_field("Blade file", file, uri).blank();
        } else {
            doc = doc.field("Blade file", file).blank();
        }
    }

    if !input.props.is_empty() {
        doc = doc.field("Props", input.props.join(", "));
    }

    DocBundle::new(title, doc).with_detail(input.detail.unwrap_or_default())
}

#[derive(Clone, Debug)]
pub struct LivewireComponentHoverInput {
    pub component: String,
    pub class_name: Option<String>,
    pub class_uri: Option<String>,
    pub view_name: Option<String>,
    pub view_file: Option<String>,
    pub view_file_uri: Option<String>,
    pub properties: Vec<String>,
    pub actions: Vec<String>,
    pub detail: Option<String>,
}

pub fn build_livewire(input: LivewireComponentHoverInput) -> DocBundle {
    let title = input
        .class_name
        .as_deref()
        .and_then(livewire_short_class_name)
        .unwrap_or(input.component.as_str())
        .to_string();

    let mut doc = MarkdownDoc::new()
        .title(&title)
        .blank()
        .separator()
        .blank();

    if let Some(class) = input.class_name.as_deref() {
        if let Some(uri) = input.class_uri.as_deref() {
            doc = doc.link_field("Class", class, uri).blank();
        } else {
            doc = doc.field("Class", class).blank();
        }
    }

    if let Some(file) = input.view_file.as_deref() {
        if let Some(uri) = input.view_file_uri.as_deref() {
            doc = doc.link_field("Blade file", file, uri).blank();
        } else {
            doc = doc.field("Blade file", file).blank();
        }
    }

    let has_members = !input.properties.is_empty() || !input.actions.is_empty();
    if has_members {
        let mut lines: Vec<String> = input
            .properties
            .iter()
            .map(|p| format!("public ${p};"))
            .collect();

        if !input.properties.is_empty() && !input.actions.is_empty() {
            lines.push(String::new());
        }

        for action in &input.actions {
            lines.push(format!("public function {action}();"));
        }

        doc = doc.code_block("php", lines.join("\n"));
    }

    DocBundle::new(title, doc).with_detail(input.detail.unwrap_or_default())
}

fn livewire_short_class_name(class: &str) -> Option<&str> {
    let marker = "Livewire\\";
    let pos = class.find(marker)?;
    Some(&class[pos + marker.len()..])
}
