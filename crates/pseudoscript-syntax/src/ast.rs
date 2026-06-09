//! The typed syntax tree produced by [`crate::parse`] (`LANG.md` §10).
//!
//! Every node carries a [`Span`]. Declarations and statements also carry
//! `leading_trivia` (comments and blank-line gaps that preceded them) so the
//! formatter can reproduce layout; doc comments, tags, macros, and modifiers
//! are first-class fields, not trivia.

use crate::lexer::SpannedTrivia;
use crate::span::Span;

/// A whole parsed file: module-level inner docs followed by declarations, in
/// source order.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// `//!` inner-doc lines documenting this module (§2.1).
    pub inner_docs: Vec<InnerDoc>,
    /// Top-level items: declarations and features, in source order.
    pub items: Vec<Item>,
    /// Source span of the whole module.
    pub span: Span,
}

/// One `//!` inner-doc line.
#[derive(Debug, Clone, PartialEq)]
pub struct InnerDoc {
    /// Doc text with the `//!` marker and surrounding whitespace stripped.
    pub text: String,
    /// Source span of the line (including the marker).
    pub span: Span,
}

/// A top-level item.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    /// A documented, annotated structural declaration (§4, §3.4).
    Decl(Decl),
    /// A `feature` BDD scenario (§5.2).
    Feature(Feature),
}

impl Item {
    /// Source span of the item.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Item::Decl(d) => d.span,
            Item::Feature(f) => f.span,
        }
    }
}

/// A `feature Name for Path { given* when+ then+ }` BDD scenario (§5.2).
///
/// The scenario documents one behavioral flow of the node named by `target`.
/// Steps are prose (string literals), not resolved against the model; the
/// strict given→when→then ordering is enforced by the parser.
#[derive(Debug, Clone, PartialEq)]
pub struct Feature {
    /// The `///` doc block (summary + tags).
    pub doc: DocBlock,
    /// The scenario's name.
    pub name: Ident,
    /// The target node path (`for <Path>`), resolved as an FQN (§8).
    pub target: Path,
    /// The ordered steps, in source order.
    pub steps: Vec<FeatureStep>,
    /// Comments / blank lines preceding this feature.
    pub leading_trivia: Vec<SpannedTrivia>,
    /// Source span covering docs and the whole feature.
    pub span: Span,
}

/// One step line in a feature flow: a step keyword and its prose (§5.2).
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureStep {
    /// Which step keyword introduced it.
    pub kind: StepKind,
    /// The step's prose, a string literal (`raw` includes the quotes).
    pub text: Literal,
    /// Source span of the step (keyword through string).
    pub span: Span,
}

/// The step keyword class of a [`FeatureStep`] (§5.2). `And`/`But` continue the
/// preceding step's flow phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// `given` — a precondition.
    Given,
    /// `when` — the action.
    When,
    /// `then` — the assertion.
    Then,
    /// `and` — continues the preceding step's kind.
    And,
    /// `but` — continues the preceding step's kind.
    But,
}

impl StepKind {
    /// The lowercase keyword spelling (`given`, `when`, …).
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            StepKind::Given => "given",
            StepKind::When => "when",
            StepKind::Then => "then",
            StepKind::And => "and",
            StepKind::But => "but",
        }
    }

    /// The step kind a step keyword token introduces, or `None` for a non-step
    /// token.
    #[must_use]
    pub fn from_token(kind: crate::token::TokenKind) -> Option<StepKind> {
        use crate::token::TokenKind;
        match kind {
            TokenKind::KwGiven => Some(StepKind::Given),
            TokenKind::KwWhen => Some(StepKind::When),
            TokenKind::KwThen => Some(StepKind::Then),
            TokenKind::KwAnd => Some(StepKind::And),
            TokenKind::KwBut => Some(StepKind::But),
            _ => None,
        }
    }
}

/// The doc block, macros, and modifiers shared by every declaration, plus the
/// structural payload itself (§2.1 declaration order: docs → macros → modifiers
/// → declaration).
#[derive(Debug, Clone, PartialEq)]
pub struct Decl {
    /// The `///` doc block (summary + extended, split per ADR-009).
    pub doc: DocBlock,
    /// Stacked `#[..]` macros (§2.4).
    pub macros: Vec<Macro>,
    /// Whether `public` precedes the construct keyword (§4.1).
    pub is_public: bool,
    /// The structural construct.
    pub kind: DeclKind,
    /// Comments / blank lines preceding this declaration.
    pub leading_trivia: Vec<SpannedTrivia>,
    /// Source span covering docs, macros, modifiers, and the construct.
    pub span: Span,
}

