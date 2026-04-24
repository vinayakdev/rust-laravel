#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Config,
    Route,
    Env,
    View,
    Asset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewDataKind {
    CompactVariable,
}

#[derive(Clone, Debug)]
pub struct SymbolContext {
    pub kind: SymbolKind,
    pub full_text: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

#[derive(Clone, Debug)]
pub struct ViewDataContext {
    pub kind: ViewDataKind,
    pub full_text: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
    pub cursor_offset: usize,
}

#[derive(Clone, Debug)]
pub struct BladeVariableContext {
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

#[derive(Clone, Debug)]
pub struct BladeComponentTagContext {
    pub full_text: String,
    pub prefix: String,
    pub has_x_dash: bool,
    pub tag_start_character: usize,
    pub start_character: usize,
    pub end_character: usize,
}

#[derive(Clone, Debug)]
pub struct BladeComponentAttrContext {
    pub component: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
    pub already_present: Vec<String>,
    pub already_typed_colon: bool,
}

pub fn detect_blade_component_tag_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<BladeComponentTagContext> {
    if !uri.ends_with(".blade.php") {
        return None;
    }
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let before = &line_text[..cursor];

    let (tag_start, has_x_dash) = find_last_open_x_tag(before)?;
    let name_start = tag_start + if has_x_dash { 3 } else { 2 };
    let between = &before[name_start..];

    if between.chars().any(|c| c.is_whitespace()) {
        return None;
    }

    let mut name_end = cursor;
    while name_end < line_text.len() {
        match line_text[name_end..].chars().next() {
            Some(c) if is_component_name_char(c) => name_end += c.len_utf8(),
            _ => break,
        }
    }

    // For completion replacement: when no dash yet, replace from `x` so newText can be `x-name`
    let replace_start = if has_x_dash { name_start } else { tag_start + 1 };

    Some(BladeComponentTagContext {
        full_text: line_text[name_start..name_end].to_string(),
        prefix: between.to_string(),
        has_x_dash,
        tag_start_character: line_text[..tag_start].chars().count(),
        start_character: line_text[..replace_start].chars().count(),
        end_character: line_text[..name_end].chars().count(),
    })
}

pub fn detect_blade_component_attr_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<BladeComponentAttrContext> {
    if !uri.ends_with(".blade.php") {
        return None;
    }
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let before = &line_text[..cursor];

    let tag_start = find_last_open_x_tag_start(before)?;
    let name_start = tag_start + 3;

    let component_name_len = before[name_start..].find(|c: char| !is_component_name_char(c))?;
    let name_end = name_start + component_name_len;
    let component = before[name_start..name_end].to_string();
    if component.is_empty() {
        return None;
    }

    if !before[name_end..].starts_with(|c: char| c.is_whitespace()) {
        return None;
    }

    // Find current token start first so attr_area excludes the token being typed
    let mut tok_start = cursor;
    while tok_start > 0 {
        let prev = line_text[..tok_start].chars().next_back()?;
        if prev.is_whitespace() || prev == '<' || prev == '>' || prev == '=' || prev == '"' || prev == '\'' {
            break;
        }
        tok_start -= prev.len_utf8();
    }

    let attr_area = &before[name_end..tok_start];
    let already_present = parse_existing_attributes(attr_area);

    let already_typed_colon =
        tok_start < line_text.len() && line_text[tok_start..].starts_with(':');
    let name_token_start = if already_typed_colon { tok_start + 1 } else { tok_start };

    let mut tok_end = cursor;
    while tok_end < line_text.len() {
        match line_text[tok_end..].chars().next() {
            Some(c) if is_identifier_char(c) || c == '-' => tok_end += c.len_utf8(),
            _ => break,
        }
    }

    Some(BladeComponentAttrContext {
        component,
        prefix: line_text[name_token_start..cursor.min(tok_end)].to_string(),
        start_character: line_text[..tok_start].chars().count(),
        end_character: line_text[..tok_end].chars().count(),
        already_present,
        already_typed_colon,
    })
}

fn find_last_open_x_tag(before: &str) -> Option<(usize, bool)> {
    let mut last: Option<(usize, bool)> = None;
    let mut i = 0;
    while i < before.len() {
        if before[i..].starts_with("<x-") {
            last = Some((i, true));
            i += 3;
        } else if before[i..].starts_with("<x") {
            let next = before[i + 2..].chars().next();
            let ok = next.map(|c| !c.is_ascii_alphanumeric() && c != '_').unwrap_or(true);
            if ok {
                last = Some((i, false));
            }
            i += 2;
        } else {
            i += before[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }
    }
    let (start, has_dash) = last?;
    if before[start..].contains('>') {
        return None;
    }
    Some((start, has_dash))
}

fn find_last_open_x_tag_start(before: &str) -> Option<usize> {
    find_last_open_x_tag(before).map(|(start, _)| start)
}

fn is_component_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '.'
}

fn parse_existing_attributes(attr_area: &str) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut i = 0;
    let bytes = attr_area.as_bytes();

    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' || bytes[i] == b'/' {
            break;
        }
        if bytes[i] == b':' {
            i += 1;
        }
        let name_start = i;
        while i < bytes.len() {
            match attr_area[i..].chars().next() {
                Some(c) if c.is_ascii_alphanumeric() || c == '-' || c == '_' => {
                    i += c.len_utf8();
                }
                _ => break,
            }
        }
        let name_end = i;
        if name_end > name_start {
            let attr_name = &attr_area[name_start..name_end];
            if !attr_name.contains(|c: char| c == '/' || c == '>') {
                attrs.push(attr_name.to_string());
            }
        } else if i < bytes.len() {
            i += 1;
            continue;
        }
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            if i < bytes.len() {
                let quote = bytes[i];
                if quote == b'"' || quote == b'\'' {
                    i += 1;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                }
            }
        }
    }

    attrs
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

