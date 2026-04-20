#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Config,
    Route,
}

#[derive(Clone, Debug)]
pub struct SymbolContext {
    pub kind: SymbolKind,
    pub full_text: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

pub fn detect_symbol_context(source: &str, line: usize, character: usize) -> Option<SymbolContext> {
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let (quote_start, quote_char) = find_quote_start(line_text, cursor)?;
    let quote_end = find_quote_end(line_text, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    let before = &line_text[..quote_start];
    let kind = detect_kind(before)?;
    let full_text = line_text[inner_start..inner_end].to_string();
    let prefix = line_text[inner_start..cursor.min(inner_end)].to_string();

    Some(SymbolContext {
        kind,
        full_text,
        prefix,
        start_character: line_text[..inner_start].chars().count(),
        end_character: line_text[..inner_end].chars().count(),
    })
}

fn detect_kind(before: &str) -> Option<SymbolKind> {
    let compact: String = before.chars().filter(|ch| !ch.is_whitespace()).collect();

    if [
        "config(",
        "Config::get(",
        "Config::has(",
        "Config::string(",
        "Config::integer(",
        "Config::boolean(",
        "Config::array(",
    ]
    .iter()
    .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::Config);
    }

    if ["route(", "to_route(", "redirect()->route(", "Route::has("]
        .iter()
        .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::Route);
    }

    None
}

fn character_to_byte_index(text: &str, character: usize) -> Option<usize> {
    if character == text.chars().count() {
        return Some(text.len());
    }

    text.char_indices().nth(character).map(|(index, _)| index)
}

fn find_quote_start(text: &str, cursor: usize) -> Option<(usize, char)> {
    let mut last = None;

    for (index, ch) in text.char_indices() {
        if index >= cursor {
            break;
        }
        if (ch == '\'' || ch == '"') && !is_escaped(text, index) {
            last = Some((index, ch));
        }
    }

    last
}

fn find_quote_end(text: &str, from: usize, quote: char) -> Option<usize> {
    for (index, ch) in text[from..].char_indices() {
        let absolute = from + index;
        if ch == quote && !is_escaped(text, absolute) {
            return Some(absolute);
        }
    }
    None
}

fn is_escaped(text: &str, index: usize) -> bool {
    let mut slash_count = 0usize;

    for ch in text[..index].chars().rev() {
        if ch == '\\' {
            slash_count += 1;
        } else {
            break;
        }
    }

    slash_count % 2 == 1
}
