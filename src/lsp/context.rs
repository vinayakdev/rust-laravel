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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteActionKind {
    ControllerClass,
    ControllerMethodArray,
    LegacyControllerString,
    LegacyMethodString,
}

#[derive(Clone, Debug)]
pub struct RouteActionContext {
    pub kind: RouteActionKind,
    pub controller: Option<String>,
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

pub fn detect_route_action_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<RouteActionContext> {
    if !uri.ends_with(".php") || !uri.contains("/routes/") {
        return None;
    }

    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;

    detect_route_action_string_context(line_text, cursor)
        .or_else(|| detect_route_controller_identifier_context(line_text, cursor))
}

fn detect_route_action_string_context(
    line_text: &str,
    cursor: usize,
) -> Option<RouteActionContext> {
    let (quote_start, quote_char) = find_quote_start(line_text, cursor)?;
    let quote_end = find_quote_end(line_text, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    let before = &line_text[..quote_start];
    let full_text = line_text[inner_start..inner_end].to_string();

    if let Some(controller) = array_controller_before(before) {
        return Some(RouteActionContext {
            kind: RouteActionKind::ControllerMethodArray,
            controller: Some(controller),
            prefix: line_text[inner_start..cursor.min(inner_end)].to_string(),
            full_text,
            start_character: line_text[..inner_start].chars().count(),
            end_character: line_text[..inner_end].chars().count(),
        });
    }

    if !looks_like_route_second_argument(before) {
        return None;
    }

    if let Some(at_index) = full_text.find('@') {
        let method_start = inner_start + at_index + 1;
        if cursor >= method_start {
            return Some(RouteActionContext {
                kind: RouteActionKind::LegacyMethodString,
                controller: Some(full_text[..at_index].to_string()),
                prefix: line_text[method_start..cursor.min(inner_end)].to_string(),
                full_text: full_text[method_start - inner_start..].to_string(),
                start_character: line_text[..method_start].chars().count(),
                end_character: line_text[..inner_end].chars().count(),
            });
        }

        return Some(RouteActionContext {
            kind: RouteActionKind::LegacyControllerString,
            controller: None,
            prefix: line_text[inner_start..cursor].to_string(),
            full_text: full_text[..at_index].to_string(),
            start_character: line_text[..inner_start].chars().count(),
            end_character: line_text[..inner_start + at_index].chars().count(),
        });
    }

    Some(RouteActionContext {
        kind: RouteActionKind::LegacyControllerString,
        controller: None,
        prefix: line_text[inner_start..cursor.min(inner_end)].to_string(),
        full_text,
        start_character: line_text[..inner_start].chars().count(),
        end_character: line_text[..inner_end].chars().count(),
    })
}

fn detect_route_controller_identifier_context(
    line_text: &str,
    cursor: usize,
) -> Option<RouteActionContext> {
    let (start, end) = find_controller_identifier_bounds(line_text, cursor)?;
    if !line_text[end..].starts_with("::class") {
        return None;
    }
    if !looks_like_controller_array_slot(line_text, start) {
        return None;
    }

    Some(RouteActionContext {
        kind: RouteActionKind::ControllerClass,
        controller: None,
        prefix: line_text[start..cursor.min(end)].to_string(),
        full_text: line_text[start..end].to_string(),
        start_character: line_text[..start].chars().count(),
        end_character: line_text[..end].chars().count(),
    })
}

fn find_controller_identifier_bounds(text: &str, cursor: usize) -> Option<(usize, usize)> {
    let mut start = cursor;
    while start > 0 {
        let (index, ch) = text[..start].char_indices().next_back()?;
        if !is_controller_identifier_char(ch) {
            break;
        }
        start = index;
    }

    let mut end = cursor;
    while end < text.len() {
        let ch = text[end..].chars().next()?;
        if !is_controller_identifier_char(ch) {
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

fn is_controller_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '\\')
}

fn looks_like_controller_array_slot(line_text: &str, start: usize) -> bool {
    let before = &line_text[..start];
    if !before.contains("Route::") {
        return false;
    }
    let Some(open_bracket) = before.rfind('[') else {
        return false;
    };
    !before[open_bracket + 1..].contains(']')
}

fn array_controller_before(before: &str) -> Option<String> {
    let open_bracket = before.rfind('[')?;
    let segment = &before[open_bracket + 1..];
    let class_index = segment.rfind("::class")?;
    let candidate = segment[..class_index].split(',').next()?.trim();

    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
    }
}

fn looks_like_route_second_argument(before: &str) -> bool {
    if !before.contains("Route::") {
        return false;
    }
    route_argument_index(before) >= 1
}

fn route_argument_index(before: &str) -> usize {
    let Some(open_paren) = before.rfind('(') else {
        return 0;
    };
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut commas = 0usize;

    for ch in before[open_paren + 1..].chars() {
        match ch {
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '[' => depth_bracket += 1,
            ']' => depth_bracket = depth_bracket.saturating_sub(1),
            ',' if depth_paren == 0 && depth_bracket == 0 => commas += 1,
            _ => {}
        }
    }

    commas
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
    let last_open = before
        .rmatch_indices("{{")
        .map(|(index, _)| index)
        .next()
        .max(before.rmatch_indices("{!!").map(|(index, _)| index).next());
    let last_close = before
        .rmatch_indices("}}")
        .map(|(index, _)| index)
        .next()
        .max(before.rmatch_indices("!!}").map(|(index, _)| index).next());

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
        .max(
            before
                .rmatch_indices("<?php")
                .map(|(index, _)| index)
                .next(),
        );
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
    use super::{RouteActionKind, detect_route_action_context};

    #[test]
    fn detects_array_controller_method_context() {
        let source = "Route::get('/', [WebsiteController::class, 'ho']);";
        let character = source.find("ho").unwrap() + 2;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        assert_eq!(context.kind, RouteActionKind::ControllerMethodArray);
        assert_eq!(context.controller.as_deref(), Some("WebsiteController"));
        assert_eq!(context.prefix, "ho");
    }

    #[test]
    fn detects_legacy_controller_method_context() {
        let source = "Route::get('/', 'WebsiteController@ho');";
        let character = source.find("ho").unwrap() + 2;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("legacy route action context");

        assert_eq!(context.kind, RouteActionKind::LegacyMethodString);
        assert_eq!(context.controller.as_deref(), Some("WebsiteController"));
        assert_eq!(context.prefix, "ho");
        assert_eq!(context.full_text, "ho");
    }

    #[test]
    fn detects_controller_class_slot_context() {
        let source = "Route::get('/', [Websit::class, 'home']);";
        let character = source.find("Websit").unwrap() + 6;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("controller class context");

        assert_eq!(context.kind, RouteActionKind::ControllerClass);
        assert_eq!(context.prefix, "Websit");
    }
}
