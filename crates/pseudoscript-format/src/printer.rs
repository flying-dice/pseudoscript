//! The canonical pretty-printer: walks the AST, emitting one canonical form.
//!
//! Each `write_*` method appends to a [`Printer`] buffer. Indentation is tracked
//! as a level (two spaces each). Leading trivia is normalised: a run of blank
//! lines collapses to at most one blank separator, and `//`/`/* */` comments are
//! reproduced at the current indent.

use pseudoscript_syntax::ast::{
    Block, BodyMember, Callable, Constant, Data, DataBody, Decl, DeclKind, DocBlock, Expr,
    ExprKind, Feature, Field, FromSource, Ident, InnerDoc, Item, Literal, Macro, MacroArg,
    MacroArgs, Module, Node, NodeKind, Param, Path, PostfixSeg, Ref, Stmt, StmtKind, Type, Variant,
};
use pseudoscript_syntax::{SpannedTrivia, Trivia};

const INDENT: &str = "  ";

/// Pretty-prints a whole module to canonical text with a trailing newline.
pub(crate) fn print_module(module: &Module) -> String {
    let mut p = Printer::default();
    p.write_module(module);
    p.finish()
}

#[derive(Default)]
struct Printer {
    out: String,
    indent: usize,
    /// `true` once the current line has indentation written, so further writes
    /// on the same line append without re-indenting.
    line_started: bool,
}

impl Printer {
    fn finish(mut self) -> String {
        // Exactly one trailing newline; trim any accidental trailing blanks.
        while self.out.ends_with('\n') {
            self.out.pop();
        }
        if !self.out.is_empty() {
            self.out.push('\n');
        }
        self.out
    }

    // --- low-level emit -----------------------------------------------------

    /// Appends text to the current line, writing indentation first if the line
    /// has not started yet.
    fn push(&mut self, text: &str) {
        if !self.line_started {
            for _ in 0..self.indent {
                self.out.push_str(INDENT);
            }
            self.line_started = true;
        }
        self.out.push_str(text);
    }

    /// Ends the current line.
    fn newline(&mut self) {
        self.out.push('\n');
        self.line_started = false;
    }

    /// Emits one blank line, unless the output is empty or already ends in a
    /// blank line (collapses runs to at most one).
    fn blank_line(&mut self) {
        if self.out.is_empty() {
            return;
        }
        if self.out.ends_with("\n\n") {
            return;
        }
        // The current line must be terminated before a blank line is meaningful.
        debug_assert!(!self.line_started);
        self.out.push('\n');
    }

    // --- trivia -------------------------------------------------------------

    /// Emits leading trivia before a declaration, member, or statement: blank
    /// lines collapse to at most one separator, and `//`/`/* */` comments are
    /// reproduced on their own lines at the current indent, in source order.
    ///
    /// `first` is `true` for the first item in a block or file, which never gets
    /// a leading blank line.
    fn write_member_separator(&mut self, trivia: &[SpannedTrivia], first: bool) {
        let mut pending_blank = false;
        for t in trivia {
            match &t.trivia {
                Trivia::BlankLines(_) => pending_blank = true,
                Trivia::LineComment(text) | Trivia::BlockComment(text) => {
                    if pending_blank && !first {
                        self.blank_line();
                    }
                    pending_blank = false;
                    self.write_comment(text);
                }
            }
        }
        if pending_blank && !first {
            self.blank_line();
        }
    }

    /// Writes one comment. Block comments may span lines; each physical line is
    /// re-indented at the current level.
    fn write_comment(&mut self, text: &str) {
        let mut lines = text.split('\n');
        if let Some(first) = lines.next() {
            self.push(first.trim_end());
            self.newline();
        }
        for line in lines {
            // Preserve interior block-comment lines verbatim but re-indented;
            // trim trailing whitespace only.
            self.push(line.trim_end());
            self.newline();
        }
    }

    // --- module -------------------------------------------------------------

    fn write_module(&mut self, module: &Module) {
        for doc in &module.inner_docs {
            self.write_inner_doc(doc);
        }
        if !module.inner_docs.is_empty() && !module.items.is_empty() {
            self.blank_line();
        }
        for (i, item) in module.items.iter().enumerate() {
            self.write_item(item, i == 0);
        }
    }

