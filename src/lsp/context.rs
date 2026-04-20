#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Config,
    Route,
    Env,
}

#[derive(Clone, Debug)]
pub struct SymbolContext {
    pub kind: SymbolKind,
    pub full_text: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperStyle {
    Php,
    BladeEcho,
}

#[derive(Clone, Debug)]
pub struct HelperContext {
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
    pub style: HelperStyle,
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

    if ["env("].iter().any(|needle| compact.ends_with(needle)) {
        return Some(SymbolKind::Env);
    }

    None
}

pub fn detect_helper_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<HelperContext> {
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let (start, end) = find_identifier_bounds(line_text, cursor)?;

    if cursor < start || cursor > end || line_text[cursor..].starts_with('(') {
        return None;
    }

    if start > 0 {
        let previous = line_text[..start].chars().next_back()?;
        if previous.is_ascii_alphanumeric() || matches!(previous, '_' | '$' | '>' | ':') {
            return None;
        }
    }

    let prefix = line_text[start..cursor].to_string();
    if prefix.len() < 2 {
        return None;
    }

    let style = if uri.ends_with(".blade.php") {
        if is_inside_blade_echo(line_text, cursor) {
            HelperStyle::BladeEcho
        } else if is_inside_blade_php(source, line, cursor) {
            HelperStyle::Php
        } else {
            return None;
        }
    } else if uri.ends_with(".php") {
        HelperStyle::Php
    } else {
        return None;
    };

    Some(HelperContext {
        prefix,
        start_character: line_text[..start].chars().count(),
        end_character: line_text[..end].chars().count(),
        style,
    })
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

fn find_identifier_bounds(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let mut start = cursor;
    while start > 0 {
        let (index, ch) = text[..start].char_indices().next_back()?;
        if !is_identifier_char(ch) {
            break;
        }
        start = index;
    }

    let mut end = cursor;
    while end < text.len() {
        let ch = text[end..].chars().next()?;
        if !is_identifier_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }

    if start == end {
        None
    } else {
        Some((start, end))
    }
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_inside_blade_echo(line_text: &str, cursor: usize) -> bool {
    let before = &line_text[..cursor];
    let last_open = before.rmatch_indices("{{").map(|(index, _)| index).next().max(
        before
            .rmatch_indices("{!!")
            .map(|(index, _)| index)
            .next(),
    );
    let last_close = before.rmatch_indices("}}").map(|(index, _)| index).next().max(
        before
            .rmatch_indices("!!}")
            .map(|(index, _)| index)
            .next(),
    );

    match (last_open, last_close) {
        (Some(open), Some(close)) => open > close,
        (Some(_), None) => true,
        _ => false,
    }
}

fn is_inside_blade_php(source: &str, line: usize, cursor: usize) -> bool {
    let lines = source.lines().collect::<Vec<_>>();
    let before_current = lines
        .iter()
        .take(line)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    let current_prefix = lines
        .get(line)
        .map(|text| &text[..cursor.min(text.len())])
        .unwrap_or("");
    let before = if before_current.is_empty() {
        current_prefix.to_string()
    } else {
        format!("{before_current}\n{current_prefix}")
    };

    let last_open = before
        .rmatch_indices("@php")
        .map(|(index, _)| index)
        .next()
        .max(before.rmatch_indices("<?php").map(|(index, _)| index).next());
    let last_close = before
        .rmatch_indices("@endphp")
        .map(|(index, _)| index)
        .next()
        .max(before.rmatch_indices("?>").map(|(index, _)| index).next());

    match (last_open, last_close) {
        (Some(open), Some(close)) => open > close,
        (Some(_), None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{HelperStyle, SymbolKind, detect_helper_context, detect_symbol_context};

    #[test]
    fn detects_env_symbol_context() {
        let source = "<?php env('APP_N');";
        let ctx = detect_symbol_context(source, 0, 16).expect("env context");
        assert_eq!(ctx.kind, SymbolKind::Env);
        assert_eq!(ctx.prefix, "APP_N");
    }

    #[test]
    fn detects_php_helper_context() {
        let source = "<?php rou";
        let ctx = detect_helper_context("file:///test.php", source, 0, 9).expect("php helper");
        assert_eq!(ctx.prefix, "rou");
        assert_eq!(ctx.style, HelperStyle::Php);
    }

    #[test]
    fn detects_blade_helper_context_inside_echo() {
        let source = "{{ rou }}";
        let ctx =
            detect_helper_context("file:///test.blade.php", source, 0, 6).expect("blade helper");
        assert_eq!(ctx.prefix, "rou");
        assert_eq!(ctx.style, HelperStyle::BladeEcho);
    }

    #[test]
    fn ignores_blade_text_outside_echo() {
        let source = "<div>rou</div>";
        assert!(detect_helper_context("file:///test.blade.php", source, 0, 8).is_none());
    }

    #[test]
    fn detects_blade_php_block_as_php_context() {
        let source = "<div>\n    @php\n        rou\n    @endphp\n</div>";
        let ctx =
            detect_helper_context("file:///test.blade.php", source, 2, 11).expect("blade php");
        assert_eq!(ctx.prefix, "rou");
        assert_eq!(ctx.style, HelperStyle::Php);
    }
}
