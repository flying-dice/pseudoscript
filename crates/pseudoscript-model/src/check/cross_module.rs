//! Cross-module visibility resolution (`LANG.md` §8.2, ADR-010).
//!
//! The single-module checks ([`super`]) cannot see other files, so they skip
//! references they cannot resolve locally. This pass runs over a whole
//! [`Workspace`]: it walks each module's qualified references — `for` parents,
//! `alias`/`feature` targets, body call targets, and type annotations (field,
//! parameter, return, and generic-argument types) — and against the global FQN
//! index enforces §8.2: a reference from module A to a symbol in module B
//! resolves only if that symbol is `public`. A cross-module reference to a
//! private symbol, or a dangling cross-module FQN, is a diagnostic.
//!
//! References whose target lives in the *same* module are left to the
//! single-module checks; only genuinely cross-module FQNs are judged here, so
//! no single-module scenario gains or loses a diagnostic.

use pseudoscript_syntax::Diagnostic;
use pseudoscript_syntax::ast::{
    Block, BodyMember, Callable, DataBody, DeclKind, Expr, ExprKind, Item, Node, Path, Ref, Stmt,
    StmtKind, Type,
};

use crate::model::{ModuleEntry, Resolution, Workspace};

/// Emits a diagnostic for every cross-module reference that resolves to a
/// private node or to nothing.
pub(crate) fn check(workspace: &Workspace) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for entry in workspace.modules() {
        check_module(workspace, entry, &mut out);
    }
    out
}

/// The cross-module diagnostics originating in a single module. Every span lies
/// in `entry`'s source, so the LSP can attribute them to that file's URI.
pub(crate) fn check_one(workspace: &Workspace, entry: &ModuleEntry) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    check_module(workspace, entry, &mut out);
    out
}

fn check_module(workspace: &Workspace, entry: &ModuleEntry, out: &mut Vec<Diagnostic>) {
    let mut ctx = Ctx {
        workspace,
        from_module: &entry.fqn,
        out,
    };
    for item in &entry.ast.items {
        match item {
            Item::Decl(decl) => ctx.check_decl_kind(&decl.kind),
            // §5.2: a cross-module feature target must be `public`.
            Item::Feature(feature) => ctx.check_ref(&feature.target, "feature target"),
        }
    }
}

struct Ctx<'a> {
    workspace: &'a Workspace,
    from_module: &'a str,
    out: &'a mut Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn check_decl_kind(&mut self, kind: &DeclKind) {
        match kind {
            DeclKind::Container(node) | DeclKind::Component(node) => {
                if let Some(parent) = &node.parent {
                    self.check_ref(parent, "parent");
                }
                self.check_node_body(node);
            }
            DeclKind::Person(node) | DeclKind::System(node) => self.check_node_body(node),
            DeclKind::Data(data) => self.check_data_types(&data.body),
        }
    }

    fn check_node_body(&mut self, node: &Node) {
        let Some(members) = &node.body else { return };
        for member in members {
            match member {
                BodyMember::Callable(callable) => self.check_callable(callable),
                BodyMember::Decl(decl) => self.check_decl_kind(&decl.kind),
            }
        }
    }

    fn check_callable(&mut self, callable: &Callable) {
        for param in &callable.params {
            self.check_type(&param.ty);
        }
        if let Some(ret) = &callable.return_ty {
            self.check_type(ret);
        }
        if let Some(body) = &callable.body {
            self.check_block(body);
        }
    }

    /// §3.3/§8.2: a qualified type — a field, parameter, or return type, and each
    /// generic argument — resolves cross-module under the same visibility rule as
    /// a node reference. A bare type is the single-module checks' business.
    fn check_data_types(&mut self, body: &DataBody) {
        match body {
            DataBody::Record(fields) => {
                for field in fields {
                    self.check_type(&field.ty);
                }
            }
            DataBody::Union(variants) => {
                for variant in variants {
                    for field in variant.record.iter().flatten() {
                        self.check_type(&field.ty);
                    }
                }
            }
            DataBody::BlackBox => {}
        }
    }

    fn check_type(&mut self, ty: &Type) {
        self.check_ref(&ty.name, "type");
        for generic in &ty.generics {
            self.check_type(generic);
        }
    }

    fn check_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Assign { value, .. } | StmtKind::Expr(value) => self.check_expr(value),
            StmtKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.check_expr(expr);
                }
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.check_expr(cond);
                self.check_block(then_block);
                if let Some(else_block) = else_block {
                    self.check_block(else_block);
                }
            }
            StmtKind::For { iter, body, .. } => {
                self.check_expr(iter);
                self.check_block(body);
            }
            StmtKind::While { cond, body } => {
                self.check_expr(cond);
                self.check_block(body);
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Postfix { base, segments } => {
                // The call target is the base of the chain (`Model::Builder` in
                // `Model::Builder.build(...)`); only it names a node.
                if let ExprKind::Ref(Ref::Path(path)) = &base.kind {
                    self.check_ref(path, "call target");
                } else {
                    self.check_expr(base);
                }
                for seg in segments {
                    if let Some(args) = &seg.call_args {
                        for arg in args {
                            self.check_expr(arg);
                        }
                    }
                }
            }
            ExprKind::From { sources, .. } => {
                for src in sources {
                    self.check_expr(src);
                }
            }
            ExprKind::Marker { payload, .. } => {
                if let Some(payload) = payload {
                    self.check_expr(payload);
                }
            }
            ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) => self.check_expr(expr),
            ExprKind::Ref(_) | ExprKind::Literal(_) => {}
        }
    }

    /// Judges one node-naming `Path` reference. Only multi-segment paths whose
    /// FQN resolves to a *different* module are cross-module; a bare name or a
    /// same-module FQN is the single-module checks' business and is skipped.
    fn check_ref(&mut self, path: &Path, what: &str) {
        let fqn = path_str(path);
        if path.is_simple() {
            return;
        }
        match self.workspace.resolve_qualified(self.from_module, &fqn) {
            Resolution::Public(_) => {}
            Resolution::Private(_) => self.out.push(Diagnostic::error(
                path.span,
                format!("{what} `{fqn}` is private to its module"),
            )),
            Resolution::Missing => {
                // A multi-segment path that is not in the global index is a
                // cross-module reference only when its module portion names a
                // module other than this one. Same-module multi-segment paths
                // (an FQN written in full to a local node) are the single-module
                // checks' concern; an unknown leaf in a *known other* module is
                // a dangling cross-module reference.
                if references_other_module(&fqn, self.from_module, self.workspace) {
                    self.out.push(Diagnostic::error(
                        path.span,
                        format!("dangling cross-module reference `{fqn}`: target does not resolve"),
                    ));
                }
            }
        }
    }
}

/// Whether `fqn`'s module portion names a known module (local or a dependency,
/// §8.3) other than `from_module` — i.e. the reference points across a module or
/// workspace boundary.
fn references_other_module(fqn: &str, from_module: &str, workspace: &Workspace) -> bool {
    let Some((module, _)) = fqn.rsplit_once("::") else {
        return false;
    };
    module != from_module && workspace.is_known_module(module)
}

/// Renders a `Path` as its `::`-joined source form.
fn path_str(path: &Path) -> String {
    path.segments
        .iter()
        .map(|id| id.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}
