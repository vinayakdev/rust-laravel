#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownFormat {
    Markdown,
    PlainText,
}

#[derive(Clone, Debug, Default)]
pub struct MarkdownDoc {
    lines: Vec<DocLine>,
}

#[derive(Clone, Debug)]
enum DocLine {
    Text(String),
    Blank,
    Separator,
    Title(String),
    Field {
        label: String,
        value: String,
    },
    LinkField {
        label: String,
        text: String,
        url: String,
    },
    CodeBlock {
        language: String,
        code: String,
    },
}

impl MarkdownDoc {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, text: impl Into<String>) -> Self {
        self.lines.push(DocLine::Title(text.into()));
        self
    }

    pub fn line(mut self, text: impl Into<String>) -> Self {
        self.lines.push(DocLine::Text(text.into()));
        self
    }

    pub fn blank(mut self) -> Self {
        self.lines.push(DocLine::Blank);
        self
    }

    pub fn separator(mut self) -> Self {
        self.lines.push(DocLine::Separator);
        self
    }

    pub fn field(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.lines.push(DocLine::Field {
            label: label.into(),
            value: value.into(),
        });
        self
    }

    pub fn code_block(mut self, language: impl Into<String>, code: impl Into<String>) -> Self {
        self.lines.push(DocLine::CodeBlock {
            language: language.into(),
            code: code.into(),
        });
        self
    }

    pub fn link_field(
        mut self,
        label: impl Into<String>,
        text: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        self.lines.push(DocLine::LinkField {
            label: label.into(),
            text: text.into(),
            url: url.into(),
        });
        self
    }

    pub fn finish_markdown(&self) -> String {
        self.render(MarkdownFormat::Markdown)
    }

    pub fn finish_plaintext(&self) -> String {
        self.render(MarkdownFormat::PlainText)
    }

    pub fn render(&self, format: MarkdownFormat) -> String {
        self.lines
            .iter()
            .map(|line| match (format, line) {
                (_, DocLine::Blank) => String::new(),
                (MarkdownFormat::Markdown, DocLine::Separator) => "---".to_string(),
                (MarkdownFormat::PlainText, DocLine::Separator) => "----".to_string(),
                (MarkdownFormat::Markdown, DocLine::Title(text)) => format!("## {text}"),
                (MarkdownFormat::PlainText, DocLine::Title(text)) => text.clone(),
                (MarkdownFormat::Markdown, DocLine::Text(text)) => text.clone(),
                (MarkdownFormat::PlainText, DocLine::Text(text)) => strip_markdown(text),
                (MarkdownFormat::Markdown, DocLine::Field { label, value }) => {
                    format!("{label}: {}", inline_code(value))
                }
                (MarkdownFormat::PlainText, DocLine::Field { label, value }) => {
                    format!("{label}: {value}")
                }
                (MarkdownFormat::Markdown, DocLine::LinkField { label, text, url }) => {
                    format!("{label}: [{text}]({url})")
                }
                (MarkdownFormat::PlainText, DocLine::LinkField { label, text, url }) => {
                    format!("{label}: {text} ({url})")
                }
                (MarkdownFormat::Markdown, DocLine::CodeBlock { language, code }) => {
                    format!("```{language}\n{code}\n```")
                }
                (MarkdownFormat::PlainText, DocLine::CodeBlock { code, .. }) => code.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn inline_code(value: &str) -> String {
    let mut longest_run = 0usize;
    let mut current_run = 0usize;

    for ch in value.chars() {
        if ch == '`' {
            current_run += 1;
            longest_run = longest_run.max(current_run);
        } else {
            current_run = 0;
        }
    }

    let fence = "`".repeat(longest_run + 1);
    if value.starts_with(' ') || value.ends_with(' ') {
        format!("{fence} {value} {fence}")
    } else {
        format!("{fence}{value}{fence}")
    }
}

fn strip_markdown(value: &str) -> String {
    value.replace(['`', '[', ']', '(', ')'], "")
}

#[cfg(test)]
mod tests {
    use super::{MarkdownDoc, MarkdownFormat};

    #[test]
    fn renders_chained_markdown_blocks() {
        let doc = MarkdownDoc::new()
            .title("logo.svg")
            .blank()
            .separator()
            .blank()
            .field("Path", "public/assets/logo.svg")
            .blank()
            .link_field(
                "File",
                "assets/logo.svg",
                "file:///tmp/public/assets/logo.svg",
            );

        assert_eq!(
            doc.render(MarkdownFormat::Markdown),
            "## logo.svg\n\n---\n\nPath: `public/assets/logo.svg`\n\nFile: [assets/logo.svg](file:///tmp/public/assets/logo.svg)"
        );
    }

    #[test]
    fn renders_plain_text_variant() {
        let doc = MarkdownDoc::new()
            .title("logo.svg")
            .blank()
            .separator()
            .blank()
            .field("Size", "1.50 KiB")
            .field("Status", "missing");

        assert_eq!(
            doc.render(MarkdownFormat::PlainText),
            "logo.svg\n\n----\n\nSize: 1.50 KiB\nStatus: missing"
        );
    }
}
