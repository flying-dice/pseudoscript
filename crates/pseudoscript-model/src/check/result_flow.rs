//! Flow-sensitive `Result` / `Option` accessor checking (`LANG.md` §6).
//!
//! Each binding carries a typestate — `Unknown`, `Ok`/`Err` (a `Result`), or
//! `Some`/`None` (an `Option`) — narrowed on entering the branches of
//! `if (r.isErr)` / `if (r.isOk)` / `if (o.isNone)` / `if (o.isSome)`. Reading
//! `.value` while a binding is known-`Err` or known-`None`, or `.error` while it
//! is known-`Ok`, is a model error. The env is cloned per branch so narrowing
//! does not leak.

use pseudoscript_syntax::Diagnostic;
use pseudoscript_syntax::ast::{Block, Expr, ExprKind, PostfixSeg, Ref, Stmt, StmtKind};
use rustc_hash::FxHashMap;

/// Which `Result`/`Option` branch a binding is known to be on at a program
/// point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Branch not yet determined.
    Unknown,
    /// Known `Ok` (e.g. inside `if (r.isOk)` or after an `Err` branch diverged).
    Ok,
    /// Known `Err`.
    Err,
    /// Known `Some` of an `Option`.
    Some,
    /// Known `None`.
    None,
}

/// The per-point typestate environment: binding name to its known branch.
#[derive(Debug, Clone, Default)]
pub(crate) struct Bindings {
    states: FxHashMap<String, State>,
}

impl Bindings {
    fn get(&self, name: &str) -> State {
        self.states.get(name).copied().unwrap_or(State::Unknown)
    }

    fn set(&mut self, name: &str, state: State) {
        self.states.insert(name.to_owned(), state);
    }
}

/// Checks the body of one callable for wrong-branch accessor reads.
pub(crate) fn check_callable_result_flow(
    block: &Block,
    env: &mut Bindings,
    out: &mut Vec<Diagnostic>,
) {
    check_block(block, env, out);
}

/// Returns whether the block diverges (every path ends in `return`), used to
/// narrow the continuation after an `if` whose else-arm is absent.
fn check_block(block: &Block, env: &mut Bindings, out: &mut Vec<Diagnostic>) -> bool {
    let mut diverges = false;
    for stmt in &block.stmts {
        if check_stmt(stmt, env, out) {
            diverges = true;
        }
    }
    diverges
}

fn check_stmt(stmt: &Stmt, env: &mut Bindings, out: &mut Vec<Diagnostic>) -> bool {
    match &stmt.kind {
        StmtKind::Assign { name, value, .. } => {
            check_expr(value, env, out);
            // A fresh binding starts Unknown; we only learn its branch through a
            // subsequent `if` guard.
            env.set(&name.name, State::Unknown);
            false
        }
        StmtKind::Return(expr) => {
            if let Some(expr) = expr {
                check_expr(expr, env, out);
            }
            true
        }
        StmtKind::Expr(expr) => {
            check_expr(expr, env, out);
            false
        }
        StmtKind::If {
            cond,
            then_block,
            else_block,
        } => check_if(cond, then_block, else_block.as_ref(), env, out),
        StmtKind::For { iter, body, .. } => {
            check_expr(iter, env, out);
            let mut loop_env = env.clone();
            check_block(body, &mut loop_env, out);
            false
        }
        StmtKind::While { cond, body } => {
            check_expr(cond, env, out);
            let mut loop_env = env.clone();
            check_block(body, &mut loop_env, out);
            false
        }
    }
}

