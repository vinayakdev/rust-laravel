use php_parser::ast::{ClassMember, ExprId, Stmt, StmtId};

/// Recursively visits every expression found inside a slice of PHP statements.
///
/// `include_class_methods` controls whether method bodies inside
/// class/interface/trait/enum declarations are descended into.
/// Pass `true` for provider analysis (methods register middleware/routes),
/// `false` for route-file analysis where only top-level expressions matter.
pub fn walk_stmts<'ast, F>(stmts: &[StmtId<'ast>], include_class_methods: bool, f: &mut F)
where
    F: FnMut(ExprId<'ast>),
{
    for stmt in stmts.iter().copied() {
        walk_one(stmt, include_class_methods, f);
    }
}

fn walk_one<'ast, F>(stmt: StmtId<'ast>, include_class_methods: bool, f: &mut F)
where
    F: FnMut(ExprId<'ast>),
{
    match stmt {
        Stmt::Expression { expr, .. } => f(expr),
        Stmt::Block { statements, .. }
        | Stmt::Declare { body: statements, .. } => {
            walk_stmts(statements, include_class_methods, f);
        }
        Stmt::Namespace { body: Some(body), .. } => {
            walk_stmts(body, include_class_methods, f);
        }
        Stmt::Class { members, .. }
        | Stmt::Interface { members, .. }
        | Stmt::Trait { members, .. }
        | Stmt::Enum { members, .. } => {
            if include_class_methods {
                for member in members.iter().copied() {
                    if let ClassMember::Method { body, .. } = member {
                        walk_stmts(body, include_class_methods, f);
                    }
                }
            }
        }
        Stmt::If { then_block, else_block, .. } => {
            walk_stmts(then_block, include_class_methods, f);
            if let Some(else_block) = else_block {
                walk_stmts(else_block, include_class_methods, f);
            }
        }
        Stmt::While { body, .. }
        | Stmt::DoWhile { body, .. }
        | Stmt::For { body, .. }
        | Stmt::Foreach { body, .. }
        | Stmt::Try { body, .. } => {
            walk_stmts(body, include_class_methods, f);
        }
        _ => {}
    }
}
