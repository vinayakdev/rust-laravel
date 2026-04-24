use crate::bundle::DocBundle;
use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct ConfigHoverInput {
    pub key: String,
    pub current_value: Option<String>,
    pub env_key: Option<String>,
    pub default_value: Option<String>,
    pub env_value: Option<String>,
    pub detail: Option<String>,
}

pub fn build(input: ConfigHoverInput) -> DocBundle {
    let mut doc = MarkdownDoc::new().title(&input.key).blank().separator().blank();

    if let Some(current_value) = input.current_value.as_deref() {
        doc = doc.field("Current value", current_value).blank();
    }
    if let Some(env_key) = input.env_key.as_deref() {
        doc = doc.field("Env key", env_key).blank();
    }
    if let Some(default_value) = input.default_value.as_deref() {
        doc = doc.field("Default", default_value).blank();
    }
    if let Some(env_value) = input.env_value.as_deref() {
        doc = doc.field("Resolved env", env_value);
    }

    DocBundle::new(input.key, doc).with_detail(input.detail.unwrap_or_default())
}