pub fn detect_view_data_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<ViewDataContext> {
    if !uri.ends_with(".php") {
        return None;
    }

    let cursor = source_position_to_byte_index(source, line, character)?;
    let (quote_start, quote_char) = find_quote_start(source, cursor)?;
    let quote_end = find_quote_end(source, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    let before = &source[..quote_start];
    let compact: String = before.chars().filter(|ch| !ch.is_whitespace()).collect();
    if !compact.ends_with("compact(") {
        return None;
    }

    Some(ViewDataContext {
        kind: ViewDataKind::CompactVariable,
        full_text: source[inner_start..inner_end].to_string(),
        prefix: source[inner_start..cursor.min(inner_end)].to_string(),
        start_character: byte_index_to_character_in_line(source, inner_start),
        end_character: byte_index_to_character_in_line(source, inner_end),
        cursor_offset: cursor,
    })
}

pub fn detect_blade_variable_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<BladeVariableContext> {
    if !uri.ends_with(".blade.php") {
        return None;
    }

    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;

    if !is_inside_blade_echo(line_text, cursor) && !is_inside_blade_php(source, line, cursor) {
        return None;
    }

    let (name_start, name_end) = find_dollar_variable_name_bounds(line_text, cursor)?;
    Some(BladeVariableContext {
        prefix: line_text[name_start..cursor.min(name_end)].to_string(),
        start_character: line_text[..name_start].chars().count(),
        end_character: line_text[..name_end].chars().count(),
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

    if ["view(", "View::make(", "view()->make("]
        .iter()
        .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::View);
    }

    if compact.contains("Route::view(") && route_argument_index(before) >= 1 {
        return Some(SymbolKind::View);
    }

    if ["asset(", "secure_asset("]
        .iter()
        .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::Asset);
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

    let cursor = source_position_to_byte_index(source, line, character)?;

    detect_route_action_string_context(source, cursor)
        .or_else(|| detect_route_controller_identifier_context(source, cursor))
}