/// The structural payload of a [`Decl`].
#[derive(Debug, Clone, PartialEq)]
pub enum DeclKind {
    /// `person Name` (block or `;`).
    Person(Node),
    /// `system Name` (block or `;`).
    System(Node),
    /// `container Name for Parent` (block or `;`).
    Container(Node),
    /// `component Name for Parent` (block or `;`).
    Component(Node),
    /// `data Name` — record, union, or black box (§3.4, §3.5).
    Data(Data),
    /// `constant NAME = Literal` — a top-level primitive constant (§3.6, ADR-039).
    Constant(Constant),
}

/// The keyword class of a structural node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// `person`.
    Person,
    /// `system`.
    System,
    /// `container`.
    Container,
    /// `component`.
    Component,
}

/// A `person` / `system` / `container` / `component` declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Which structural keyword introduced it.
    pub kind: NodeKind,
    /// The node's name.
    pub name: Ident,
    /// The `for <Parent>` path, present for `container`/`component` (§4).
    pub parent: Option<Path>,
    /// Disclosed body members, or `None` for a black box (`;`).
    ///
    /// A disclosed block holds callables and — as the §11 worked example shows
    /// — nested structural declarations (a `component` inside a `container`).
    pub body: Option<Vec<BodyMember>>,
    /// Source span of the node declaration.
    pub span: Span,
}

/// A member of a disclosed node body: a callable or a nested declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum BodyMember {
    /// A callable (implicit operation, §5.1).
    Callable(Callable),
    /// A nested structural declaration (e.g. a `component` inside a `container`).
    Decl(Decl),
}

impl BodyMember {
    /// Source span of the member.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            BodyMember::Callable(c) => c.span,
            BodyMember::Decl(d) => d.span,
        }
    }
}

/// A `constant NAME = Literal` declaration (§3.6, ADR-039). `public` lives on the
/// enclosing [`Decl`], mirroring [`Data`].
#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    /// The constant's name.
    pub name: Ident,
    /// The declared primitive literal value.
    pub value: Literal,
    /// Source span of the declaration.
    pub span: Span,
}

/// A `data` declaration: a record body, a union, or a black box (§3.4, §3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct Data {
    /// The data type's name.
    pub name: Ident,
    /// The data form.
    pub body: DataBody,
    /// Source span of the declaration.
    pub span: Span,
}

/// The three `data` forms.
#[derive(Debug, Clone, PartialEq)]
pub enum DataBody {
    /// `{ field: Type, ... }`.
    Record(Vec<Field>),
    /// `= | A | B { ... }` discriminated union (§3.5).
    Union(Vec<Variant>),
    /// `;` black box (shape undisclosed).
    BlackBox,
}

/// A record field `name: Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Field name.
    pub name: Ident,
    /// Field type.
    pub ty: Type,
    /// Source span of the field.
    pub span: Span,
}

/// One union variant.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Variant name.
    pub name: Ident,
    /// Inline record body if the variant declares one (`| Name { ... }`); a
    /// bare `| Name` reference has `None` (§3.5, ADR-006).
    pub record: Option<Vec<Field>>,
    /// Source span of the variant.
    pub span: Span,
}

/// A callable (implicit operation) declared inside a disclosed node (§5.1).
#[derive(Debug, Clone, PartialEq)]
pub struct Callable {
    /// The `///` doc block.
    pub doc: DocBlock,
    /// Stacked `#[..]` macros (trigger macros target callables).
    pub macros: Vec<Macro>,
    /// Whether the callable is `public` (§8.2).
    pub is_public: bool,
    /// The callable's name.
    pub name: Ident,
    /// Parameters.
    pub params: Vec<Param>,
    /// Declared return type; `None` means `void` (§5.1).
    pub return_ty: Option<Type>,
    /// Statement block, or `None` for a black box (`;`).
    pub body: Option<Block>,
    /// Comments / blank lines preceding this callable.
    pub leading_trivia: Vec<SpannedTrivia>,
    /// Source span of the callable.
    pub span: Span,
}

/// A callable parameter `name: Type`.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Parameter name.
    pub name: Ident,
    /// Parameter type.
    pub ty: Type,
    /// Source span.
    pub span: Span,
}

/// A `{ ... }` statement block (§7).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    /// Statements in order.
    pub stmts: Vec<Stmt>,
    /// Source span including the braces.
    pub span: Span,
}

