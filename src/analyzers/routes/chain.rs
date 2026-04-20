use php_parser::ast::{Arg, Expr, ExprId};
use std::path::Path;

use super::context::{
    MiddlewareIndex, RouteContext, collect_parameter_patterns, resolve_middleware,
};
use crate::php::ast::{expr_name, expr_to_string, expr_to_string_list, strip_root};
use crate::types::{RouteEntry, RouteRegistration};

#[derive(Clone, Copy)]
pub(crate) enum ChainOp<'ast> {
    StaticCall {
        class: ExprId<'ast>,
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
    },
    MethodCall {
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
    },
}

pub(crate) struct RouteSignature {
    pub(crate) creator: String,
    pub(crate) methods: Vec<String>,
    pub(crate) uri_arg_index: usize,
    pub(crate) action_arg_index: usize,
}

#[derive(Clone)]
pub(crate) struct ResourceRouteSpec {
    pub(crate) suffix: String,
    pub(crate) methods: Vec<String>,
    pub(crate) action: String,
    pub(crate) name_suffix: String,
}

pub(crate) fn flatten_route_chain(expr: ExprId<'_>) -> Option<Vec<ChainOp<'_>>> {
    let mut ops = Vec::new();
    fn visit<'a>(expr: ExprId<'a>, ops: &mut Vec<ChainOp<'a>>) -> bool {
        match expr {
            Expr::MethodCall {
                target,
                method,
                args,
                ..
            } => {
                if !visit(target, ops) {
                    return false;
                }
                ops.push(ChainOp::MethodCall { method, args });
                true
            }
            Expr::StaticCall {
                class,
                method,
                args,
                ..
            } => {
                ops.push(ChainOp::StaticCall {
                    class,
                    method,
                    args,
                });
                true
            }
            _ => false,
        }
    }
    if visit(expr, &mut ops) {
        Some(ops)
    } else {
        None
    }
}

