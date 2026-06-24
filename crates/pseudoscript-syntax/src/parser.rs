//! Recursive-descent parser for the `LANG.md` §10 grammar.
//!
//! The parser never panics: on unexpected input it records a [`Diagnostic`] and
//! resynchronises to the next statement/declaration boundary, then continues.
//! The result always carries a (possibly partial) [`Module`].

use crate::ast::{
    BinOp, Block, BodyMember, Callable, Constant, Data, DataBody, Decl, DeclKind, DocBlock, Expr,
    ExprKind, Feature, FeatureStep, Field, FromSource, Ident, InnerDoc, Item, Literal, Macro,
    MacroArg, MacroArgs, MarkerKind, Module, Node, NodeKind, Param, Path, PostfixSeg, Ref,
    StepKind, Stmt, StmtKind, Tag, Type, UnaryOp, Variant,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::{Lexed, SpannedTrivia, lex};
use crate::span::Span;
use crate::token::{Token, TokenKind};

/// The output of [`parse`]: the syntax tree and any diagnostics.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// The parsed module (partial if there were errors).
    pub ast: Module,
    /// Diagnostics produced while parsing (errors and warnings).
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses `src` into a [`Module`], recovering from errors (§10).
#[must_use]
#[tracing::instrument(level = "debug", skip(src), fields(bytes = src.len()))]
pub fn parse(src: &str) -> Parsed {
    let lexed = lex(src);
    let mut parser = Parser::new(src, lexed);
    let ast = parser.parse_module();
    Parsed {
        ast,
        diagnostics: parser.diagnostics,
    }
}

struct Parser {
    tokens: Vec<Token>,
    trivia: Vec<SpannedTrivia>,
    pos: usize,
    trivia_pos: usize,
    diagnostics: Vec<Diagnostic>,
    eof_span: Span,
}

/// The flow phase reached while parsing a feature body (§5.2). `and`/`but`
/// continue the current phase without advancing it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FeaturePhase {
    /// No step seen yet.
    Start,
    /// Inside the `given` run.
    Given,
    /// Inside the `when` run.
    When,
    /// Inside the `then` run.
    Then,
}

impl Parser {
    fn new(src: &str, lexed: Lexed) -> Self {
        let end = src.len() as u32;
        Self {
            tokens: lexed.tokens,
            trivia: lexed.trivia,
            pos: 0,
            trivia_pos: 0,
            diagnostics: Vec::new(),
            eof_span: Span::new(end, end),
        }
    }