    fn write_inner_doc(&mut self, doc: &InnerDoc) {
        if doc.text.is_empty() {
            self.push("//!");
        } else {
            self.push("//! ");
            self.push(&doc.text);
        }
        self.newline();
    }

    fn write_item(&mut self, item: &Item, first: bool) {
        let trivia = match item {
            Item::Decl(d) => &d.leading_trivia,
            Item::Feature(f) => &f.leading_trivia,
        };
        self.write_member_separator(trivia, first);
        match item {
            Item::Decl(d) => self.write_decl(d),
            Item::Feature(f) => self.write_feature(f),
        }
    }

    // --- features (§5.2) ----------------------------------------------------

    /// Writes a feature: its doc block, the `feature Name for Path` header, then
    /// one step per line. Step prose is left as written; keywords are not
    /// column-aligned, so the form is idempotent.
    fn write_feature(&mut self, feature: &Feature) {
        self.write_doc_block(&feature.doc);
        self.push("feature ");
        self.push(&feature.name.name);
        self.push(" for ");
        self.write_path(&feature.target);
        if feature.steps.is_empty() {
            self.push(" { }");
            self.newline();
            return;
        }
        self.push(" {");
        self.newline();
        self.indent += 1;
        for step in &feature.steps {
            self.push(step.kind.keyword());
            self.push(" ");
            self.write_literal(&step.text);
            self.newline();
        }
        self.indent -= 1;
        self.push("}");
        self.newline();
    }

    // --- declarations -------------------------------------------------------

    fn write_decl(&mut self, decl: &Decl) {
        self.write_doc_block(&decl.doc);
        for m in &decl.macros {
            self.write_macro(m);
            self.newline();
        }
        if decl.is_public {
            self.push("public ");
        }
        match &decl.kind {
            DeclKind::Person(n) => self.write_node("person", n),
            DeclKind::System(n) => self.write_node("system", n),
            DeclKind::Container(n) => self.write_node("container", n),
            DeclKind::Component(n) => self.write_node("component", n),
            DeclKind::Data(d) => self.write_data(d),
            DeclKind::Constant(c) => self.write_constant(c),
        }
    }

    /// Writes `constant NAME = <literal>` on one line (§3.6); `public` is written
    /// by [`Self::write_decl`].
    fn write_constant(&mut self, constant: &Constant) {
        self.push("constant ");
        self.push(&constant.name.name);
        self.push(" = ");
        self.write_literal(&constant.value);
        self.newline();
    }

    fn write_node(&mut self, keyword: &str, node: &Node) {
        debug_assert!(matches!(
            (keyword, node.kind),
            ("person", NodeKind::Person)
                | ("system", NodeKind::System)
                | ("container", NodeKind::Container)
                | ("component", NodeKind::Component)
        ));
        self.push(keyword);
        self.push(" ");
        self.push(&node.name.name);
        if let Some(parent) = &node.parent {
            self.push(" for ");
            self.write_path(parent);
        }
        match &node.body {
            None => {
                self.push(";");
                self.newline();
            }
            Some(members) => {
                self.write_node_body(members);
            }
        }
    }

    fn write_node_body(&mut self, members: &[BodyMember]) {
        if members.is_empty() {
            self.push(" { }");
            self.newline();
            return;
        }
        self.push(" {");
        self.newline();
        self.indent += 1;
        for (i, member) in members.iter().enumerate() {
            let trivia = match member {
                BodyMember::Callable(c) => &c.leading_trivia,
                BodyMember::Decl(d) => &d.leading_trivia,
            };
            self.write_member_separator(trivia, i == 0);
            match member {
                BodyMember::Callable(c) => self.write_callable(c),
                BodyMember::Decl(d) => self.write_decl(d),
            }
        }
        self.indent -= 1;
        self.push("}");
        self.newline();
    }

    fn write_data(&mut self, data: &Data) {
        self.push("data ");
        self.push(&data.name.name);
        match &data.body {
            DataBody::BlackBox => {
                self.push(";");
                self.newline();
            }
            DataBody::Record(fields) => {
                self.write_record(fields);
                self.newline();
            }
            DataBody::Union(variants) => {
                self.push(" =");
                self.newline();
                self.indent += 1;
                for v in variants {
                    self.write_variant(v);
                }
                self.indent -= 1;
            }
        }
    }

