#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Config,
    Route,
    Env,
    View,
    Livewire,
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

#[derive(Clone, Debug)]
pub struct LivewireComponentTagContext {
    pub full_text: String,
    pub prefix: String,
    pub tag_start_character: usize,
    pub start_character: usize,
    pub end_character: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LivewireDirectiveValueKind {
    Property,
    Action,
}

#[derive(Clone, Debug)]
pub struct LivewireDirectiveValueContext {
    pub kind: LivewireDirectiveValueKind,
    pub directive: String,
    pub full_text: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
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
    let replace_start = if has_x_dash {
        name_start
    } else {
        tag_start + 1
    };

    Some(BladeComponentTagContext {
        full_text: line_text[name_start..name_end].to_string(),
        prefix: between.to_string(),
        has_x_dash,
        tag_start_character: line_text[..tag_start].chars().count(),
        start_character: line_text[..replace_start].chars().count(),
        end_character: line_text[..name_end].chars().count(),
    })
}

pub fn detect_livewire_component_tag_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<LivewireComponentTagContext> {
    if !uri.ends_with(".blade.php") {
        return None;
    }
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let before = &line_text[..cursor];

    let tag_start = find_last_open_livewire_tag_start(before)?;
    let name_start = tag_start + "<livewire:".len();
    let between = &before[name_start..];

    if between.chars().any(|c| c.is_whitespace()) {
        return None;
    }

    let mut name_end = cursor;
    while name_end < line_text.len() {
        match line_text[name_end..].chars().next() {
            Some(c) if is_livewire_component_name_char(c) => name_end += c.len_utf8(),
            _ => break,
        }
    }

    Some(LivewireComponentTagContext {
        full_text: line_text[name_start..name_end].to_string(),
        prefix: between.to_string(),
        tag_start_character: line_text[..tag_start].chars().count(),
        start_character: line_text[..name_start].chars().count(),
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
        if prev.is_whitespace()
            || prev == '<'
            || prev == '>'
            || prev == '='
            || prev == '"'
            || prev == '\''
        {
            break;
        }
        tok_start -= prev.len_utf8();
    }

    // If the walk stopped at a quote, the cursor is inside an attribute value, not a name
    if tok_start > 0 {
        let stop_char = line_text[..tok_start].chars().next_back();
        if matches!(stop_char, Some('"') | Some('\'')) {
            return None;
        }
    }

    let attr_area = &before[name_end..tok_start];
    let already_present = parse_existing_attributes(attr_area);

    let already_typed_colon =
        tok_start < line_text.len() && line_text[tok_start..].starts_with(':');
    let name_token_start = if already_typed_colon {
        tok_start + 1
    } else {
        tok_start
    };

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
            let ok = next
                .map(|c| !c.is_ascii_alphanumeric() && c != '_')
                .unwrap_or(true);
            if ok {
                last = Some((i, false));
            }
            i += 2;
        } else {
            i += before[i..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
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

fn find_last_open_livewire_tag_start(before: &str) -> Option<usize> {
    let mut last = None;
    let mut i = 0;
    while i < before.len() {
        if before[i..].starts_with("<livewire:") {
            last = Some(i);
            i += "<livewire:".len();
        } else {
            i += before[i..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
        }
    }
    let start = last?;
    if before[start..].contains('>') {
        return None;
    }
    Some(start)
}

fn is_component_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '.'
}

fn is_livewire_component_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | ':')
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

    if !is_inside_blade_echo(line_text, cursor)
        && !is_inside_blade_php(source, line, cursor)
        && !is_inside_blade_directive_parens(line_text, cursor)
    {
        return None;
    }

    let (name_start, name_end) = find_dollar_variable_name_bounds(line_text, cursor)?;
    Some(BladeVariableContext {
        prefix: line_text[name_start..cursor.min(name_end)].to_string(),
        start_character: line_text[..name_start].chars().count(),
        end_character: line_text[..name_end].chars().count(),
    })
}

#[derive(Clone, Debug)]
pub struct BladeModelPropertyContext {
    pub variable_name: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

pub fn detect_blade_model_property_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<BladeModelPropertyContext> {
    if !uri.ends_with(".blade.php") {
        return None;
    }

    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;

    if !is_inside_blade_echo(line_text, cursor)
        && !is_inside_blade_php(source, line, cursor)
        && !is_inside_blade_directive_parens(line_text, cursor)
    {
        return None;
    }

    let arrow_byte = find_last_arrow_before(line_text, cursor)?;
    let before_arrow = &line_text[..arrow_byte];

    let dollar_pos = before_arrow.rfind('$')?;
    let var_name: String = before_arrow[dollar_pos + 1..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    if var_name.is_empty() {
        return None;
    }

    // If var_name is a @foreach iteration variable, resolve it to the collection
    // variable so the index can look up its model class.
    let resolved_var = find_foreach_collection_var(source, line, &var_name)
        .unwrap_or(var_name);

    let after_arrow = arrow_byte + 2;
    let mut prop_end = cursor;
    while prop_end < line_text.len() {
        let ch = line_text[prop_end..].chars().next()?;
        if !is_identifier_char(ch) {
            break;
        }
        prop_end += ch.len_utf8();
    }

    Some(BladeModelPropertyContext {
        variable_name: resolved_var,
        prefix: line_text[after_arrow..cursor.min(prop_end)].to_string(),
        start_character: line_text[..after_arrow].chars().count(),
        end_character: line_text[..prop_end].chars().count(),
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

    if [
        "->layout(",
        "->extends(",
        "View::exists(",
        "view()->exists(",
    ]
    .iter()
    .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::View);
    }

    if compact.contains("Route::view(") && route_argument_index(before) >= 1 {
        return Some(SymbolKind::View);
    }

    if ["@livewire(", "Livewire::component("]
        .iter()
        .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::Livewire);
    }

    if compact.contains("Route::livewire(") && route_argument_index(before) >= 1 {
        return Some(SymbolKind::Livewire);
    }

    if ["asset(", "secure_asset("]
        .iter()
        .any(|needle| compact.ends_with(needle))
    {
        return Some(SymbolKind::Asset);
    }

    None
}

pub fn detect_livewire_directive_value_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<LivewireDirectiveValueContext> {
    if !uri.ends_with(".blade.php") {
        return None;
    }

    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let (quote_start, quote_char) = find_quote_start(line_text, cursor)?;
    let quote_end = find_quote_end(line_text, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    let before_quote = &line_text[..quote_start];
    let directive = directive_before_quote(before_quote)?;
    let kind = livewire_directive_value_kind(&directive)?;

    Some(LivewireDirectiveValueContext {
        kind,
        directive,
        full_text: line_text[inner_start..inner_end].to_string(),
        prefix: line_text[inner_start..cursor.min(inner_end)].to_string(),
        start_character: line_text[..inner_start].chars().count(),
        end_character: line_text[..inner_end].chars().count(),
    })
}

fn directive_before_quote(before_quote: &str) -> Option<String> {
    let eq_index = before_quote.rfind('=')?;
    if !before_quote[eq_index + 1..].trim().is_empty() {
        return None;
    }

    let before_eq = before_quote[..eq_index].trim_end();
    let start = before_eq
        .rfind(|ch: char| ch.is_whitespace() || ch == '<')
        .map(|index| index + 1)
        .unwrap_or(0);
    let directive = before_eq[start..].trim();
    directive
        .starts_with("wire:")
        .then(|| directive.to_string())
}

fn livewire_directive_value_kind(directive: &str) -> Option<LivewireDirectiveValueKind> {
    let name = directive
        .strip_prefix("wire:")?
        .split('.')
        .next()
        .unwrap_or_default();
    match name {
        "model" | "target" => Some(LivewireDirectiveValueKind::Property),
        "click" | "submit" | "init" | "poll" | "keydown" | "keyup" | "change" | "blur"
        | "input" | "mouseenter" | "mouseleave" => Some(LivewireDirectiveValueKind::Action),
        _ => None,
    }
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

fn is_inside_blade_directive_parens(line_text: &str, cursor: usize) -> bool {
    let before = &line_text[..cursor];
    // Walk backward to find the last `@word(` whose parens are still open at cursor.
    let bytes = before.as_bytes();
    let mut i = bytes.len().saturating_sub(1);
    loop {
        // Find the previous `@`
        let at = match before[..=i].rfind('@') {
            Some(pos) => pos,
            None => return false,
        };
        let after_at = &before[at + 1..];
        let name_len = after_at
            .find(|c: char| !c.is_alphabetic())
            .unwrap_or(after_at.len());
        // Must have at least one letter followed immediately by `(`
        if name_len > 0 {
            let paren_pos = at + 1 + name_len;
            if before.as_bytes().get(paren_pos) == Some(&b'(') {
                // Count depth from the opening `(` to cursor
                let mut depth = 0i32;
                for ch in before[paren_pos..].chars() {
                    match ch {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        _ => {}
                    }
                }
                if depth > 0 {
                    return true;
                }
            }
        }
        if at == 0 {
            return false;
        }
        i = at - 1;
    }
}

/// Scan backward from `current_line` through `source` to find an enclosing
/// `@foreach($collection as $item)` that introduces `item_var`. Returns the
/// collection variable name (without `$`) when found.
fn find_foreach_collection_var(source: &str, current_line: usize, item_var: &str) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let mut open_endforeach: usize = 0;
    let item_dollar = format!("${item_var}");

    for ln in (0..current_line).rev() {
        let text = lines.get(ln)?.trim();
        if text.starts_with("@endforeach") {
            open_endforeach += 1;
            continue;
        }
        if let Some(rest) = text.strip_prefix("@foreach") {
            if open_endforeach > 0 {
                open_endforeach -= 1;
                continue;
            }
            // Parse `@foreach($collection as $item)` — find ` as $item_var`
            let needle = format!(" as {item_dollar}");
            let needle_close = format!(" as {item_dollar})");
            if rest.contains(&needle_close) || rest.contains(&needle) {
                // Extract collection var: first `$word` inside the parens
                let inner = rest
                    .find('(')
                    .and_then(|s| rest[s + 1..].find(')').map(|e| &rest[s + 1..s + 1 + e]))?;
                let col_start = inner.find('$')?;
                let col: String = inner[col_start + 1..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !col.is_empty() {
                    return Some(col);
                }
            }
        }
    }
    None
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

/// Context for completing vendor class chainable methods after `->`.
/// Detected when cursor is positioned in a method name slot in a fluent chain.
#[derive(Clone, Debug)]
pub struct VendorChainContext {
    /// Resolved FQN of the chain-originating class (e.g. `Filament\Forms\Components\TextInput`).
    pub class_fqn: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

/// Context for completing model column names inside `ClassName::make('|')`.
#[derive(Clone, Debug)]
pub struct VendorMakeContext {
    /// Short class name extracted from before `::make(` (e.g. `TextInput`).
    pub class_short: String,
    /// Resolved FQN if the use map contained the short name.
    pub class_fqn: Option<String>,
    /// Model class name found directly in the current source file (`$model = X::class`).
    pub model_class: Option<String>,
    /// Path to the current file — used by the query layer to scan sibling Resource files
    /// when `model_class` is None (i.e. the form lives in a separate Schema class).
    pub current_file: Option<std::path::PathBuf>,
    /// Short name of the class declared in the current file (e.g. `WhatsAppLinkForm`).
    pub current_class_name: Option<String>,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

/// Detect a vendor method chain context: cursor is typing a method name after `->`.
///
/// Looks for `->identifier` at the cursor position (no `(` between `->` and cursor),
/// then scans backwards through the current statement to find the originating `ClassName::`.
pub fn detect_vendor_chain_context(
    source: &str,
    line: usize,
    character: usize,
) -> Option<VendorChainContext> {
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;

    let arrow_byte = find_last_arrow_before(line_text, cursor)?;
    let after_arrow = &line_text[arrow_byte + 2..cursor];

    // We must be typing a method name: no `(` between `->` and cursor
    if after_arrow.contains('(') || after_arrow.contains(')') {
        return None;
    }
    if !after_arrow.chars().all(|c| is_identifier_char(c)) {
        return None;
    }

    let prefix = after_arrow.to_string();

    // Extend to end of current identifier token
    let mut end = cursor;
    while end < line_text.len() {
        match line_text[end..].chars().next() {
            Some(c) if is_identifier_char(c) => end += c.len_utf8(),
            _ => break,
        }
    }

    let lines: Vec<&str> = source.lines().collect();
    let class_short = find_chain_origin_class(&lines, line, line_text, arrow_byte)?;
    let use_map = extract_use_map(source);
    let class_fqn = use_map
        .get(&class_short)
        .cloned()
        .unwrap_or(class_short);

    Some(VendorChainContext {
        class_fqn,
        prefix,
        start_character: line_text[..arrow_byte + 2].chars().count(),
        end_character: line_text[..end].chars().count(),
    })
}

/// Detect the context for `ClassName::make('|')` — column name completion.
///
/// Fires when the cursor is inside the first string argument of a `::make()` call.
/// Accepts `uri` so that the query layer can locate a sibling Resource file when the
/// form is defined in a separate Schema class (the common Filament v3 pattern).
pub fn detect_vendor_make_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Option<VendorMakeContext> {
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let (quote_start, quote_char) = find_quote_start(line_text, cursor)?;
    let quote_end =
        find_quote_end(line_text, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    let before_quote = &line_text[..quote_start];
    let compact_before: String = before_quote.chars().filter(|c| !c.is_whitespace()).collect();
    if !compact_before.ends_with("::make(") {
        return None;
    }

    // Extract class short name: chars before `::make(`
    let marker = "::make(";
    let make_pos = compact_before.rfind(marker)?;
    let class_short_compact = &compact_before[..make_pos];
    let class_short = class_short_compact
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '\\')
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();

    if class_short.is_empty() {
        return None;
    }

    let use_map = extract_use_map(source);
    let class_fqn = use_map.get(&class_short).cloned();
    let model_class = extract_model_class_from_resource(source);
    let current_file = file_uri_to_path(uri);
    let current_class_name = extract_class_name_from_source(source);
    let prefix = source_chars_slice(line_text, inner_start, cursor.min(inner_end));

    Some(VendorMakeContext {
        class_short,
        class_fqn,
        model_class,
        current_file,
        current_class_name,
        prefix,
        start_character: line_text[..inner_start].chars().count(),
        end_character: line_text[..inner_end].chars().count(),
    })
}

fn file_uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    let path = uri.strip_prefix("file://")?;
    let decoded = percent_decode_simple(path);
    Some(std::path::PathBuf::from(decoded))
}

fn percent_decode_simple(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                out.push(v as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn extract_class_name_from_source(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        if t.starts_with("class ") || t.starts_with("abstract class ") || t.starts_with("final class ") {
            let after = t
                .trim_start_matches("final ")
                .trim_start_matches("abstract ")
                .trim_start_matches("class ");
            let name = after
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches('{');
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn find_last_arrow_before(line: &str, cursor: usize) -> Option<usize> {
    let bytes = line.as_bytes();
    let limit = cursor.min(bytes.len());
    let mut last = None;
    let mut i = 0;
    while i + 1 < limit {
        if bytes[i] == b'-' && bytes[i + 1] == b'>' {
            last = Some(i);
            i += 2;
        } else {
            i += 1;
        }
    }
    last
}

fn find_chain_origin_class(
    lines: &[&str],
    current_line: usize,
    line_text: &str,
    arrow_byte: usize,
) -> Option<String> {
    // Check current line before the arrow for ClassName::
    if let Some(name) = extract_class_from_segment(&line_text[..arrow_byte]) {
        return Some(name);
    }

    // Scan backwards up to 30 lines for the start of this chain
    let start = current_line.saturating_sub(30);
    for idx in (start..current_line).rev() {
        let prev = lines.get(idx)?;
        let trimmed = prev.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(name) = extract_class_from_segment(prev) {
            return Some(name);
        }
        // Stop at statement boundaries (semicolons, closing braces at start)
        if trimmed.ends_with(';') || trimmed == "}" || trimmed.starts_with("function ")
            || trimmed.starts_with("class ") || trimmed.starts_with("return ")
        {
            break;
        }
    }
    None
}

fn extract_class_from_segment(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    let mut last_class: Option<String> = None;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b':' && bytes[i + 1] == b':' {
            // Find the word immediately before `::`
            let mut start = i;
            while start > 0 {
                let slice = &text[..start];
                let Some(ch) = slice.chars().next_back() else { break };
                if !is_identifier_char(ch) { break; }
                start -= ch.len_utf8();
            }
            let word = &text[start..i];
            if !word.is_empty() && word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                last_class = Some(word.to_string());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    last_class
}

pub fn extract_use_map(source: &str) -> std::collections::HashMap<String, String> {
    rust_php_foundation::vendor::parse_file_use_statements(source)
}

/// Context for completing model column names inside builder/collection method string args.
/// Fires when cursor is inside the first string arg of a known Eloquent builder method
/// (e.g. `->pluck('|')`, `->orderBy('|')`, `->where('|', ...)`).
#[derive(Clone, Debug)]
pub struct BuilderArgContext {
    /// Inferred model class (short name resolved from typed param or PHPDoc `@var`).
    pub model_class: String,
    pub prefix: String,
    pub start_character: usize,
    pub end_character: usize,
}

/// Known Eloquent/Collection builder methods whose first string arg is a column name.
const BUILDER_COLUMN_METHODS: &[&str] = &[
    "pluck",
    "orderBy",
    "orderByDesc",
    "orderByAsc",
    "where",
    "whereIn",
    "whereNotIn",
    "whereBetween",
    "whereNull",
    "whereNotNull",
    "select",
    "value",
    "firstWhere",
    "find",
    "sum",
    "min",
    "max",
    "avg",
    "average",
    "groupBy",
    "having",
    "withAggregate",
    "withCount",
    "withSum",
    "withMin",
    "withMax",
    "withAvg",
    "latest",
    "oldest",
];

/// Detect a builder method string-arg context.
///
/// Fires when the cursor is inside the first string argument of a known Eloquent column method.
/// Infers the model from:
/// - typed function parameter (`BlogPost $record`) or PHPDoc `@var`/`@param`
/// - static model call origin (`BlogPost::query()->pluck(...)` or `BlogPost::all()->...`)
pub fn detect_builder_arg_context(
    source: &str,
    line: usize,
    character: usize,
) -> Option<BuilderArgContext> {
    let line_text = source.lines().nth(line)?;
    let cursor = character_to_byte_index(line_text, character)?;
    let (quote_start, quote_char) = find_quote_start(line_text, cursor)?;
    let quote_end = find_quote_end(line_text, quote_start + quote_char.len_utf8(), quote_char)?;
    let inner_start = quote_start + quote_char.len_utf8();
    let inner_end = quote_end;

    if cursor < inner_start || cursor > inner_end {
        return None;
    }

    // Check that immediately before the opening quote is `->methodName(`
    let before_quote = &line_text[..quote_start];
    let compact_before: String = before_quote.chars().filter(|c| !c.is_whitespace()).collect();
    let method_name = BUILDER_COLUMN_METHODS
        .iter()
        .find(|&&m| compact_before.ends_with(&format!("->{m}(")))?;

    // Find the chain segment before `->method(`
    let arrow_needle = format!("->{method_name}(");
    let arrow_pos = before_quote.rfind(&arrow_needle)?;
    let before_arrow = &before_quote[..arrow_pos];

    let prefix = source_chars_slice(line_text, inner_start, cursor.min(inner_end));

    // Strategy 1: extract `$variable` and resolve its type from typed params/PHPDoc
    let model_class = if let Some(var_name) = extract_root_variable(before_arrow) {
        resolve_variable_type(source, &var_name)
    } else {
        None
    };

    // Strategy 2: extract static origin class from `ClassName::` in the chain
    let model_class = model_class.or_else(|| {
        let lines: Vec<&str> = source.lines().collect();
        let class_short = find_chain_origin_class(&lines, line, line_text, arrow_pos)?;
        if is_model_like_class(&class_short) {
            Some(class_short)
        } else {
            None
        }
    })?;

    Some(BuilderArgContext {
        model_class,
        prefix,
        start_character: line_text[..inner_start].chars().count(),
        end_character: line_text[..inner_end].chars().count(),
    })
}

/// Extract the root variable name from a chain like `$record->tagsWithType('x')`.
/// Returns `record` (without `$`).
fn extract_root_variable(text: &str) -> Option<String> {
    // Walk right-to-left: skip method calls and whitespace to find the last `$varName`
    let trimmed = text.trim_end();

    // Find the last `$identifier` before a `->` or at the end of the segment
    let mut i = trimmed.len();
    // Skip any trailing `->chainedMethod()` parts
    loop {
        let s = &trimmed[..i];
        if let Some(arrow_pos) = s.rfind("->") {
            i = arrow_pos;
        } else {
            break;
        }
    }

    let segment = trimmed[..i].trim_end();
    let dollar_pos = segment.rfind('$')?;
    let after_dollar = &segment[dollar_pos + 1..];
    let var_name: String = after_dollar
        .chars()
        .take_while(|c| is_identifier_char(*c))
        .collect();

    if var_name.is_empty() {
        None
    } else {
        Some(var_name)
    }
}

/// Scan the source for typed parameter hints and PHPDoc `@var` annotations to resolve
/// a variable name to a model class name.
///
/// Handles:
/// - `function foo(BlogPost $record)` → `record` → `BlogPost`
/// - `/** @var BlogPost $record */` → `record` → `BlogPost`
/// - `@param BlogPost $record` → `record` → `BlogPost`
fn resolve_variable_type(source: &str, var_name: &str) -> Option<String> {
    let needle = format!("${var_name}");

    for line in source.lines() {
        let trimmed = line.trim();

        // PHPDoc @var or @param: `@var TypeName $varName` or `@param TypeName $varName`
        if trimmed.starts_with("* @var ") || trimmed.starts_with("* @param ") || trimmed.starts_with("@var ") || trimmed.starts_with("@param ") {
            let after_tag = trimmed
                .trim_start_matches('*')
                .trim()
                .trim_start_matches("@var")
                .trim_start_matches("@param")
                .trim();
            // `TypeName $varName ...`
            let mut parts = after_tag.split_whitespace();
            let type_name = parts.next()?;
            let var_part = parts.next().unwrap_or("");
            if var_part == needle {
                let class_name = type_name.trim_matches('\\').split('\\').last()?;
                if is_model_like_class(class_name) {
                    return Some(class_name.to_string());
                }
            }
            continue;
        }

        // Typed function/closure parameter: `TypeName $varName` optionally followed by `,`, `)`, `=`
        if !trimmed.contains(&needle) {
            continue;
        }
        if let Some(class_name) = extract_typed_param(trimmed, &needle) {
            return Some(class_name);
        }
    }
    None
}

/// From a line like `static fn(BlogPost $record): array`, extract the type of `$varName`.
fn extract_typed_param(line: &str, needle: &str) -> Option<String> {
    let pos = line.find(needle)?;
    // Walk backwards from needle to find identifier (type hint)
    let before = &line[..pos];
    let trimmed_before = before.trim_end();
    let type_name: String = trimmed_before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '\\')
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    if type_name.is_empty() {
        return None;
    }

    // The char immediately before the type hint must be a word boundary (space, `(`, `,`)
    let type_start = trimmed_before.len().saturating_sub(type_name.len());
    let boundary = if type_start == 0 {
        true
    } else {
        let ch = trimmed_before[..type_start].chars().next_back()?;
        matches!(ch, ' ' | '\t' | '(' | ',')
    };

    // The char after needle must be `)`, `,`, ` `, `:`, `=` or end of relevant token
    let after_needle = &line[pos + needle.len()..];
    let after_ok = after_needle.is_empty()
        || after_needle.starts_with([',', ')', ' ', ':', '=', '\n'].as_ref());

    if boundary && after_ok && is_model_like_class(&type_name) {
        let short = type_name.split('\\').last()?.to_string();
        Some(short)
    } else {
        None
    }
}

/// Heuristic: a class name looks like a model if it starts with an uppercase letter
/// and is not a PHP keyword or common non-model type.
fn is_model_like_class(name: &str) -> bool {
    let first_char = name.chars().next();
    matches!(first_char, Some(c) if c.is_uppercase())
        && !matches!(
            name,
            "Schema" | "Builder" | "Collection" | "Request" | "Response"
                | "Closure" | "Callable" | "Void" | "Never" | "Static" | "Self"
                | "True" | "False" | "Null" | "Int" | "Float" | "Bool" | "String"
                | "Array" | "Object" | "Mixed" | "Iterable" | "Countable"
                | "JsonSerializable" | "Throwable" | "Exception" | "Error"
        )
}

fn extract_model_class_from_resource(source: &str) -> Option<String> {
    for line in source.lines() {
        let t = line.trim();
        // Match: protected static ?string $model = SomeModel::class;
        // or: protected static ?SomeModel $model = SomeModel::class;
        if t.contains("$model") && t.contains("::class") {
            // Extract the identifier before `::class`
            let class_pos = t.rfind("::class")?;
            let before = &t[..class_pos];
            let class_name = before
                .chars()
                .rev()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '\\')
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            if !class_name.is_empty() {
                return Some(class_name);
            }
        }
    }
    None
}

fn source_chars_slice(text: &str, byte_start: usize, byte_end: usize) -> String {
    text.get(byte_start..byte_end)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        RouteActionKind, SymbolKind, ViewDataKind, detect_blade_component_attr_context,
        detect_blade_model_property_context, detect_blade_variable_context,
        detect_builder_arg_context, detect_route_action_context, detect_symbol_context,
        detect_vendor_chain_context, detect_vendor_make_context, detect_view_data_context,
    };

    #[test]
    fn detects_variable_context_inside_foreach_directive() {
        let source = "@foreach($bo as $b)";
        let line = 0;
        // position right after "bo" — cursor sits past both letters
        let character = source.find("$bo").expect("$bo") + 3;
        let ctx = detect_blade_variable_context(
            "resources/views/list.blade.php",
            source,
            line,
            character,
        );
        assert!(ctx.is_some(), "should detect variable inside @foreach parens");
        assert_eq!(ctx.unwrap().prefix, "bo");
    }

    #[test]
    fn detects_variable_context_inside_dd_directive() {
        let source = "@dd($user)";
        let line = 0;
        let character = source.find("$user").expect("$user") + 3;
        let ctx = detect_blade_variable_context(
            "resources/views/x.blade.php",
            source,
            line,
            character,
        );
        assert!(ctx.is_some(), "should detect variable inside @dd parens");
    }

    #[test]
    fn detects_model_property_context_inside_foreach_directive() {
        let source = "@foreach($books as $book->na)";
        let line = 0;
        let character = source.find("->na").expect("->na") + 4;
        let ctx = detect_blade_model_property_context(
            "resources/views/x.blade.php",
            source,
            line,
            character,
        );
        assert!(ctx.is_some(), "should detect model property inside @foreach parens");
    }

    #[test]
    fn resolves_foreach_item_var_to_collection_var() {
        let source = "@foreach($books as $book)\n{{ $book->na }}\n@endforeach";
        let line = 1;
        let character = source.lines().nth(1).unwrap().find("->na").unwrap() + 4;
        let ctx = detect_blade_model_property_context(
            "resources/views/x.blade.php",
            source,
            line,
            character,
        )
        .expect("should detect model property context");
        assert_eq!(ctx.variable_name, "books", "item var should resolve to collection var");
        assert_eq!(ctx.prefix, "na");
    }

    #[test]
    fn blade_component_attr_ignores_cursor_inside_attribute_value() {
        // Cursor inside route('book') — should NOT match attr context so route completion can fire
        let source = "<x-button href=\"{{ route('book') }}\" variant=\"primary\">Book</x-button>";
        let line = 0;
        let line_text = source;
        let character = line_text.find("book").expect("book") + 2; // mid-word in 'book'
        let ctx = detect_blade_component_attr_context(
            "resources/views/home.blade.php",
            source,
            line,
            character,
        );
        assert!(ctx.is_none(), "should not match inside a quoted attribute value");
    }

    #[test]
    fn blade_component_attr_matches_at_attribute_name() {
        // Cursor at an attribute name position — should still match
        let source = "<x-button var";
        let line = 0;
        let character = source.len(); // cursor at end, typing "var"
        let ctx = detect_blade_component_attr_context(
            "resources/views/home.blade.php",
            source,
            line,
            character,
        );
        assert!(ctx.is_some(), "should match when cursor is at an attribute name position");
        let ctx = ctx.unwrap();
        assert_eq!(ctx.component, "button");
        assert_eq!(ctx.prefix, "var");
    }

    #[test]
    fn detects_blade_model_property_context_in_echo() {
        let source = "{{ $user->na }}";
        let line = 0;
        let character = source.find("na").expect("na") + 1;
        let ctx = detect_blade_model_property_context(
            "resources/views/profile.blade.php",
            source,
            line,
            character,
        )
        .expect("should detect model property context");
        assert_eq!(ctx.variable_name, "user");
        assert_eq!(ctx.prefix, "n");
    }

    #[test]
    fn blade_model_property_context_ignores_non_blade_files() {
        let source = "{{ $user->na }}";
        let ctx = detect_blade_model_property_context("app/Http/Controllers/Foo.php", source, 0, 12);
        assert!(ctx.is_none());
    }

    #[test]
    fn blade_model_property_context_ignores_outside_echo() {
        let source = "$user->name";
        let ctx = detect_blade_model_property_context(
            "resources/views/x.blade.php",
            source,
            0,
            source.len(),
        );
        assert!(ctx.is_none());
    }

    #[test]
    fn detects_vendor_chain_context_on_arrow() {
        let source = "<?php\nuse Filament\\Forms\\Components\\TextInput;\n\nTextInput::make('label')\n    ->lab\n";
        let line = 4;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("->lab").expect("token") + "->lab".len();
        let ctx = detect_vendor_chain_context(source, line, character)
            .expect("vendor chain context");
        assert_eq!(ctx.class_fqn, "Filament\\Forms\\Components\\TextInput");
        assert_eq!(ctx.prefix, "lab");
    }

    #[test]
    fn detects_vendor_chain_context_inline() {
        let source = "<?php\nuse App\\Components\\Button;\n\nButton::make('x')->dis\n";
        let line = 3;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("->dis").expect("token") + "->dis".len();
        let ctx = detect_vendor_chain_context(source, line, character)
            .expect("vendor chain context");
        assert_eq!(ctx.class_fqn, "App\\Components\\Button");
        assert_eq!(ctx.prefix, "dis");
    }

    #[test]
    fn does_not_detect_vendor_chain_context_inside_parens() {
        let source = "TextInput::make('label')->label('Gr')\n";
        let line = 0;
        let character = source.find("->label('Gr')").expect("token") + "->label('G".len();
        let ctx = detect_vendor_chain_context(source, line, character);
        assert!(ctx.is_none(), "should not fire inside a method argument");
    }

    #[test]
    fn detects_vendor_make_context() {
        let source = "<?php\nuse Filament\\Forms\\Components\\TextInput;\n\nprotected static ?string $model = WhatsAppLink::class;\n\nTextInput::make('lab')\n";
        let line = 5;
        let line_text = source.lines().nth(line).expect("line should exist");
        let character = line_text.find("'lab'").expect("token") + "'lab".len();
        let ctx = detect_vendor_make_context("file:///tmp/app/Filament/Resources/WhatsAppLinkResource.php", source, line, character)
            .expect("vendor make context");
        assert_eq!(ctx.class_short, "TextInput");
        assert_eq!(ctx.prefix, "lab");
        assert_eq!(ctx.model_class.as_deref(), Some("WhatsAppLink"));
    }

    #[test]
    fn does_not_detect_vendor_make_context_for_non_make_calls() {
        let source = "TextInput::label('something')\n";
        let line = 0;
        let character = source.find("'something'").expect("token") + "'someth".len();
        let ctx = detect_vendor_make_context("file:///tmp/foo.php", source, line, character);
        assert!(ctx.is_none());
    }

    #[test]
    fn detects_vendor_make_context_for_separate_form_class() {
        // Simulates WhatsAppLinkForm — no $model here, but class name captured
        let source = "<?php\n\nnamespace App\\Filament\\Resources\\WhatsAppLinks\\Schemas;\n\nuse Filament\\Forms\\Components\\TextInput;\nuse Filament\\Schemas\\Schema;\n\nclass WhatsAppLinkForm\n{\n    public static function configure(Schema $schema): Schema\n    {\n        return $schema->components([\n            TextInput::make('lab')\n        ]);\n    }\n}\n";
        let line = source.lines().position(|l| l.contains("TextInput::make")).expect("line");
        let line_text = source.lines().nth(line).expect("line text");
        let character = line_text.find("'lab'").expect("token") + "'lab".len();
        let ctx = detect_vendor_make_context(
            "file:///tmp/app/Filament/Resources/WhatsAppLinks/Schemas/WhatsAppLinkForm.php",
            source,
            line,
            character,
        )
        .expect("vendor make context");

        assert_eq!(ctx.class_short, "TextInput");
        assert_eq!(ctx.prefix, "lab");
        // $model is NOT in this file — model_class should be None
        assert!(ctx.model_class.is_none());
        // But we should have captured the form class name for sibling lookup
        assert_eq!(ctx.current_class_name.as_deref(), Some("WhatsAppLinkForm"));
        // And the file path should be set
        assert!(ctx.current_file.is_some());
    }

    #[test]
    fn detects_builder_arg_context_for_pluck() {
        let source = "<?php\nstatic fn(BlogPost $record): array => $record->tagsWithType('category')->pluck('na')\n";
        let line = 1;
        let line_text = source.lines().nth(line).expect("line");
        let character = line_text.find("->pluck('na')").expect("token") + "->pluck('na".len();
        let ctx = detect_builder_arg_context(source, line, character).expect("builder arg context");
        assert_eq!(ctx.model_class, "BlogPost");
        assert_eq!(ctx.prefix, "na");
    }

    #[test]
    fn detects_builder_arg_context_for_order_by_static_chain() {
        let source = "<?php\nreturn Article::query()->orderBy('pub')\n";
        let line = 1;
        let line_text = source.lines().nth(line).expect("line");
        let character = line_text.find("->orderBy('pub')").expect("token") + "->orderBy('pub".len();
        let ctx = detect_builder_arg_context(source, line, character).expect("builder arg context");
        assert_eq!(ctx.model_class, "Article");
        assert_eq!(ctx.prefix, "pub");
    }

    #[test]
    fn does_not_detect_builder_arg_context_for_unknown_method() {
        let source = "<?php\nstatic fn(BlogPost $record) => $record->unknownMethod('na')\n";
        let line = 1;
        let line_text = source.lines().nth(line).expect("line");
        let character = line_text.find("->unknownMethod('na')").expect("token") + "->unknownMethod('na".len();
        let ctx = detect_builder_arg_context(source, line, character);
        assert!(ctx.is_none());
    }

    #[test]
    fn detects_builder_arg_context_from_phpdoc_var() {
        let source = "<?php\n/** @var BlogPost $post */\n$post->where('titl')\n";
        let line = 2;
        let line_text = source.lines().nth(line).expect("line");
        let character = line_text.find("->where('titl')").expect("token") + "->where('titl".len();
        let ctx = detect_builder_arg_context(source, line, character).expect("builder arg context");
        assert_eq!(ctx.model_class, "BlogPost");
        assert_eq!(ctx.prefix, "titl");
    }

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