    // --- token cursor -------------------------------------------------------

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }

    fn peek2_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos + 1).map(|t| t.kind)
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek_kind() == Some(kind)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Consumes `kind` if present, returning its token.
    fn eat(&mut self, kind: TokenKind) -> Option<Token> {
        if self.at(kind) { self.bump() } else { None }
    }

    /// The span of the current token, or end-of-input.
    fn cur_span(&self) -> Span {
        self.peek().map_or(self.eof_span, |t| t.span)
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic::error(span, message));
    }

    /// Drains trivia whose end is at or before `offset` and returns it, so the
    /// next declaration/statement can own the comments that preceded it.
    fn take_trivia_before(&mut self, offset: u32) -> Vec<SpannedTrivia> {
        let mut out = Vec::new();
        while let Some(t) = self.trivia.get(self.trivia_pos) {
            if t.span.end <= offset {
                out.push(t.clone());
                self.trivia_pos += 1;
            } else {
                break;
            }
        }
        out
    }

    // --- module -------------------------------------------------------------

    fn parse_module(&mut self) -> Module {
        let start = self.cur_span().start;
        let mut inner_docs = Vec::new();
        while self.at(TokenKind::InnerDoc) {
            let token = self.bump().expect("peeked InnerDoc");
            inner_docs.push(InnerDoc {
                text: token.text,
                span: token.span,
            });
        }

        let mut items = Vec::new();
        while !self.is_eof() {
            // Inner docs may appear after blank lines; absorb stray ones.
            if self.at(TokenKind::InnerDoc) {
                let token = self.bump().expect("peeked InnerDoc");
                inner_docs.push(InnerDoc {
                    text: token.text,
                    span: token.span,
                });
                continue;
            }
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                // No progress possible at this token: skip it to avoid a
                // loop, after recording the gap once.
                let span = self.cur_span();
                self.error(span, "unexpected token at top level");
                self.bump();
                self.recover_to_item();
            }
        }

        let end = items.last().map_or(start, |i| i.span().end);
        Module {
            inner_docs,
            items,
            span: Span::new(start, end.max(start)),
        }
    }

    fn parse_item(&mut self) -> Option<Item> {
        let leading_trivia = self.take_trivia_before(self.cur_span().start);

        // Decl = DocBlock { Macro } { Modifier } Structural
        let doc = self.parse_doc_block();
        let macros = self.parse_macros();
        let is_public = self.eat(TokenKind::KwPublic).is_some();

        let start = leading_trivia
            .first()
            .map(|t| t.span.start)
            .or_else(|| doc.tags.first().map(|t| t.span.start))
            .unwrap_or_else(|| self.cur_span().start);

        // Feature = DocBlock "feature" Ident "for" Path FeatureBody (§5.2). A
        // feature takes no macros and no modifier; reject any that were consumed.
        if self.at(TokenKind::KwFeature) {
            return Some(Item::Feature(self.parse_feature(
                leading_trivia,
                doc,
                &macros,
                is_public,
                start,
            )));
        }

        let kind = self.parse_decl_kind()?;
        let span = Span::new(start, self.prev_end());
        Some(Item::Decl(Decl {
            doc,
            macros,
            is_public,
            kind,
            leading_trivia,
            span,
        }))
    }

    /// End offset of the most recently consumed token.
    fn prev_end(&self) -> u32 {
        if self.pos == 0 {
            return 0;
        }
        self.tokens
            .get(self.pos - 1)
            .map_or(self.eof_span.end, |t| t.span.end)
    }

    // --- features (§5.2) ----------------------------------------------------

    /// Parses `feature Name for Path { given* when+ then+ }`. `macros`/`is_public`
    /// were consumed by [`Self::parse_item`]; a feature accepts neither, so any
    /// present are rejected (§5.2).
    fn parse_feature(
        &mut self,
        leading_trivia: Vec<SpannedTrivia>,
        doc: DocBlock,
        macros: &[Macro],
        is_public: bool,
        start: u32,
    ) -> Feature {
        let kw = self.bump().expect("peeked feature");
        for mac in macros {
            self.error(mac.span, "macro on a `feature`: macros target callables");
        }
        if is_public {
            self.error(kw.span, "`public` modifier on a `feature`");
        }

        let name = self.expect_ident("feature name");
        let target = if self.eat(TokenKind::KwFor).is_some() {
            self.parse_path()
        } else {
            self.error(
                name.span,
                "feature declaration missing `for <target>` clause",
            );
            Path {
                segments: Vec::new(),
                span: name.span,
            }
        };

        let steps = self.parse_feature_body(name.span);
        Feature {
            doc,
            name,
            target,
            steps,
            leading_trivia,
            span: Span::new(start, self.prev_end()),
        }
    }

    /// Parses the `{ ... }` flow block, enforcing the strict given* when+ then+
    /// order: `and`/`but` continue the preceding step's kind; a `then` before any
    /// `when`, a `when` after a `then`, or a leading `and`/`but` are rejected.
    fn parse_feature_body(&mut self, name_span: Span) -> Vec<FeatureStep> {
        if !self.at(TokenKind::LBrace) {
            self.eat(TokenKind::Semi);
            self.error(name_span, "feature declaration has no `{ }` flow block");
            return Vec::new();
        }
        self.bump(); // `{`

        let mut steps = Vec::new();
        let mut phase = FeaturePhase::Start;

        while !self.is_eof() && !self.at(TokenKind::RBrace) {
            let before = self.pos;
            let Some(step_kind) = self.peek_kind().and_then(StepKind::from_token) else {
                let span = self.cur_span();
                self.error(span, "expected a `given`, `when`, or `then` step");
                self.recover_in_block();
                if self.pos == before {
                    self.bump();
                }
                continue;
            };

            let kw = self.bump().expect("peeked step keyword");
            match step_kind {
                StepKind::Given => match phase {
                    FeaturePhase::Start | FeaturePhase::Given => phase = FeaturePhase::Given,
                    FeaturePhase::When => self.error(kw.span, "feature flow: `given` after `when`"),
                    FeaturePhase::Then => self.error(kw.span, "feature flow: `given` after `then`"),
                },
                StepKind::When => match phase {
                    FeaturePhase::Start | FeaturePhase::Given | FeaturePhase::When => {
                        phase = FeaturePhase::When;
                    }
                    FeaturePhase::Then => self.error(kw.span, "feature flow: `when` after `then`"),
                },
                StepKind::Then => match phase {
                    FeaturePhase::Start | FeaturePhase::Given => {
                        self.error(kw.span, "feature flow: `then` before any `when`");
                        // Advance to the `then` phase so repeats and the
                        // missing-`when`/`then` guards don't pile on for one mistake.
                        phase = FeaturePhase::Then;
                    }
                    FeaturePhase::When | FeaturePhase::Then => phase = FeaturePhase::Then,
                },
                StepKind::And | StepKind::But => {
                    if phase == FeaturePhase::Start {
                        self.error(
                            kw.span,
                            format!("feature flow: leading `{}` with no preceding step", kw.text),
                        );
                    }
                }
            }

            let text = self.parse_step_text();
            steps.push(FeatureStep {
                kind: step_kind,
                span: Span::new(kw.span.start, self.prev_end()),
                text,
            });
        }
        self.eat(TokenKind::RBrace);

        // §5.2: one or more `when` and one or more `then` are required. Reaching
        // the `When`/`Then` phase means a valid step of that kind was seen.
        if !matches!(phase, FeaturePhase::When | FeaturePhase::Then) {
            self.error(name_span, "feature flow: missing `when` step");
        }
        if phase != FeaturePhase::Then {
            self.error(name_span, "feature flow: missing `then` step");
        }
        steps
    }

    /// Parses a step's prose: a string literal. A non-string step body is an
    /// error; an empty string is synthesised so recovery continues.
    fn parse_step_text(&mut self) -> Literal {
        if self.at(TokenKind::String) {
            self.parse_literal().expect("peeked string")
        } else {
            let span = self.cur_span();
            self.error(span, "feature step expects a string literal");
            Literal::String {
                raw: String::new(),
                span,
            }
        }
    }

    // --- declarations -------------------------------------------------------

    fn parse_decl_kind(&mut self) -> Option<DeclKind> {
        match self.peek_kind()? {
            TokenKind::KwPerson => Some(DeclKind::Person(self.parse_node(NodeKind::Person))),
            TokenKind::KwSystem => Some(DeclKind::System(self.parse_node(NodeKind::System))),
            TokenKind::KwContainer => {
                Some(DeclKind::Container(self.parse_node(NodeKind::Container)))
            }
            TokenKind::KwComponent => {
                Some(DeclKind::Component(self.parse_node(NodeKind::Component)))
            }
            TokenKind::KwData => Some(DeclKind::Data(self.parse_data())),
            TokenKind::KwConstant => Some(DeclKind::Constant(self.parse_constant())),
            _ => None,
        }
    }

    /// Parses `constant Ident "=" Literal` (§3.6, ADR-039). The value MUST be a
    /// primitive literal; a non-literal right-hand side is rejected and a
    /// placeholder `0` substituted for recovery.
    fn parse_constant(&mut self) -> Constant {
        let kw = self.bump().expect("peeked `constant`");
        let name = self.expect_ident("constant name");
        if self.eat(TokenKind::Eq).is_none() {
            self.error(self.cur_span(), "expected `=` after the constant name");
        }
        let value = self.parse_literal().unwrap_or_else(|| {
            let span = self.cur_span();
            self.error(
                span,
                "a constant value must be a primitive literal (number, string, or bool)",
            );
            Literal::Number {
                raw: "0".to_owned(),
                span,
            }
        });
        Constant {
            name,
            span: Span::new(kw.span.start, self.prev_end()),
            value,
        }
    }

    fn parse_node(&mut self, kind: NodeKind) -> Node {
        let kw = self.bump().expect("peeked node keyword");
        let name = self.expect_ident("node name");

        let mut parent = None;
        if matches!(kind, NodeKind::Container | NodeKind::Component) {
            if self.eat(TokenKind::KwFor).is_some() {
                parent = Some(self.parse_path());
            } else {
                let noun = if kind == NodeKind::Container {
                    "container"
                } else {
                    "component"
                };
                self.error(
                    name.span,
                    format!("{noun} declaration missing `for <parent>` clause"),
                );
            }
        }

        let body = self.parse_node_body(kind, name.span);
        let end = self.prev_end();
        Node {
            kind,
            name,
            parent,
            body,
            span: Span::new(kw.span.start, end),
        }
    }

    /// Parses a node body: a `{ members }` block, a `;` black box, or — if
    /// neither is present — records the missing-terminator error (§2.5).
    fn parse_node_body(&mut self, kind: NodeKind, name_span: Span) -> Option<Vec<BodyMember>> {
        if self.at(TokenKind::LBrace) {
            return Some(self.parse_body_block());
        }
        if self.eat(TokenKind::Semi).is_some() {
            return None;
        }
        let noun = match kind {
            NodeKind::Person => "person",
            NodeKind::System => "system",
            NodeKind::Container => "container",
            NodeKind::Component => "component",
        };
        self.error(
            name_span,
            format!("{noun} declaration with neither a `{{ }}` block nor a terminating `;`"),
        );
        None
    }

    /// Parses a `{ ... }` body block of callables and nested declarations.
    fn parse_body_block(&mut self) -> Vec<BodyMember> {
        self.bump(); // `{`
        let mut members = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::RBrace) {
            let before = self.pos;
            let member = self.parse_body_member();
            if self.pos == before {
                // Forward-progress guard (see `parse_block`): a token starting
                // neither a callable nor a nested declaration must not spin.
                let span = self.cur_span();
                self.error(span, "expected a callable or declaration");
                self.recover_in_block();
            } else if let Some(m) = member {
                members.push(m);
            }
        }
        self.eat(TokenKind::RBrace);
        members
    }

    /// Parses one body member: shared `doc → macros → public` prefix, then a
    /// nested structural declaration if a structural keyword follows, otherwise
    /// a callable.
    fn parse_body_member(&mut self) -> Option<BodyMember> {
        let leading_trivia = self.take_trivia_before(self.cur_span().start);
        let doc = self.parse_doc_block();
        let macros = self.parse_macros();
        let is_public = self.eat(TokenKind::KwPublic).is_some();
        let prefix_consumed = !doc.is_empty() || !macros.is_empty() || is_public;
        let start = leading_trivia
            .first()
            .map_or_else(|| self.cur_span().start, |t| t.span.start);

        if matches!(
            self.peek_kind(),
            Some(
                TokenKind::KwPerson
                    | TokenKind::KwSystem
                    | TokenKind::KwContainer
                    | TokenKind::KwComponent
                    | TokenKind::KwData
                    | TokenKind::KwConstant
            )
        ) {
            // ADR-011 / §5: a disclosed block holds callables only. Containers
            // and components are top-level declarations wired by `for` and MUST
            // NOT nest inside a block. Parse the declaration anyway (error
            // recovery) but reject it.
            let (kw_span, kw_lexeme) = {
                let kw = self.peek().expect("structural keyword present");
                (kw.span, kw.text.clone())
            };
            self.error(
                kw_span,
                format!(
                    "`{kw_lexeme}` declaration inside a block: a disclosed block holds callables only"
                ),
            );
            let kind = self.parse_decl_kind()?;
            let span = Span::new(start, self.prev_end());
            return Some(BodyMember::Decl(Decl {
                doc,
                macros,
                is_public,
                kind,
                leading_trivia,
                span,
            }));
        }

        self.parse_callable(
            leading_trivia,
            doc,
            macros,
            is_public,
            prefix_consumed,
            start,
        )
        .map(BodyMember::Callable)
    }

    #[allow(clippy::too_many_arguments)]
    fn parse_callable(
        &mut self,
        leading_trivia: Vec<SpannedTrivia>,
        doc: DocBlock,
        macros: Vec<Macro>,
        is_public: bool,
        prefix_consumed: bool,
        start: u32,
    ) -> Option<Callable> {
        if !self.at(TokenKind::Ident) {
            // Nothing callable-shaped here. If we consumed docs/macros, that is
            // still progress; otherwise the caller recovers.
            if !prefix_consumed {
                return None;
            }
            let span = self.cur_span();
            self.error(span, "expected a callable name");
            return None;
        }

        let name = self.expect_ident("callable name");

        self.eat(TokenKind::LParen);
        let params = self.parse_params();
        self.eat(TokenKind::RParen);

        // ADR-040: every callable declares a return type; a missing one is
        // reported and recovered as `void` (`return_ty` stays `None`).
        let return_ty = if self.eat(TokenKind::Colon).is_some() {
            Some(self.parse_type())
        } else {
            self.error(name.span, "callable without a declared return type");
            None
        };

        let body = if self.at(TokenKind::LBrace) {
            Some(self.parse_block())
        } else if self.eat(TokenKind::Semi).is_some() {
            None
        } else {
            self.error(
                name.span,
                "callable with neither a `{ }` body block nor a terminating `;`",
            );
            None
        };

        Some(Callable {
            doc,
            macros,
            is_public,
            name,
            params,
            return_ty,
            body,
            leading_trivia,
            span: Span::new(start, self.prev_end()),
        })
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::RParen) {
            if !self.at_name() {
                break;
            }
            let name = self.expect_name("parameter name");
            self.eat(TokenKind::Colon);
            let ty = self.parse_type();
            let span = name.span.to(ty.span);
            params.push(Param { name, ty, span });
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        params
    }

    // --- data ---------------------------------------------------------------

    fn parse_data(&mut self) -> Data {
        let kw = self.bump().expect("peeked data");
        let name = self.expect_ident("data name");

        let body = if self.at(TokenKind::LBrace) {
            DataBody::Record(self.parse_record())
        } else if self.eat(TokenKind::Eq).is_some() {
            DataBody::Union(self.parse_union())
        } else if self.eat(TokenKind::Semi).is_some() {
            DataBody::BlackBox
        } else {
            self.error(
                name.span,
                "data declaration with neither a `{ }` block, `= union`, nor a terminating `;`",
            );
            DataBody::BlackBox
        };

        Data {
            name,
            body,
            span: Span::new(kw.span.start, self.prev_end()),
        }
    }

    fn parse_record(&mut self) -> Vec<Field> {
        self.bump(); // `{`
        let mut fields = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::RBrace) {
            if !self.at_name() {
                let span = self.cur_span();
                self.error(span, "expected a field name");
                self.recover_in_block();
                if self.at(TokenKind::RBrace) {
                    break;
                }
                continue;
            }
            let name = self.expect_name("field name");
            self.eat(TokenKind::Colon);
            let ty = self.parse_type();
            let span = name.span.to(ty.span);
            fields.push(Field { name, ty, span });
            self.eat(TokenKind::Comma);
        }
        self.eat(TokenKind::RBrace);
        fields
    }

    fn parse_union(&mut self) -> Vec<Variant> {
        let mut variants = Vec::new();
        while self.eat(TokenKind::Pipe).is_some() {
            if !self.at_name() {
                let span = self.cur_span();
                self.error(span, "expected a union variant name");
                break;
            }
            let name = self.expect_name("variant name");
            let record = if self.at(TokenKind::LBrace) {
                Some(self.parse_record())
            } else {
                None
            };
            let end = self.prev_end();
            variants.push(Variant {
                span: Span::new(name.span.start, end),
                name,
                record,
            });
        }
        variants
    }

    // --- types --------------------------------------------------------------

    fn parse_type(&mut self) -> Type {
        let path = self.parse_path();
        self.finish_type(path)
    }

    /// Completes a [`Type`] from an already-parsed base `path`: optional `<..>`
    /// generics and an optional `[]` array suffix (§3.3, ADR-008).
    fn finish_type(&mut self, path: Path) -> Type {
        let mut span = path.span;

        let mut generics = Vec::new();
        if self.at(TokenKind::LAngle) {
            self.bump();
            loop {
                generics.push(self.parse_type());
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            if let Some(gt) = self.eat(TokenKind::RAngle) {
                span = span.to(gt.span);
            } else {
                self.error(self.cur_span(), "expected `>` to close generic arguments");
            }
        }

        // `[]` array suffix (ADR-008: the only type suffix).
        if self.at(TokenKind::LBracket) && self.peek2_kind() == Some(TokenKind::RBracket) {
            self.bump();
            let rb = self.bump().expect("peeked `]`");
            span = span.to(rb.span);
            return Type {
                name: path,
                generics,
                is_array: true,
                span,
            };
        }

        // A `?` in type position is the removed optionality marker (ADR-008).
        if self.at(TokenKind::Question) {
            let q = self.cur_span();
            self.error(q, "unexpected `?` in type: no optionality marker");
            self.bump();
        }

        Type {
            name: path,
            generics,
            is_array: false,
            span,
        }
    }

    // --- doc blocks, tags, macros -------------------------------------------

    /// Parses a run of `DOC`/`TAG` tokens into a [`DocBlock`], splitting summary
    /// from extended on the first blank `///` line (ADR-009).
    fn parse_doc_block(&mut self) -> DocBlock {
        let mut block = DocBlock::default();
        let mut in_extended = false;
        let mut seen_summary = false;

        while let Some(kind) = self.peek_kind() {
            match kind {
                TokenKind::Doc => {
                    let token = self.bump().expect("peeked Doc");
                    if token.text.is_empty() {
                        // Blank `///` line: switch to the extended section once.
                        if seen_summary {
                            in_extended = true;
                        }
                    } else if in_extended {
                        block.extended.push(token.text);
                    } else {
                        block.summary.push(token.text);
                        seen_summary = true;
                    }
                }
                TokenKind::Tag => {
                    let token = self.bump().expect("peeked Tag");
                    block.tags.push(Tag {
                        text: token.text,
                        span: token.span,
                    });
                }
                _ => break,
            }
        }
        block
    }

    fn parse_macros(&mut self) -> Vec<Macro> {
        let mut macros = Vec::new();
        while self.at(TokenKind::HashLBracket) {
            macros.push(self.parse_macro());
        }
        macros
    }

    fn parse_macro(&mut self) -> Macro {
        let open = self.bump().expect("peeked `#[`");
        let name = self.parse_path();

        let args = if self.at(TokenKind::LParen) {
            self.bump();
            let mut list = Vec::new();
            while !self.is_eof() && !self.at(TokenKind::RParen) {
                if let Some(arg) = self.parse_macro_arg() {
                    list.push(arg);
                } else {
                    break;
                }
                if self.eat(TokenKind::Comma).is_none() {
                    break;
                }
            }
            self.eat(TokenKind::RParen);
            MacroArgs::List(list)
        } else if self.eat(TokenKind::Eq).is_some() {
            if let Some(lit) = self.parse_literal() {
                MacroArgs::NameValue(lit)
            } else {
                self.error(self.cur_span(), "expected a literal after `=` in macro");
                MacroArgs::Word
            }
        } else {
            MacroArgs::Word
        };

        let end = if let Some(rb) = self.eat(TokenKind::RBracket) {
            rb.span.end
        } else {
            self.error(self.cur_span(), "expected `]` to close macro");
            self.prev_end()
        };

        Macro {
            name,
            args,
            span: Span::new(open.span.start, end),
        }
    }

    fn parse_macro_arg(&mut self) -> Option<MacroArg> {
        match self.peek_kind()? {
            TokenKind::String | TokenKind::Number | TokenKind::KwTrue | TokenKind::KwFalse => {
                self.parse_literal().map(MacroArg::Literal)
            }
            TokenKind::Ident => {
                let path = self.parse_path();
                if self.at(TokenKind::LParen) || self.at(TokenKind::Eq) {
                    // A nested meta item, e.g. `outer(inner(...))`.
                    let args = if self.at(TokenKind::LParen) {
                        self.bump();
                        let mut list = Vec::new();
                        while !self.is_eof() && !self.at(TokenKind::RParen) {
                            if let Some(arg) = self.parse_macro_arg() {
                                list.push(arg);
                            } else {
                                break;
                            }
                            if self.eat(TokenKind::Comma).is_none() {
                                break;
                            }
                        }
                        self.eat(TokenKind::RParen);
                        MacroArgs::List(list)
                    } else {
                        self.bump();
                        self.parse_literal()
                            .map_or(MacroArgs::Word, MacroArgs::NameValue)
                    };
                    let span = path.span;
                    Some(MacroArg::Nested(Box::new(Macro {
                        name: path,
                        args,
                        span,
                    })))
                } else {
                    Some(MacroArg::Path(path))
                }
            }
            _ => None,
        }
    }

    // --- statements & blocks ------------------------------------------------

    fn parse_block(&mut self) -> Block {
        let open = self.bump().expect("`{`");
        let mut stmts = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::RBrace) {
            let before = self.pos;
            let stmt = self.parse_stmt();
            if self.pos == before {
                // Forward-progress guard: the current token cannot start a
                // statement and `parse_stmt` consumed nothing (e.g. a stray `;`
                // — §10 has no statement terminator). Recover so the loop can
                // never spin; `recover_in_block` advances past the offending
                // token.
                let span = self.cur_span();
                self.error(span, "expected a statement");
                self.recover_in_block();
            } else if let Some(stmt) = stmt {
                stmts.push(stmt);
            }
        }
        let end = self
            .eat(TokenKind::RBrace)
            .map_or(self.prev_end(), |t| t.span.end);
        Block {
            stmts,
            span: Span::new(open.span.start, end),
        }
    }

    fn parse_stmt(&mut self) -> Option<Stmt> {
        let leading_trivia = self.take_trivia_before(self.cur_span().start);
        let start = leading_trivia
            .first()
            .map_or_else(|| self.cur_span().start, |t| t.span.start);

        let kind = match self.peek_kind()? {
            TokenKind::KwReturn => self.parse_return(),
            TokenKind::KwIf => self.parse_if(),
            TokenKind::KwFor => self.parse_for(),
            TokenKind::KwWhile => self.parse_while(),
            // `name = Expr` binds a name once (§7.1); its type comes from a
            // `from` right-hand side (ADR-035).
            TokenKind::Ident if self.peek2_kind() == Some(TokenKind::Eq) => {
                let name = self.expect_ident("binding name");
                self.bump(); // `=`
                let value = self.parse_expr();
                StmtKind::Assign { name, value }
            }
            // Binding annotations are removed (ADR-035): a binding states its
            // type through `from`. Recover from the old `name: Type = Expr`
            // shape with one precise diagnostic.
            TokenKind::Ident if self.peek2_kind() == Some(TokenKind::Colon) => {
                let name = self.expect_ident("binding name");
                self.bump(); // `:`
                let _ty = self.parse_type();
                self.eat(TokenKind::Eq);
                let value = self.parse_expr();
                self.error(
                    name.span,
                    "a binding states its type through `from`: write `name = Type from …`",
                );
                StmtKind::Assign { name, value }
            }
            _ => StmtKind::Expr(self.parse_expr()),
        };

        Some(Stmt {
            kind,
            leading_trivia,
            span: Span::new(start, self.prev_end()),
        })
    }

    fn parse_return(&mut self) -> StmtKind {
        self.bump(); // `return`
        // A bare `return` (no expression) is valid for void callables.
        let value = if self.starts_expr() {
            Some(self.parse_expr())
        } else {
            None
        };
        StmtKind::Return(value)
    }

    fn parse_if(&mut self) -> StmtKind {
        self.bump(); // `if`
        let cond = self.parse_paren_cond();
        let then_block = self.parse_block();
        let else_block = if self.eat(TokenKind::KwElse).is_some() {
            Some(self.parse_block())
        } else {
            None
        };
        StmtKind::If {
            cond,
            then_block,
            else_block,
        }
    }

    fn parse_for(&mut self) -> StmtKind {
        self.bump(); // `for`
        self.eat(TokenKind::LParen);
        let binding = self.expect_ident("loop binding");
        self.eat(TokenKind::KwIn);
        let iter = self.parse_expr();
        self.eat(TokenKind::RParen);
        let body = self.parse_block();
        StmtKind::For {
            binding,
            iter,
            body,
        }
    }

    fn parse_while(&mut self) -> StmtKind {
        self.bump(); // `while`
        let cond = self.parse_paren_cond();
        let body = self.parse_block();
        StmtKind::While { cond, body }
    }

    fn parse_paren_cond(&mut self) -> Expr {
        self.eat(TokenKind::LParen);
        let cond = self.parse_expr();
        self.eat(TokenKind::RParen);
        cond
    }

    // --- expressions --------------------------------------------------------

    fn starts_expr(&self) -> bool {
        matches!(
            self.peek_kind(),
            Some(
                TokenKind::KwOk
                    | TokenKind::KwErr
                    | TokenKind::KwSome
                    | TokenKind::KwNone
                    | TokenKind::KwSelf
                    | TokenKind::Ident
                    | TokenKind::String
                    | TokenKind::Number
                    | TokenKind::KwTrue
                    | TokenKind::KwFalse
                    | TokenKind::Bang
                    | TokenKind::Minus
                    | TokenKind::LParen
            )
        )
    }

    /// Whether the cursor begins a `FromExpr` head (§10): a type path, optional
    /// `<…>` generics and `[]` suffix, then `from`. Lookahead only — it consumes
    /// nothing. Distinguishes `T from …` and `Result<A,B> from …` from a value
    /// path that merely starts the same way (a comparison `a < b`, a bare ref). A
    /// non-type token inside the angles, or no trailing `from`, means it is not a
    /// `from` head.
    fn at_from_head(&self) -> bool {
        if !self.at(TokenKind::Ident) {
            return false;
        }
        // Skip the `::`-path: Ident { "::" Ident }.
        let mut i = self.pos + 1;
        while self.tokens.get(i).map(|t| t.kind) == Some(TokenKind::ColonColon) {
            if self.tokens.get(i + 1).map(|t| t.kind) != Some(TokenKind::Ident) {
                return false;
            }
            i += 2;
        }
        // Optional `<…>` generics: scan to the balanced close.
        if self.tokens.get(i).map(|t| t.kind) == Some(TokenKind::LAngle) {
            let mut depth = 1u32;
            i += 1;
            loop {
                match self.tokens.get(i).map(|t| t.kind) {
                    Some(TokenKind::LAngle) => depth += 1,
                    Some(TokenKind::RAngle) => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    // Tokens that may appear inside a type-argument list.
                    Some(
                        TokenKind::Ident
                        | TokenKind::ColonColon
                        | TokenKind::Comma
                        | TokenKind::LBracket
                        | TokenKind::RBracket,
                    ) => {}
                    // Anything else (operator, literal, …) ⇒ not generics.
                    _ => return false,
                }
                i += 1;
            }
        }
        // Optional `[]` array suffix.
        if self.tokens.get(i).map(|t| t.kind) == Some(TokenKind::LBracket)
            && self.tokens.get(i + 1).map(|t| t.kind) == Some(TokenKind::RBracket)
        {
            i += 2;
        }
        self.tokens.get(i).map(|t| t.kind) == Some(TokenKind::KwFrom)
    }

    fn parse_expr(&mut self) -> Expr {
        // §10 `Expr = Marker | FromExpr | OrExpr`: `Marker` and a `from` head do
        // not combine with binary operators (§7.5). Both are detected before the
        // precedence cascade so neither can become a binary operand.
        if matches!(
            self.peek_kind(),
            Some(TokenKind::KwOk | TokenKind::KwErr | TokenKind::KwSome | TokenKind::KwNone)
        ) {
            let marker = self.parse_marker();
            let expr = self.parse_postfix(marker);
            self.reject_operator_after_head();
            return expr;
        }
        if self.at_from_head() {
            let ty = self.parse_type();
            let from = self.parse_from(ty);
            let expr = self.parse_postfix(from);
            self.reject_operator_after_head();
            return expr;
        }
        self.parse_or()
    }

    /// §7.5: a `Marker` or `from` head does not combine with a binary operator.
    /// If one follows the head, report it (the operator is left unconsumed for
    /// the enclosing context to resync on).
    fn reject_operator_after_head(&mut self) {
        if matches!(
            self.peek_kind(),
            Some(
                TokenKind::PipePipe
                    | TokenKind::AmpAmp
                    | TokenKind::EqEq
                    | TokenKind::BangEq
                    | TokenKind::LAngle
                    | TokenKind::RAngle
                    | TokenKind::LAngleEq
                    | TokenKind::RAngleEq
                    | TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Star
                    | TokenKind::Slash
                    | TokenKind::Percent
            )
        ) {
            self.error(
                self.cur_span(),
                "a `from` or marker expression is not an operand of a binary operator (§7.5)",
            );
        }
    }

    /// Builds one left-associative binary level: `next { op next }` (§7.5).
    fn parse_binary_level(
        &mut self,
        ops: &[(TokenKind, BinOp)],
        next: fn(&mut Self) -> Expr,
    ) -> Expr {
        let mut left = next(self);
        while let Some(&(_, op)) = self
            .peek_kind()
            .and_then(|k| ops.iter().find(|(tk, _)| *tk == k))
        {
            let op_span = self.bump().expect("peeked operator").span;
            let right = next(self);
            let span = left.span.to(right.span);
            left = Expr {
                kind: ExprKind::Binary {
                    left: Box::new(left),
                    op,
                    op_span,
                    right: Box::new(right),
                },
                span,
            };
        }
        left
    }

    fn parse_or(&mut self) -> Expr {
        self.parse_binary_level(&[(TokenKind::PipePipe, BinOp::Or)], Self::parse_and)
    }

    fn parse_and(&mut self) -> Expr {
        self.parse_binary_level(&[(TokenKind::AmpAmp, BinOp::And)], Self::parse_eq)
    }

    fn parse_eq(&mut self) -> Expr {
        self.parse_binary_level(
            &[(TokenKind::EqEq, BinOp::Eq), (TokenKind::BangEq, BinOp::Ne)],
            Self::parse_rel,
        )
    }

    fn parse_rel(&mut self) -> Expr {
        self.parse_binary_level(
            &[
                (TokenKind::LAngle, BinOp::Lt),
                (TokenKind::RAngle, BinOp::Gt),
                (TokenKind::LAngleEq, BinOp::Le),
                (TokenKind::RAngleEq, BinOp::Ge),
            ],
            Self::parse_add,
        )
    }

    fn parse_add(&mut self) -> Expr {
        self.parse_binary_level(
            &[
                (TokenKind::Plus, BinOp::Add),
                (TokenKind::Minus, BinOp::Sub),
            ],
            Self::parse_mul,
        )
    }

    fn parse_mul(&mut self) -> Expr {
        self.parse_binary_level(
            &[
                (TokenKind::Star, BinOp::Mul),
                (TokenKind::Slash, BinOp::Div),
                (TokenKind::Percent, BinOp::Rem),
            ],
            Self::parse_unary,
        )
    }

    /// Parses a prefix unary (`!` / `-`) or, failing that, a postfix chain over a
    /// primary (§7.5).
    fn parse_unary(&mut self) -> Expr {
        let op = match self.peek_kind() {
            Some(TokenKind::Bang) => Some(UnaryOp::Not),
            Some(TokenKind::Minus) => Some(UnaryOp::Neg),
            _ => None,
        };
        if let Some(op) = op {
            let token = self.bump().expect("peeked unary operator");
            let expr = self.parse_unary();
            let span = token.span.to(expr.span);
            return Expr {
                kind: ExprKind::Unary {
                    op,
                    op_span: token.span,
                    expr: Box::new(expr),
                },
                span,
            };
        }
        let base = self.parse_primary();
        self.parse_postfix(base)
    }

    fn parse_primary(&mut self) -> Expr {
        match self.peek_kind() {
            Some(
                TokenKind::String | TokenKind::Number | TokenKind::KwTrue | TokenKind::KwFalse,
            ) => {
                let lit = self.parse_literal().expect("peeked literal");
                Expr {
                    span: lit.span(),
                    kind: ExprKind::Literal(lit),
                }
            }
            Some(TokenKind::KwSelf) => {
                // `self.` is removed (ADR-041); a same-node call is bare
                // `Name(args)`. Recover `self.Name(args)` as that bare call so
                // only this diagnostic surfaces and the body still resolves.
                let token = self.bump().expect("peeked self");
                if self.eat(TokenKind::Dot).is_some() {
                    let name = self.expect_ident("callable name");
                    let (args, end) = if self.at(TokenKind::LParen) {
                        self.parse_call_args()
                    } else {
                        (Vec::new(), name.span.end)
                    };
                    let span = Span::new(token.span.start, end);
                    self.error(
                        span,
                        "`self.` is removed; call `Name(args)` directly (ADR-041)",
                    );
                    Expr {
                        kind: ExprKind::OwnCall { name, args },
                        span,
                    }
                } else {
                    self.error(
                        token.span,
                        "`self` is removed; call a same-node callable as `Name(args)` (ADR-041)",
                    );
                    Expr {
                        kind: ExprKind::Ref(Ref::Path(Path {
                            segments: Vec::new(),
                            span: token.span,
                        })),
                        span: token.span,
                    }
                }
            }
            Some(TokenKind::LParen) => {
                let open = self.bump().expect("peeked `(`");
                let inner = self.parse_expr();
                let end = self
                    .eat(TokenKind::RParen)
                    .map_or(inner.span.end, |t| t.span.end);
                Expr {
                    kind: ExprKind::Paren(Box::new(inner)),
                    span: Span::new(open.span.start, end),
                }
            }
            Some(TokenKind::Ident) => {
                // A `from` head (`Type from …`) is detected and parsed by
                // `parse_expr` before the cascade (§10), so a path here is a
                // plain value reference; a following `<` is the less-than
                // operator (§7.5), left for the precedence cascade.
                let path = self.parse_path();
                // A single-segment path immediately followed by `(` is a bare
                // same-node call `Name(args)` (§5.1, ADR-041); a longer path or
                // no `(` is a value reference, any `.method(…)` left to postfix.
                if path.segments.len() == 1 && self.at(TokenKind::LParen) {
                    let start = path.span.start;
                    let name = path.segments.into_iter().next().expect("len == 1");
                    let (args, end) = self.parse_call_args();
                    Expr {
                        span: Span::new(start, end),
                        kind: ExprKind::OwnCall { name, args },
                    }
                } else {
                    Expr {
                        span: path.span,
                        kind: ExprKind::Ref(Ref::Path(path)),
                    }
                }
            }
            _ => {
                // Not an expression: synthesise an empty path ref so callers do
                // not have to special-case `None`, and record nothing extra.
                let span = self.cur_span();
                self.error(span, "expected an expression");
                Expr {
                    kind: ExprKind::Ref(Ref::Path(Path {
                        segments: Vec::new(),
                        span,
                    })),
                    span,
                }
            }
        }
    }

    fn parse_marker(&mut self) -> Expr {
        let token = self.bump().expect("peeked marker keyword");
        // The caller dispatches only on the four marker keywords.
        let kind = match token.kind {
            TokenKind::KwOk => MarkerKind::Ok,
            TokenKind::KwErr => MarkerKind::Err,
            TokenKind::KwSome => MarkerKind::Some,
            TokenKind::KwNone => MarkerKind::None,
            other => unreachable!("parse_marker dispatched on a non-marker token: {other:?}"),
        };
        let mut end = token.span.end;
        let payload = if self.at(TokenKind::LParen) {
            self.bump();
            let inner = self.parse_expr();
            end = self
                .eat(TokenKind::RParen)
                .map_or(inner.span.end, |t| t.span.end);
            Some(Box::new(inner))
        } else {
            None
        };
        Expr {
            kind: ExprKind::Marker { kind, payload },
            span: Span::new(token.span.start, end),
        }
    }

    fn parse_from(&mut self, ty: Type) -> Expr {
        let start = ty.span.start;
        if self.eat(TokenKind::KwFrom).is_none() {
            self.error(self.cur_span(), "expected `from` after the target type");
        }
        // `{` opens a source set (composition); anything else is a single-value
        // conversion (§7.2, §7.4, ADR-035).
        let source = if self.at(TokenKind::LBrace) {
            FromSource::Compose(self.parse_from_sources())
        } else {
            FromSource::Convert(Box::new(self.parse_expr()))
        };
        Expr {
            span: Span::new(start, self.prev_end()),
            kind: ExprKind::From { ty, source },
        }
    }

    /// Parses a `from` brace source set `{ a, b }` (§7.2): comma-separated bare
    /// expressions, possibly empty.
    fn parse_from_sources(&mut self) -> Vec<Expr> {
        self.eat(TokenKind::LBrace);
        let mut sources = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::RBrace) {
            sources.push(self.parse_expr());
            if self.eat(TokenKind::Comma).is_some() {
                continue;
            }
            if !self.at(TokenKind::RBrace) {
                // A source set separates entries with `,` and closes with `}`
                // (§7.2). A `key: value` colon is the common mistake: `from`
                // takes bare references, not labelled fields (ADR-003), so the
                // value is bound to a name and the name listed. Point at the fix
                // and drop the mis-parsed label so it isn't later reported as an
                // unresolved reference. Either way, skip to the closing `}` to
                // keep the error local instead of derailing the body.
                if self.at(TokenKind::Colon) {
                    sources.pop();
                    self.error(
                        self.cur_span(),
                        "`from` takes bare references, not `field: value` — \
                         bind the value to a name and list the name (§7.2)",
                    );
                } else {
                    self.error(self.cur_span(), "expected `,` or `}` in `from` source set");
                }
                // Skip to this source set's own `}`, counting nested braces so a
                // labelled *inner* `from { … }` doesn't swallow the outer close.
                let mut depth: u32 = 0;
                while !self.is_eof() {
                    match self.peek_kind() {
                        Some(TokenKind::LBrace) => depth += 1,
                        Some(TokenKind::RBrace) if depth == 0 => break,
                        Some(TokenKind::RBrace) => depth -= 1,
                        _ => {}
                    }
                    self.bump();
                }
            }
            break;
        }
        self.eat(TokenKind::RBrace);
        sources
    }

    /// Parses chained `.name` / `.name(args)` segments onto `base` (ADR-007).
    fn parse_postfix(&mut self, base: Expr) -> Expr {
        if !self.at(TokenKind::Dot) {
            return base;
        }
        let start = base.span.start;
        let mut segments = Vec::new();
        while self.at(TokenKind::Dot) {
            let dot = self.bump().expect("peeked `.`");
            let name = self.expect_ident("member name");
            let (call_args, end) = if self.at(TokenKind::LParen) {
                let (args, end) = self.parse_call_args();
                (Some(args), end)
            } else {
                (None, name.span.end)
            };
            segments.push(PostfixSeg {
                span: Span::new(dot.span.start, end),
                name,
                call_args,
            });
        }
        let end = segments.last().map_or(base.span.end, |s| s.span.end);
        Expr {
            kind: ExprKind::Postfix {
                base: Box::new(base),
                segments,
            },
            span: Span::new(start, end),
        }
    }

    /// Parses a call argument list `"(" [ Expr { "," Expr } ] ")"`, the current
    /// token being the open paren. Returns the arguments and the end position
    /// (the open paren's end when the list is unterminated).
    fn parse_call_args(&mut self) -> (Vec<Expr>, u32) {
        let open = self.bump().expect("peeked `(`");
        let mut args = Vec::new();
        while !self.is_eof() && !self.at(TokenKind::RParen) {
            args.push(self.parse_expr());
            if self.eat(TokenKind::Comma).is_none() {
                break;
            }
        }
        let end = self
            .eat(TokenKind::RParen)
            .map_or(open.span.end, |t| t.span.end);
        (args, end)
    }

    fn parse_literal(&mut self) -> Option<Literal> {
        match self.peek_kind()? {
            TokenKind::String => {
                let token = self.bump().expect("peeked string");
                Some(Literal::String {
                    raw: token.text,
                    span: token.span,
                })
            }
            TokenKind::Number => {
                let token = self.bump().expect("peeked number");
                Some(Literal::Number {
                    raw: token.text,
                    span: token.span,
                })
            }
            TokenKind::KwTrue => {
                let token = self.bump().expect("peeked true");
                Some(Literal::Bool {
                    value: true,
                    span: token.span,
                })
            }
            TokenKind::KwFalse => {
                let token = self.bump().expect("peeked false");
                Some(Literal::Bool {
                    value: false,
                    span: token.span,
                })
            }
            _ => None,
        }
    }

    // --- paths & idents -----------------------------------------------------

    fn parse_path(&mut self) -> Path {
        let mut segments = Vec::new();
        let start = self.cur_span().start;
        if self.at(TokenKind::Ident) {
            segments.push(self.expect_ident("path segment"));
            while self.at(TokenKind::ColonColon) {
                self.bump();
                segments.push(self.expect_ident("path segment"));
            }
        } else {
            self.error(self.cur_span(), "expected a path");
        }
        let end = segments.last().map_or(start, |s| s.span.end);
        Path {
            segments,
            span: Span::new(start, end),
        }
    }

    fn expect_ident(&mut self, what: &str) -> Ident {
        if let Some(token) = self.eat(TokenKind::Ident) {
            Ident {
                name: token.text,
                span: token.span,
            }
        } else {
            let span = self.cur_span();
            self.error(span, format!("expected {what}"));
            Ident {
                name: String::new(),
                span,
            }
        }
    }

    /// Whether the current token can serve as a declared name. In addition to a
    /// plain `IDENT`, a reserved keyword token is accepted here: `LANG.md` §2.3
    /// makes reserved-word misuse a *static* error (enforced by the model
    /// crate), not a parse error, and the worked example uses `Container` /
    /// `Component` as `data` union variant names.
    fn at_name(&self) -> bool {
        matches!(self.peek_kind(), Some(k) if k == TokenKind::Ident || Self::is_keyword(k))
    }

    fn is_keyword(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::KwSystem
                | TokenKind::KwContainer
                | TokenKind::KwComponent
                | TokenKind::KwPerson
                | TokenKind::KwData
                | TokenKind::KwFor
                | TokenKind::KwFrom
                | TokenKind::KwPublic
                | TokenKind::KwSelf
                | TokenKind::KwReturn
                | TokenKind::KwOk
                | TokenKind::KwErr
                | TokenKind::KwSome
                | TokenKind::KwNone
                | TokenKind::KwIf
                | TokenKind::KwElse
                | TokenKind::KwWhile
                | TokenKind::KwIn
                | TokenKind::KwTrue
                | TokenKind::KwFalse
                | TokenKind::KwFeature
                | TokenKind::KwGiven
                | TokenKind::KwWhen
                | TokenKind::KwThen
                | TokenKind::KwAnd
                | TokenKind::KwBut
        )
    }

    /// Consumes a name token (an `IDENT` or, leniently, a reserved keyword used
    /// in name position — see [`Self::at_name`]).
    fn expect_name(&mut self, what: &str) -> Ident {
        if self.at_name() {
            let token = self.bump().expect("peeked name token");
            Ident {
                name: token.text,
                span: token.span,
            }
        } else {
            let span = self.cur_span();
            self.error(span, format!("expected {what}"));
            Ident {
                name: String::new(),
                span,
            }
        }
    }

    // --- recovery -----------------------------------------------------------

    /// Skips tokens until the next top-level declaration boundary.
    fn recover_to_item(&mut self) {
        while let Some(kind) = self.peek_kind() {
            if matches!(
                kind,
                TokenKind::KwPublic
                    | TokenKind::KwPerson
                    | TokenKind::KwSystem
                    | TokenKind::KwContainer
                    | TokenKind::KwComponent
                    | TokenKind::KwData
                    | TokenKind::KwConstant
                    | TokenKind::KwFeature
                    | TokenKind::Doc
                    | TokenKind::HashLBracket
            ) {
                break;
            }
            self.bump();
        }
    }

    /// Skips tokens to the next `;`, `}`, or statement/member start, consuming a
    /// trailing `;` so the enclosing loop makes progress.
    fn recover_in_block(&mut self) {
        while let Some(kind) = self.peek_kind() {
            match kind {
                TokenKind::Semi => {
                    self.bump();
                    return;
                }
                TokenKind::RBrace => return,
                _ => {
                    self.bump();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::diagnostic::Diagnostic;

    /// Regression: a stray `;` in statement position (§10 has no statement
    /// terminator) must not spin the block loop. Before the forward-progress
    /// guard, `parse_stmt` returned `Some` without consuming the `;`, so
    /// `parse_block` looped forever, growing `stmts` until the process ran out
    /// of memory. Parsing must terminate and report the stray token.
    #[test]
    fn stray_semicolon_in_block_terminates() {
        let parsed = parse("public system S { f() { self.g(); } }");
        assert!(
            parsed.diagnostics.iter().any(Diagnostic::is_error),
            "a stray `;` is reported"
        );
        // The callable still parses; the `self.g()` call survives recovery.
        assert_eq!(parsed.ast.items.len(), 1);
    }

    /// A token that starts neither a callable nor a nested declaration must not
    /// spin the body-member loop either.
    #[test]
    fn stray_token_in_body_terminates() {
        let parsed = parse("public system S { = }");
        assert!(parsed.diagnostics.iter().any(Diagnostic::is_error));
    }

    /// Regression: top-level recovery stops at a `constant` declaration
    /// (ADR-039), so a stray token never swallows the constant after it.
    #[test]
    fn recovery_stops_at_constant() {
        let parsed = parse("}\nconstant LIMIT = 10\n");
        assert!(
            parsed.diagnostics.iter().any(Diagnostic::is_error),
            "the stray token is reported"
        );
        assert_eq!(
            parsed.ast.items.len(),
            1,
            "the constant survives recovery: {:?}",
            parsed.ast.items
        );
    }

    /// Regression: a labelled `from` source set (`{ name: "x" }` — invalid, since
    /// `from` takes bare references, ADR-003) reports one fix-oriented error and
    /// recovers at its `}`. Before the fix, the stray `:` derailed the body into a
    /// cascade of spurious "unexpected token at top level" errors, and the
    /// callable after it was lost. The mis-parsed label is dropped, so it is not
    /// also flagged as an unresolved reference.
    #[test]
    fn labelled_from_source_does_not_cascade() {
        let parsed = parse(
            "public system S { f(): T { return T from { name: \"x\" } } g(): T { return Ok } }",
        );
        let errors: Vec<&str> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            errors
                .iter()
                .any(|m| m.contains("`from` takes bare references")),
            "the source set error names the fix: {errors:?}"
        );
        assert!(
            !errors.iter().any(|m| m.contains("top level")),
            "no top-level cascade: {errors:?}"
        );
        // The label `name` is dropped, not re-reported as a parse error.
        assert_eq!(errors.len(), 1, "exactly one error: {errors:?}");
        // Both callables survive recovery — the container parses as one item.
        assert_eq!(parsed.ast.items.len(), 1);
    }

    /// A labelled *inner* `from` must not let recovery swallow the outer `}`:
    /// brace-aware skipping stops at the source set's own close, so the trailing
    /// statements still parse.
    #[test]
    fn labelled_nested_from_recovers_to_matching_brace() {
        let parsed =
            parse("public system S { f(a: T): T { x = T from { m: a from { n } } return Ok } }");
        let errors: Vec<&str> = parsed
            .diagnostics
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.message.as_str())
            .collect();
        assert!(
            !errors.iter().any(|m| m.contains("top level")),
            "no top-level cascade from a nested labelled from: {errors:?}"
        );
        assert_eq!(parsed.ast.items.len(), 1);
    }
}