/// A statement valid inside a callable body (§7).
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    /// The statement form.
    pub kind: StmtKind,
    /// Comments / blank lines preceding this statement.
    pub leading_trivia: Vec<SpannedTrivia>,
    /// Source span.
    pub span: Span,
}

/// The statement forms (§7).
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// `x = Expr` single-assignment (§7.1): a binding states its type through a
    /// `from` right-hand side (ADR-035).
    Assign { name: Ident, value: Expr },
    /// `return [Expr]` (§7).
    Return(Option<Expr>),
    /// `if (C) { } [else { }]` (§7).
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
    },
    /// `for (x in Expr) { }` (§7.3).
    For {
        binding: Ident,
        iter: Expr,
        body: Block,
    },
    /// `while (C) { }` (§7).
    While { cond: Expr, body: Block },
    /// A bare expression statement (a call or access chain, §7).
    Expr(Expr),
}

/// An expression (§7, §10).
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// The expression form.
    pub kind: ExprKind,
    /// Source span.
    pub span: Span,
}

/// The built-in generic constructors (§6, ADR-019): `Ok`/`Err` build a
/// `Result`, `Some`/`None` build an `Option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    /// `Ok` — the success branch of a `Result`.
    Ok,
    /// `Err` — the error branch of a `Result`.
    Err,
    /// `Some` — a present `Option`.
    Some,
    /// `None` — an empty `Option`.
    None,
}

impl MarkerKind {
    /// The keyword spelling (`Ok` / `Err` / `Some` / `None`).
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            MarkerKind::Ok => "Ok",
            MarkerKind::Err => "Err",
            MarkerKind::Some => "Some",
            MarkerKind::None => "None",
        }
    }
}

/// The expression forms (§10).
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A built-in generic constructor: `Ok`/`Err` (`Result`) or `Some`/`None`
    /// (`Option`), optionally wrapping a payload (§6, ADR-019).
    Marker {
        /// Which constructor this is.
        kind: MarkerKind,
        /// Optional `( Expr )` payload; `None` carries none.
        payload: Option<Box<Expr>>,
    },
    /// `Type from …` — carries a type onto a value (§7.2, ADR-035). The target
    /// `ty` may be any non-node type, including `Result<…>` generics and a `[]`
    /// array. The source is a brace set (composition) or a single value
    /// (conversion).
    From { ty: Type, source: FromSource },
    /// A postfix chain over a primary: `a.b.c`, `Repo.f(x).g()` (ADR-007).
    Postfix {
        base: Box<Expr>,
        segments: Vec<PostfixSeg>,
    },
    /// `self` or an FQN (§10 `Ref`).
    Ref(Ref),
    /// A string / number / bool literal (ADR-013).
    Literal(Literal),
    /// A unary operator applied to an operand: `! Expr` or `- Expr` (§7.5).
    Unary {
        /// Which unary operator.
        op: UnaryOp,
        /// Source span of the operator token.
        op_span: Span,
        /// The operand.
        expr: Box<Expr>,
    },
    /// A binary operator over two operands: `left op right` (§7.5).
    Binary {
        /// The left operand.
        left: Box<Expr>,
        /// Which binary operator.
        op: BinOp,
        /// Source span of the operator token(s).
        op_span: Span,
        /// The right operand.
        right: Box<Expr>,
    },
    /// A `( Expr )` group.
    Paren(Box<Expr>),
}

/// A unary operator (§7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// `!` — boolean negation.
    Not,
    /// `-` — numeric negation.
    Neg,
}

impl UnaryOp {
    /// The operator's source spelling.
    #[must_use]
    pub fn spelling(self) -> &'static str {
        match self {
            UnaryOp::Not => "!",
            UnaryOp::Neg => "-",
        }
    }
}

/// A binary operator (§7.5), grouped by the type rule it obeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `&&`
    And,
    /// `||`
    Or,
}