fn detect_route_action_string_context(source: &str, cursor: usize) -> Option<RouteActionContext> {
    let (quote_start, quote_char) = find_quote_start(source, cursor)?;
    let quote_end = find_quote_end(source, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    let before = &source[..quote_start];
    let full_text = source[inner_start..inner_end].to_string();
    let start_character = byte_index_to_character_in_line(source, inner_start);
    let end_character = byte_index_to_character_in_line(source, inner_end);

    if let Some(controller) = array_controller_before(before) {
        return Some(RouteActionContext {
            kind: RouteActionKind::ControllerMethodArray,
            controller: Some(controller),
            prefix: source[inner_start..cursor.min(inner_end)].to_string(),
            full_text,
            start_character,
            end_character,
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
                prefix: source[method_start..cursor.min(inner_end)].to_string(),
                full_text: full_text[method_start - inner_start..].to_string(),
                start_character: byte_index_to_character_in_line(source, method_start),
                end_character,
            });
        }

        return Some(RouteActionContext {
            kind: RouteActionKind::LegacyControllerString,
            controller: None,
            prefix: source[inner_start..cursor].to_string(),
            full_text: full_text[..at_index].to_string(),
            start_character,
            end_character: byte_index_to_character_in_line(source, inner_start + at_index),
        });
    }

    Some(RouteActionContext {
        kind: RouteActionKind::LegacyControllerString,
        controller: None,
        prefix: source[inner_start..cursor.min(inner_end)].to_string(),
        full_text,
        start_character,
        end_character,
    })
}