    /// Writes a record body inline: `{ a: T, b: U }`, or `{ }` when empty.
    fn write_record(&mut self, fields: &[Field]) {
        if fields.is_empty() {
            self.push(" { }");
            return;
        }
        self.push(" { ");
        for (i, field) in fields.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.write_field(field);
        }
        self.push(" }");
    }

    fn write_field(&mut self, field: &Field) {
        self.push(&field.name.name);
        self.push(": ");
        self.write_type(&field.ty);
    }

    fn write_variant(&mut self, variant: &Variant) {
        self.push("| ");
        self.push(&variant.name.name);
        if let Some(fields) = &variant.record {
            self.write_record(fields);
        }
        self.newline();
    }

    // --- callables ----------------------------------------------------------

    fn write_callable(&mut self, callable: &Callable) {
        self.write_doc_block(&callable.doc);
        for m in &callable.macros {
            self.write_macro(m);
            self.newline();
        }
        if callable.is_public {
            self.push("public ");
        }
        self.push(&callable.name.name);
        self.push("(");
        for (i, param) in callable.params.iter().enumerate() {
            if i > 0 {
                self.push(", ");
            }
            self.write_param(param);
        }
        self.push(")");
        if let Some(ret) = &callable.return_ty {
            self.push(": ");
            self.write_type(ret);
        }
        match &callable.body {
            None => {
                self.push(";");
                self.newline();
            }
            Some(block) => {
                self.write_block(block);
                self.newline();
            }
        }
    }

    fn write_param(&mut self, param: &Param) {
        self.push(&param.name.name);
        self.push(": ");
        self.write_type(&param.ty);
    }

    // --- statements & blocks ------------------------------------------------

    /// Writes a `{ ... }` block, appended to the current line, opening with
    /// ` {`. An empty block renders as ` { }`. Does not emit a trailing newline.
    fn write_block(&mut self, block: &Block) {
        if block.stmts.is_empty() {
            self.push(" { }");
            return;
        }
        self.push(" {");
        self.newline();
        self.indent += 1;
        for (i, stmt) in block.stmts.iter().enumerate() {
            self.write_member_separator(&stmt.leading_trivia, i == 0);
            self.write_stmt(stmt);
        }
        self.indent -= 1;
        self.push("}");
    }

    fn write_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Assign { name, value } => {
                self.push(&name.name);
                self.push(" = ");
                self.write_expr(value);
                self.newline();
            }
            StmtKind::Return(value) => {
                self.push("return");
                if let Some(expr) = value {
                    self.push(" ");
                    self.write_expr(expr);
                }
                self.newline();
            }
            StmtKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.push("if (");
                self.write_expr(cond);
                self.push(")");
                self.write_block(then_block);
                if let Some(else_block) = else_block {
                    self.push(" else");
                    self.write_block(else_block);
                }
                self.newline();
            }
            StmtKind::For {
                binding,
                iter,
                body,
            } => {
                self.push("for (");
                self.push(&binding.name);
                self.push(" in ");
                self.write_expr(iter);
                self.push(")");
                self.write_block(body);
                self.newline();
            }
            StmtKind::While { cond, body } => {
                self.push("while (");
                self.write_expr(cond);
                self.push(")");
                self.write_block(body);
                self.newline();
            }
            StmtKind::Expr(expr) => {
                self.write_expr(expr);
                self.newline();
            }
        }
    }

    // --- expressions --------------------------------------------------------

    fn write_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Marker { kind, payload } => {
                self.push(kind.keyword());
                if let Some(payload) = payload {
                    self.push("(");
                    self.write_expr(payload);
                    self.push(")");
                }
            }
            ExprKind::From { ty, source } => {
                self.write_type(ty);
                match source {
                    FromSource::Compose(sources) if sources.is_empty() => self.push(" from {}"),
                    FromSource::Compose(sources) => {
                        self.push(" from { ");
                        for (i, src) in sources.iter().enumerate() {
                            if i > 0 {
                                self.push(", ");
                            }
                            self.write_expr(src);
                        }
                        self.push(" }");
                    }
                    FromSource::Convert(expr) => {
                        self.push(" from ");
                        self.write_expr(expr);
                    }
                }
            }
            ExprKind::Postfix { base, segments } => {
                self.write_expr(base);
                for seg in segments {
                    self.write_postfix_seg(seg);
                }
            }
            ExprKind::Ref(r) => self.write_ref(r),
            ExprKind::Literal(lit) => self.write_literal(lit),
            ExprKind::Unary { op, expr, .. } => {
                self.push(op.spelling());
                self.write_expr(expr);
            }
            // §7.5: `left op right`. Explicit grouping is carried by `Paren`
            // nodes, so the round-trip stays faithful without re-deriving
            // precedence.
            ExprKind::Binary {
                left, op, right, ..
            } => {
                self.write_expr(left);
                self.push(" ");
                self.push(op.spelling());
                self.push(" ");
                self.write_expr(right);
            }
            ExprKind::Paren(inner) => {
                self.push("(");
                self.write_expr(inner);
                self.push(")");
            }
        }
    }

    fn write_postfix_seg(&mut self, seg: &PostfixSeg) {
        self.push(".");
        self.push(&seg.name.name);
        if let Some(args) = &seg.call_args {
            self.push("(");
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    self.push(", ");
                }
                self.write_expr(arg);
            }
            self.push(")");
        }
    }

    fn write_ref(&mut self, r: &Ref) {
        match r {
            Ref::SelfNode(_) => self.push("self"),
            Ref::Path(path) => self.write_path(path),
        }
    }

    fn write_literal(&mut self, lit: &Literal) {
        match lit {
            Literal::String { raw, .. } | Literal::Number { raw, .. } => self.push(raw),
            Literal::Bool { value, .. } => self.push(if *value { "true" } else { "false" }),
        }
    }

    // --- types & paths ------------------------------------------------------

    fn write_type(&mut self, ty: &Type) {
        self.write_path(&ty.name);
        if !ty.generics.is_empty() {
            self.push("<");
            for (i, g) in ty.generics.iter().enumerate() {
                if i > 0 {
                    self.push(", ");
                }
                self.write_type(g);
            }
            self.push(">");
        }
        if ty.is_array {
            self.push("[]");
        }
    }

    fn write_path(&mut self, path: &Path) {
        for (i, seg) in path.segments.iter().enumerate() {
            if i > 0 {
                self.push("::");
            }
            self.write_ident(seg);
        }
    }

    fn write_ident(&mut self, ident: &Ident) {
        self.push(&ident.name);
    }

    // --- doc blocks & macros ------------------------------------------------

    fn write_doc_block(&mut self, doc: &DocBlock) {
        if doc.is_empty() {
            return;
        }
        for line in &doc.summary {
            self.write_doc_line(line);
        }
        if !doc.extended.is_empty() {
            self.push("///");
            self.newline();
            for line in &doc.extended {
                self.write_doc_line(line);
            }
        }
        for tag in &doc.tags {
            self.push("/// ");
            self.push(&tag.text);
            self.newline();
        }
    }

    fn write_doc_line(&mut self, text: &str) {
        if text.is_empty() {
            self.push("///");
        } else {
            self.push("/// ");
            self.push(text);
        }
        self.newline();
    }

    fn write_macro(&mut self, m: &Macro) {
        self.push("#[");
        self.write_path(&m.name);
        match &m.args {
            MacroArgs::Word => {}
            MacroArgs::List(args) => {
                self.push("(");
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        self.push(", ");
                    }
                    self.write_macro_arg(arg);
                }
                self.push(")");
            }
            MacroArgs::NameValue(lit) => {
                self.push(" = ");
                self.write_literal(lit);
            }
        }
        self.push("]");
    }

    fn write_macro_arg(&mut self, arg: &MacroArg) {
        match arg {
            MacroArg::Literal(lit) => self.write_literal(lit),
            MacroArg::Path(path) => self.write_path(path),
            MacroArg::Nested(m) => self.write_macro(m),
        }
    }
}
