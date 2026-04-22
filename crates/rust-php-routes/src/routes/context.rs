use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(crate) struct RouteContext {
    pub(crate) uri_prefix: String,
    pub(crate) name_prefix: String,
    pub(crate) middleware: Vec<String>,
    pub(crate) controller: Option<String>,
}

#[derive(Clone, Default)]
pub struct MiddlewareIndex {
    pub(crate) aliases: BTreeMap<String, String>,
    pub(crate) groups: BTreeMap<String, Vec<String>>,
    pub(crate) patterns: BTreeMap<String, String>,
}

impl MiddlewareIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_aliases(mut self, aliases: impl IntoIterator<Item = (String, String)>) -> Self {
        self.aliases.extend(aliases);
        self
    }

    pub fn with_groups(mut self, groups: impl IntoIterator<Item = (String, Vec<String>)>) -> Self {
        self.groups.extend(groups);
        self
    }

    pub fn with_patterns(mut self, patterns: impl IntoIterator<Item = (String, String)>) -> Self {
        self.patterns.extend(patterns);
        self
    }
}

pub(crate) fn resolve_middleware(values: &[String], index: &MiddlewareIndex) -> Vec<String> {
    let mut resolved = Vec::new();
    let mut stack = Vec::new();
    for value in values {
        expand_middleware(value, index, &mut stack, &mut resolved);
    }
    resolved
}

fn expand_middleware(
    value: &str,
    index: &MiddlewareIndex,
    stack: &mut Vec<String>,
    resolved: &mut Vec<String>,
) {
    if stack.iter().any(|item| item == value) {
        return;
    }
    if let Some(group) = index.groups.get(value) {
        stack.push(value.to_string());
        for member in group.clone() {
            expand_middleware(&member, index, stack, resolved);
        }
        stack.pop();
        return;
    }
    let target = index
        .aliases
        .get(value)
        .cloned()
        .unwrap_or_else(|| value.to_string());
    if !resolved.iter().any(|item| item == &target) {
        resolved.push(target);
    }
}

pub(crate) fn collect_parameter_patterns(
    uri: &str,
    index: &MiddlewareIndex,
) -> BTreeMap<String, String> {
    let mut patterns = BTreeMap::new();
    let mut search = uri;
    while let Some(start) = search.find('{') {
        let rest = &search[start + 1..];
        let Some(end) = rest.find('}') else { break };
        let raw = &rest[..end];
        let parameter = raw
            .trim_end_matches('?')
            .split(':')
            .next()
            .unwrap_or(raw)
            .to_string();
        if let Some(pattern) = index.patterns.get(&parameter) {
            patterns.insert(parameter, pattern.clone());
        }
        search = &rest[end + 1..];
    }
    patterns
}