impl BinOp {
    /// The operator's source spelling.
    #[must_use]
    pub fn spelling(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Gt => ">",
            BinOp::Le => "<=",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

/// The source of a `from` expression (§7.2, ADR-035).
#[derive(Debug, Clone, PartialEq)]
pub enum FromSource {
    /// `from { a, b }` — composes a `data` record/variant from a source set.
    Compose(Vec<Expr>),
    /// `from expr` — carries the target type onto a single value.
    Convert(Box<Expr>),
}

impl FromSource {
    /// The source expressions, whichever form — for walkers that treat both
    /// uniformly.
    #[must_use]
    pub fn sources(&self) -> &[Expr] {
        match self {
            FromSource::Compose(sources) => sources,
            FromSource::Convert(expr) => std::slice::from_ref(expr),
        }
    }
}

/// One `.name` or `.name(args)` step in a postfix chain (ADR-007).
#[derive(Debug, Clone, PartialEq)]
pub struct PostfixSeg {
    /// The accessed/invoked member name.
    pub name: Ident,
    /// `Some(args)` if this step is a call `( ... )`; `None` for field access.
    pub call_args: Option<Vec<Expr>>,
    /// Source span of the segment (the `.name` plus any call).
    pub span: Span,
}

/// A reference primary (§10 `Ref`).
#[derive(Debug, Clone, PartialEq)]
pub enum Ref {
    /// `self` — the enclosing node (ADR-004).
    SelfNode(Span),
    /// An identifier or `::`-separated path (a node FQN).
    Path(Path),
}

/// A literal value (ADR-013).
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// A string literal; `raw` includes the surrounding quotes.
    String { raw: String, span: Span },
    /// A numeric literal; `raw` is the source digit run.
    Number { raw: String, span: Span },
    /// A boolean literal.
    Bool { value: bool, span: Span },
}

impl Literal {
    /// Source span of the literal.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Literal::String { span, .. }
            | Literal::Number { span, .. }
            | Literal::Bool { span, .. } => *span,
        }
    }
}

/// A type expression: a named type with optional generics and an optional `[]`
/// array suffix (§3.3, ADR-008 — no optionality marker).
#[derive(Debug, Clone, PartialEq)]
pub struct Type {
    /// The base named type (a path, possibly with `<..>` generic arguments).
    pub name: Path,
    /// Generic arguments inside `<..>`, if any (only `Result<T, E>` is built-in).
    pub generics: Vec<Type>,
    /// Whether a trailing `[]` makes it an array type.
    pub is_array: bool,
    /// Source span of the whole type.
    pub span: Span,
}

/// A `::`-separated path of identifiers (§2.2, §10 `Path`).
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    /// Path segments in order; never empty.
    pub segments: Vec<Ident>,
    /// Source span covering the whole path.
    pub span: Span,
}

impl Path {
    /// Whether the path is a single segment (a bare identifier).
    #[must_use]
    pub fn is_simple(&self) -> bool {
        self.segments.len() == 1
    }
}

/// An identifier with its source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// The identifier text.
    pub name: String,
    /// Source span.
    pub span: Span,
}

/// A `///` doc block split into summary and extended description (ADR-009).
///
/// A blank `///` line ends the summary; everything before it is `summary`,
/// everything after is `extended`. With no blank line the whole block is the
/// summary and `extended` is empty.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocBlock {
    /// Summary lines (compact-diagram text).
    pub summary: Vec<String>,
    /// Extended description lines (tooltip text).
    pub extended: Vec<String>,
    /// Tags (`#name`, including the `#`) gathered from the block (§2.4).
    pub tags: Vec<Tag>,
}

impl DocBlock {
    /// Whether the block carries no prose and no tags.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.summary.is_empty() && self.extended.is_empty() && self.tags.is_empty()
    }
}

/// A `#name` tag from a doc block (§2.4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The tag text, including the leading `#`.
    pub text: String,
    /// Source span (the `#` onward).
    pub span: Span,
}

/// A `#[..]` macro (outer attribute) on a declaration (§2.4).
#[derive(Debug, Clone, PartialEq)]
pub struct Macro {
    /// The macro's name path (e.g. `http`, `onevent`).
    pub name: Path,
    /// The macro's argument form.
    pub args: MacroArgs,
    /// Source span including `#[` and `]`.
    pub span: Span,
}

/// The three macro argument forms (§2.4).
#[derive(Debug, Clone, PartialEq)]
pub enum MacroArgs {
    /// `#[manual]` — word form, no arguments.
    Word,
    /// `#[onevent(Path)]` / `#[http("...")]` — list form.
    List(Vec<MacroArg>),
    /// `#[schedule = "..."]` — name = value form.
    NameValue(Literal),
}

/// One argument inside a macro's `( ... )` list (§10 `MetaArg`).
#[derive(Debug, Clone, PartialEq)]
pub enum MacroArg {
    /// A literal argument.
    Literal(Literal),
    /// A path argument (e.g. an event type).
    Path(Path),
    /// A nested macro-style meta item.
    Nested(Box<Macro>),
}