fn check_if(
    cond: &Expr,
    then_block: &Block,
    else_block: Option<&Block>,
    env: &mut Bindings,
    out: &mut Vec<Diagnostic>,
) -> bool {
    check_expr(cond, env, out);
    let narrow = branch_guard(cond);

    let mut then_env = env.clone();
    if let Some((name, state)) = &narrow {
        then_env.set(name, *state);
    }
    let then_diverges = check_block(then_block, &mut then_env, out);

    let else_diverges = if let Some(else_block) = else_block {
        let mut else_env = env.clone();
        if let Some((name, state)) = &narrow {
            else_env.set(name, state.inverse());
        }
        check_block(else_block, &mut else_env, out)
    } else {
        false
    };

    // Continuation narrowing: if the then-branch (the guarded state) diverges,
    // the fall-through carries the inverse state for the guarded binding.
    if then_diverges
        && else_block.is_none()
        && let Some((name, state)) = &narrow
    {
        env.set(name, state.inverse());
    }

    match else_block {
        Some(_) => then_diverges && else_diverges,
        None => false,
    }
}

impl State {
    fn inverse(self) -> State {
        match self {
            State::Ok => State::Err,
            State::Err => State::Ok,
            State::Some => State::None,
            State::None => State::Some,
            State::Unknown => State::Unknown,
        }
    }
}

/// If `cond` is a bare `r.isErr` / `r.isOk` / `o.isNone` / `o.isSome` guard,
/// returns the binding name and the branch it narrows to inside the then-block.
fn branch_guard(cond: &Expr) -> Option<(String, State)> {
    let ExprKind::Postfix { base, segments } = &cond.kind else {
        return None;
    };
    let [seg] = segments.as_slice() else {
        return None;
    };
    if seg.call_args.is_some() {
        return None;
    }
    let name = ref_name(base)?;
    match seg.name.name.as_str() {
        "isErr" => Some((name, State::Err)),
        "isOk" => Some((name, State::Ok)),
        "isNone" => Some((name, State::None)),
        "isSome" => Some((name, State::Some)),
        _ => None,
    }
}

/// Walks an expression, reporting any wrong-branch `.value` / `.error` read.
fn check_expr(expr: &Expr, env: &Bindings, out: &mut Vec<Diagnostic>) {
    match &expr.kind {
        ExprKind::Postfix { base, segments } => {
            check_accessor(base, segments, env, out);
            check_expr(base, env, out);
            for seg in segments {
                if let Some(args) = &seg.call_args {
                    for arg in args {
                        check_expr(arg, env, out);
                    }
                }
            }
        }
        ExprKind::Marker { payload, .. } => {
            if let Some(payload) = payload {
                check_expr(payload, env, out);
            }
        }
        ExprKind::From { sources, .. } => {
            for src in sources {
                check_expr(src, env, out);
            }
        }
        ExprKind::Unary { expr, .. } | ExprKind::Paren(expr) => check_expr(expr, env, out),
        ExprKind::Ref(_) | ExprKind::Literal(_) => {}
    }
}

/// Reports a wrong-branch read when a postfix chain is exactly `r.value` /
/// `r.error` over a known-state binding.
fn check_accessor(base: &Expr, segments: &[PostfixSeg], env: &Bindings, out: &mut Vec<Diagnostic>) {
    let [seg] = segments else { return };
    if seg.call_args.is_some() {
        return;
    }
    let Some(name) = ref_name(base) else { return };
    match (seg.name.name.as_str(), env.get(&name)) {
        ("value", State::Err) => {
            out.push(Diagnostic::error(
                seg.span,
                "`.value` read on an `Err` branch",
            ));
        }
        ("value", State::None) => {
            out.push(Diagnostic::error(
                seg.span,
                "`.value` read on a `None` branch",
            ));
        }
        ("error", State::Ok) => {
            out.push(Diagnostic::error(
                seg.span,
                "`.error` read on an `Ok` branch",
            ));
        }
        _ => {}
    }
}

/// The bare binding name of a `Ref` expression (`r`), if the base is a simple
/// path reference (not `self`, not a multi-segment FQN).
fn ref_name(expr: &Expr) -> Option<String> {
    match &expr.kind {
        ExprKind::Ref(Ref::Path(path)) if path.is_simple() => Some(path.segments[0].name.clone()),
        _ => None,
    }
}