fn detect_route_controller_identifier_context(
    source: &str,
    cursor: usize,
) -> Option<RouteActionContext> {
    let (start, end) = find_controller_identifier_bounds(source, cursor)?;
    if !source[end..].starts_with("::class") {
        return None;
    }
    if !looks_like_controller_array_slot(source, start) {
        return None;
    }

    Some(RouteActionContext {
        kind: RouteActionKind::ControllerClass,
        controller: None,
        prefix: source[start..cursor.min(end)].to_string(),
        full_text: source[start..end].to_string(),
        start_character: byte_index_to_character_in_line(source, start),
        end_character: byte_index_to_character_in_line(source, end),
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
    if before[open_bracket + 1..].contains(']') {
        return None;
    }
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

fn source_position_to_byte_index(source: &str, line: usize, character: usize) -> Option<usize> {
    let line_start = source
        .split_inclusive('\n')
        .take(line)
        .map(str::len)
        .sum::<usize>();
    let line_text = source.lines().nth(line)?;
    let character_offset = character_to_byte_index(line_text, character)?;
    Some(line_start + character_offset)
}

fn byte_index_to_character_in_line(source: &str, index: usize) -> usize {
    source[..index]
        .rsplit('\n')
        .next()
        .unwrap_or("")
        .chars()
        .count()
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

fn find_dollar_variable_name_bounds(text: &str, cursor: usize) -> Option<(usize, usize)> {
    if cursor > text.len() {
        return None;
    }

    let mut start = cursor;
    while start > 0 {
        let ch = text[..start].chars().next_back()?;
        if !is_identifier_char(ch) {
            break;
        }
        start -= ch.len_utf8();
    }

    if start == 0 || text[..start].chars().next_back()? != '$' {
        return None;
    }

    let mut end = cursor;
    while end < text.len() {
        let ch = text[end..].chars().next()?;
        if !is_identifier_char(ch) {
            break;
        }
        end += ch.len_utf8();
    }

    Some((start, end))
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
    use super::{
        RouteActionKind, SymbolKind, ViewDataKind, detect_route_action_context,
        detect_symbol_context, detect_view_data_context,
    };

    #[test]
    fn detects_symbol_context_at_end_of_non_empty_string() {
        let source = "return config('a');";
        let character = source.find("');").expect("closing quote should exist");
        let context = detect_symbol_context(source, 0, character).expect("symbol context");

        assert_eq!(context.kind, SymbolKind::Config);
        assert_eq!(context.full_text, "a");
        assert_eq!(context.prefix, "a");
    }

    #[test]
    fn detects_symbol_context_at_end_of_dotted_string() {
        let source = "return route('resources.');";
        let character = source.find("');").expect("closing quote should exist");
        let context = detect_symbol_context(source, 0, character).expect("symbol context");

        assert_eq!(context.kind, SymbolKind::Route);
        assert_eq!(context.full_text, "resources.");
        assert_eq!(context.prefix, "resources.");
    }

    #[test]
    fn detects_array_controller_method_context() {
        let source = "Route::get('/', [WebsiteController::class, 'ho']);";
        let character = source.find("ho").unwrap() + 1;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        assert_eq!(context.kind, RouteActionKind::ControllerMethodArray);
        assert_eq!(context.controller.as_deref(), Some("WebsiteController"));
        assert_eq!(context.prefix, "h");
    }

    #[test]
    fn detects_array_controller_method_context_at_end_of_token_with_route_chain() {
        let source = "Route::get('/', [VirtualOfficeController::class, 'index'])->name('index');";
        let character = source.find("index']").unwrap() + "index".len();
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        assert_eq!(context.kind, RouteActionKind::ControllerMethodArray);
        assert_eq!(
            context.controller.as_deref(),
            Some("VirtualOfficeController")
        );
        assert_eq!(context.full_text, "index");
        assert_eq!(context.prefix, "index");
    }

    #[test]
    fn detects_empty_array_controller_method_context_with_route_chain() {
        let source = "Route::get('/', [VirtualOfficeController::class, ''])->name('index');";
        let character = source.find("''").unwrap() + 1;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("route action context");

        assert_eq!(context.kind, RouteActionKind::ControllerMethodArray);
        assert_eq!(
            context.controller.as_deref(),
            Some("VirtualOfficeController")
        );
        assert_eq!(context.full_text, "");
        assert_eq!(context.prefix, "");
    }

    #[test]
    fn detects_legacy_controller_method_context() {
        let source = "Route::get('/', 'WebsiteController@ho');";
        let character = source.find("ho").unwrap() + 1;
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("legacy route action context");

        assert_eq!(context.kind, RouteActionKind::LegacyMethodString);
        assert_eq!(context.controller.as_deref(), Some("WebsiteController"));
        assert_eq!(context.prefix, "h");
        assert_eq!(context.full_text, "ho");
    }

    #[test]
    fn detects_legacy_controller_method_context_at_end_of_token() {
        let source = "Route::get('/', 'WebsiteController@index');";
        let character = source.find("index").unwrap() + "index".len();
        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character)
                .expect("legacy route action context");

        assert_eq!(context.kind, RouteActionKind::LegacyMethodString);
        assert_eq!(context.controller.as_deref(), Some("WebsiteController"));
        assert_eq!(context.prefix, "index");
        assert_eq!(context.full_text, "index");
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

    #[test]
    fn does_not_treat_route_name_string_as_controller_method_context() {
        let source =
            "Route::get('/{city}', [VirtualOfficeController::class, 'center'])->name('center');";
        let character = source.rfind("center").unwrap() + 2;

        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character);

        assert!(context.is_none());
    }

    #[test]
    fn does_not_treat_closing_quote_as_controller_method_context() {
        let source = "Route::get('/', [VirtualOfficeController::class, 'center'])->name('center');";
        let character = source.find("'center']").unwrap();

        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, 0, character);

        assert!(context.is_none());
    }

    #[test]
    fn detects_multiline_array_controller_method_context() {
        let source = "Route::get('/{city}/{locality}/{centerSlug}', [\n    SitemapController::class,\n    'ssdssdsddsdme3dds',\n])->name('center');";
        let line = 2;
        let character = 7;

        let context =
            detect_route_action_context("file:///tmp/routes/web.php", source, line, character)
                .expect("multiline route action context");

        assert_eq!(context.kind, RouteActionKind::ControllerMethodArray);
        assert_eq!(context.controller.as_deref(), Some("SitemapController"));
        assert_eq!(context.full_text, "ssdssdsddsdme3dds");
        assert_eq!(context.prefix, "ss");
    }

    #[test]
    fn detects_compact_variable_context_inside_multiline_view_call() {
        let source =
            "<?php\n\nreturn view(\n    'demo',\n    compact(\n        'currentUs'\n    )\n);\n";
        let line = 5;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character =
            line_text.find("currentUs").expect("token should exist") + "currentUs".len();

        let context = detect_view_data_context(
            "file:///tmp/app/Http/Controllers/DemoController.php",
            source,
            line,
            character,
        )
        .expect("view data context");

        assert_eq!(context.kind, ViewDataKind::CompactVariable);
        assert_eq!(context.full_text, "currentUs");
        assert_eq!(context.prefix, "currentUs");
    }
}
