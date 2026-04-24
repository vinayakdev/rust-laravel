use crate::doc::MarkdownDoc;

#[derive(Clone, Debug)]
pub struct DocBundle {
    pub label: String,
    pub detail: Option<String>,
    pub hover: MarkdownDoc,
    pub completion: MarkdownDoc,
}

impl DocBundle {
    pub fn new(label: impl Into<String>, hover: MarkdownDoc) -> Self {
        let completion = hover.clone();
        Self {
            label: label.into(),
            detail: None,
            hover,
            completion,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_completion(mut self, completion: MarkdownDoc) -> Self {
        self.completion = completion;
        self
    }

    pub fn hover_markdown(&self) -> String {
        self.hover.finish_markdown()
    }

    pub fn completion_markdown(&self) -> String {
        self.completion.finish_markdown()
    }
}