pub(crate) fn route_signature(
    method_name: &str,
    args: &[Arg<'_>],
    source: &[u8],
) -> Option<RouteSignature> {
    let simple = |name: &str| {
        Some(RouteSignature {
            creator: name.to_ascii_lowercase(),
            methods: vec![name.to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        })
    };
    match method_name {
        "get" => simple("GET"),
        "post" => simple("POST"),
        "put" => simple("PUT"),
        "patch" => simple("PATCH"),
        "delete" => simple("DELETE"),
        "options" => simple("OPTIONS"),
        "any" => simple("ANY"),
        "view" => Some(RouteSignature {
            creator: "view".to_string(),
            methods: vec!["GET".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "redirect" | "permanentRedirect" => Some(RouteSignature {
            creator: method_name.to_string(),
            methods: vec!["GET".to_string()],
            uri_arg_index: 0,
            action_arg_index: 1,
        }),
        "fallback" => Some(RouteSignature {
            creator: "fallback".to_string(),
            methods: vec!["ANY".to_string()],
            uri_arg_index: usize::MAX,
            action_arg_index: 0,
        }),
        "match" => args
            .first()
            .map(|a| {
                expr_to_string_list(a.value, source)
                    .into_iter()
                    .map(|m| m.to_ascii_uppercase())
                    .collect::<Vec<_>>()
            })
            .filter(|m| !m.is_empty())
            .map(|methods| RouteSignature {
                creator: "match".to_string(),
                methods,
                uri_arg_index: 1,
                action_arg_index: 2,
            }),
        _ => None,
    }
}

pub(crate) fn apply_modifier(
    context: &mut RouteContext,
    route: Option<&mut RouteEntry>,
    method_name: &str,
    args: &[Arg<'_>],
    source: &[u8],
) {
    match method_name {
        "prefix" => {
            if let Some(value) = args.first().and_then(|a| expr_to_string(a.value, source)) {
                if let Some(route) = route {
                    route.uri = join_uri(&value, &route.uri);
                } else {
                    context.uri_prefix = join_uri(&context.uri_prefix, &value);
                }
            }
        }
        "name" | "as" => {
            if let Some(value) = args.first().and_then(|a| expr_to_string(a.value, source)) {
                if let Some(route) = route {
                    let current = route.name.take().unwrap_or_default();
                    route.name = Some(format!("{current}{value}"));
                } else {
                    context.name_prefix.push_str(&value);
                }
            }
        }
        "middleware" => {
            let values = args_to_string_list(args, source);
            if let Some(route) = route {
                route.middleware.extend(values);
            } else {
                context.middleware.extend(values);
            }
        }
        "controller" => {
            if let Some(value) = args
                .first()
                .and_then(|a| expr_to_controller(a.value, source))
            {
                context.controller = Some(value);
            }
        }
        _ => {}
    }
}

pub(crate) fn build_route_entry(
    context: &RouteContext,
    project_root: &Path,
    file: &Path,
    registration: &RouteRegistration,
    line: usize,
    signature: RouteSignature,
    args: &[Arg<'_>],
    source: &[u8],
    middleware_index: &MiddlewareIndex,
) -> RouteEntry {
    let (line, column) = route_position(source, args, line);
    let raw_uri = if signature.uri_arg_index == usize::MAX {
        "{fallbackPlaceholder}".to_string()
    } else {
        args.get(signature.uri_arg_index)
            .and_then(|a| expr_to_string(a.value, source))
            .unwrap_or_else(|| "/".to_string())
    };
    let action = build_special_action(&signature, args, context.controller.as_deref(), source)
        .or_else(|| {
            args.get(signature.action_arg_index)
                .and_then(|a| expr_to_action(a.value, context.controller.as_deref(), source))
        });

    let uri = join_uri(&context.uri_prefix, &raw_uri);
    let resolved_middleware = resolve_middleware(&context.middleware, middleware_index);
    let parameter_patterns = collect_parameter_patterns(&uri, middleware_index);

    RouteEntry {
        file: strip_root(project_root, file),
        line,
        column,
        methods: signature.methods,
        uri,
        name: (!context.name_prefix.is_empty()).then(|| context.name_prefix.clone()),
        action,
        middleware: context.middleware.clone(),
        resolved_middleware,
        parameter_patterns,
        registration: registration.clone(),
    }
}

fn build_special_action(
    signature: &RouteSignature,
    args: &[Arg<'_>],
    controller: Option<&str>,
    source: &[u8],
) -> Option<String> {
    match signature.creator.as_str() {
        "view" => args
            .get(signature.action_arg_index)
            .and_then(|a| expr_to_string(a.value, source))
            .map(|view| format!("view:{view}")),
        "redirect" => args
            .get(signature.action_arg_index)
            .and_then(|a| expr_to_string(a.value, source))
            .map(|target| format!("redirect:{target}")),
        "permanentRedirect" => args
            .get(signature.action_arg_index)
            .and_then(|a| expr_to_string(a.value, source))
            .map(|target| format!("redirect-permanent:{target}")),
        "fallback" => args
            .get(signature.action_arg_index)
            .and_then(|a| expr_to_action(a.value, controller, source)),
        _ => None,
    }
}

pub(crate) fn resource_routes(
    resource: &str,
    controller: &str,
    api: bool,
    singleton: bool,
) -> Vec<ResourceRouteSpec> {
    let base = resource.trim_matches('/').to_string();
    let name_base = base.replace('/', ".");
    let resource_key = base
        .rsplit('/')
        .next()
        .unwrap_or(base.as_str())
        .trim_end_matches('s')
        .to_string();

    let mut routes = Vec::new();

    if !singleton {
        routes.push(ResourceRouteSpec {
            suffix: "".to_string(),
            methods: vec!["GET".to_string()],
            action: format!("{controller}@index"),
            name_suffix: format!("{name_base}.index"),
        });
    }

    if !api {
        routes.push(ResourceRouteSpec {
            suffix: "/create".to_string(),
            methods: vec!["GET".to_string()],
            action: format!("{controller}@create"),
            name_suffix: format!("{name_base}.create"),
        });
    }

    routes.push(ResourceRouteSpec {
        suffix: "".to_string(),
        methods: vec!["POST".to_string()],
        action: format!("{controller}@store"),
        name_suffix: format!("{name_base}.store"),
    });

    if singleton {
        routes.push(ResourceRouteSpec {
            suffix: "".to_string(),
            methods: vec!["GET".to_string()],
            action: format!("{controller}@show"),
            name_suffix: format!("{name_base}.show"),
        });
        if !api {
            routes.push(ResourceRouteSpec {
                suffix: "/edit".to_string(),
                methods: vec!["GET".to_string()],
                action: format!("{controller}@edit"),
                name_suffix: format!("{name_base}.edit"),
            });
        }
        routes.push(ResourceRouteSpec {
            suffix: "".to_string(),
            methods: vec!["PUT".to_string(), "PATCH".to_string()],
            action: format!("{controller}@update"),
            name_suffix: format!("{name_base}.update"),
        });
        routes.push(ResourceRouteSpec {
            suffix: "".to_string(),
            methods: vec!["DELETE".to_string()],
            action: format!("{controller}@destroy"),
            name_suffix: format!("{name_base}.destroy"),
        });
        return routes;
    }

    let member = format!("/{{{resource_key}}}");
    routes.push(ResourceRouteSpec {
        suffix: member.clone(),
        methods: vec!["GET".to_string()],
        action: format!("{controller}@show"),
        name_suffix: format!("{name_base}.show"),
    });

    if !api {
        routes.push(ResourceRouteSpec {
            suffix: format!("{member}/edit"),
            methods: vec!["GET".to_string()],
            action: format!("{controller}@edit"),
            name_suffix: format!("{name_base}.edit"),
        });
    }

    routes.push(ResourceRouteSpec {
        suffix: member.clone(),
        methods: vec!["PUT".to_string(), "PATCH".to_string()],
        action: format!("{controller}@update"),
        name_suffix: format!("{name_base}.update"),
    });
    routes.push(ResourceRouteSpec {
        suffix: member,
        methods: vec!["DELETE".to_string()],
        action: format!("{controller}@destroy"),
        name_suffix: format!("{name_base}.destroy"),
    });

    routes
}

pub(crate) fn route_line(expr: ExprId<'_>, source: &[u8], line_offset: usize) -> usize {
    line_offset + chunk_relative_line(expr, source) - 1
}

fn route_position(source: &[u8], args: &[Arg<'_>], fallback_line: usize) -> (usize, usize) {
    if let Some(arg) = args.first()
        && let Some(info) = arg.value.span().line_info(source)
    {
        return (fallback_line, info.column);
    }
    (fallback_line, 1)
}

fn chunk_relative_line(expr: ExprId<'_>, source: &[u8]) -> usize {
    expr.span().line_info(source).map_or(1, |i| i.line)
}

fn expr_to_controller(expr: ExprId<'_>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::ClassConstFetch {
            class, constant, ..
        } => {
            let class_name = expr_name(class, source)?;
            let constant_name = expr_name(constant, source)?;
            if constant_name == "class" {
                Some(class_name)
            } else {
                Some(format!("{class_name}::{constant_name}"))
            }
        }
        _ => expr_to_string(expr, source),
    }
}

fn expr_to_action(expr: ExprId<'_>, controller: Option<&str>, source: &[u8]) -> Option<String> {
    match expr {
        Expr::Closure { .. } | Expr::ArrowFunction { .. } => Some("closure".to_string()),
        Expr::ClassConstFetch { .. } => expr_to_controller(expr, source),
        Expr::Array { items, .. } if items.len() >= 2 => {
            let controller_name = expr_to_controller(items.first()?.value, source)?;
            let method_name = expr_to_string(items.get(1)?.value, source)?;
            Some(format!("{controller_name}@{method_name}"))
        }
        _ => {
            let value = expr_to_string(expr, source)?;
            if let Some(controller) = controller {
                if !value.contains('@') && !value.contains("::") {
                    return Some(format!("{controller}@{value}"));
                }
            }
            Some(value)
        }
    }
}

pub(crate) fn join_uri(prefix: &str, path: &str) -> String {
    let prefix = prefix.trim_matches('/');
    let path = path.trim_matches('/');
    match (prefix.is_empty(), path.is_empty()) {
        (true, true) => "/".to_string(),
        (true, false) => format!("/{path}"),
        (false, true) => format!("/{prefix}"),
        (false, false) => format!("/{prefix}/{path}"),
    }
}

pub(crate) fn args_to_string_list(args: &[Arg<'_>], source: &[u8]) -> Vec<String> {
    let mut values = Vec::new();
    for arg in args {
        values.extend(expr_to_string_list(arg.value, source));
    }
    values
}
