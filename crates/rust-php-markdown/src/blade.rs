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
    let title = input.component.clone();
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

    if let Some(view_name) = input.view_name.as_deref() {
        doc = doc.field("View", view_name).blank();
    }

    if let Some(file) = input.view_file.as_deref() {
        if let Some(uri) = input.view_file_uri.as_deref() {
            doc = doc.link_field("Blade file", file, uri).blank();
        } else {
            doc = doc.field("Blade file", file).blank();
        }
    }

    if !input.properties.is_empty() {
        doc = doc.field("Properties", input.properties.join(", ")).blank();
    }

    if !input.actions.is_empty() {
        doc = doc.field("Actions", input.actions.join(", "));
    }

    DocBundle::new(title, doc).with_detail(input.detail.unwrap_or_default())
}
