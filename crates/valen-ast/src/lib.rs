//! Valen AST types. Shared between parser, HIR, and codegen.
//!
//! The AST is intentionally untyped (no type inference results here).
//! Name resolution and type checking happen in `valen-hir`.

use smol_str::SmolStr;

pub mod span;
pub mod token;

pub use span::{FileId, Span, Spanned};

/// Top-level item in a `.vln` file.
#[derive(Debug, Clone)]
pub enum Item {
    Package(PackageDecl),
    Import(ImportDecl),
    Fn(FnDecl),
    Class(ClassDecl),
    DataClass(DataClassDecl),
    Enum(EnumDecl),
    Trait(TraitDecl),
    Impl(ImplBlock),
    TypeAlias(TypeAliasDecl),
    NewType(NewTypeDecl),
    AnnotationClass(AnnotationClassDecl),
}

/// An annotation applied to a declaration (e.g. `@Foo(x = 1)`).
#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: SmolStr,
    pub args: Vec<AnnotationArg>,
    pub span: Span,
}

/// A single argument in an annotation application.
#[derive(Debug, Clone)]
pub struct AnnotationArg {
    /// `None` for single-parameter shorthand (e.g. `@Foo("bar")`).
    pub name: Option<SmolStr>,
    pub value: Literal,
    pub span: Span,
}

/// Annotation class declaration (e.g. `annotation class Foo(pub x: Int)`).
#[derive(Debug, Clone)]
pub struct AnnotationClassDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub annotations: Vec<Annotation>,
    pub params: Vec<AnnotationParam>,
    pub span: Span,
}

/// A parameter of an annotation class declaration.
#[derive(Debug, Clone)]
pub struct AnnotationParam {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub ty: Type,
    pub span: Span,
}

/// Package declaration at the top of a source file (e.g. `package foo.bar`).
#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub path: Vec<SmolStr>,
    pub span: Span,
}

/// Import declaration (e.g. `import java.util.List` or `import foo as bar`).
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: Vec<SmolStr>,
    pub alias: Option<SmolStr>,
    pub span: Span,
}

/// Function declaration, used for top-level functions and methods.
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    /// `None` for abstract/trait method signatures without a body.
    pub body: Option<Block>,
    pub is_open: bool,
    pub is_override: bool,
    pub is_abstract: bool,
    pub is_unsafe: bool,
    pub is_inline: bool,
    pub span: Span,
}

/// A function parameter with name, type, mutability, and optional default value.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub default: Option<Expr>,
    pub span: Span,
}

/// Class declaration with optional primary constructor and body members.
#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub kind: ClassKind,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    /// Primary constructor parameters.
    pub ctor_params: Vec<CtorParam>,
    pub supertypes: Vec<Type>,
    /// Traits to auto-derive: `derives(Eq, Hash)`.
    pub derives: Vec<SmolStr>,
    pub body: Vec<ClassMember>,
    pub span: Span,
}

/// Modifier determining inheritance behaviour of a class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    /// Cannot be subclassed (default).
    Final,
    /// Can be subclassed.
    Open,
    /// Must be subclassed; may contain abstract members.
    Abstract,
    /// Subclasses restricted to the same compilation unit.
    Sealed,
}

/// Parameter of a primary constructor; may also declare a field.
#[derive(Debug, Clone)]
pub struct CtorParam {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub default: Option<Expr>,
    pub span: Span,
}

/// A member inside a class body — either a field or a method.
#[derive(Debug, Clone)]
pub enum ClassMember {
    Field(FieldDecl),
    Method(FnDecl),
}

/// Field declaration inside a class body.
#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub init: Option<Expr>,
    pub span: Span,
}

/// Data class declaration — value type with auto-generated `equals`/`hashCode`/`toString`.
#[derive(Debug, Clone)]
pub struct DataClassDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub ctor_params: Vec<CtorParam>,
    pub supertypes: Vec<Type>,
    /// Traits to auto-derive: `derives(Eq, Hash)`.
    pub derives: Vec<SmolStr>,
    pub span: Span,
}

/// Enum (algebraic data type) declaration.
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    /// Traits to auto-derive: `derives(Eq, Hash)`.
    pub derives: Vec<SmolStr>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// A single variant of an enum declaration.
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: SmolStr,
    pub fields: EnumVariantFields,
    pub span: Span,
}

/// Describes the payload shape of an enum variant.
#[derive(Debug, Clone)]
pub enum EnumVariantFields {
    /// No payload: `Shape::Point`.
    Unit,
    /// Named fields: `Shape::Circle(r: Float)`.
    Named(Vec<EnumField>),
}

/// A named field inside an enum variant.
#[derive(Debug, Clone)]
pub struct EnumField {
    pub name: SmolStr,
    pub ty: Type,
    pub span: Span,
}

/// Trait declaration — defines an interface with methods and associated types.
#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub annotations: Vec<Annotation>,
    pub visibility: Visibility,
    pub is_sealed: bool,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub items: Vec<TraitItem>,
    pub span: Span,
}

/// An item inside a trait body.
#[derive(Debug, Clone)]
pub enum TraitItem {
    AssociatedType(AssocTypeDecl),
    Fn(FnDecl),
}

/// Associated type declaration inside a trait, with optional default.
#[derive(Debug, Clone)]
pub struct AssocTypeDecl {
    pub name: SmolStr,
    pub default: Option<Type>,
    pub span: Span,
}

/// `impl` block — inherent methods or trait implementation for a type.
#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    /// `None` = inherent impl, `Some` = trait impl.
    pub trait_ref: Option<Type>,
    pub target: Type,
    pub items: Vec<ImplItem>,
    pub span: Span,
}

/// An item inside an `impl` block.
#[derive(Debug, Clone)]
pub enum ImplItem {
    AssociatedType(AssocTypeDef),
    Fn(FnDecl),
}

/// Concrete associated type definition inside an `impl` block.
#[derive(Debug, Clone)]
pub struct AssocTypeDef {
    pub name: SmolStr,
    pub ty: Type,
    pub span: Span,
}

/// Type alias declaration (e.g. `type StringList = List<String>`).
#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub ty: Type,
    pub span: Span,
}

/// `newtype Name = InnerType;` — creates a distinct type wrapping another type.
#[derive(Debug, Clone)]
pub struct NewTypeDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub inner_ty: Type,
    pub span: Span,
}

/// Generic type parameter with optional variance annotation and trait bounds.
#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: SmolStr,
    pub variance: Variance,
    pub is_reified: bool,
    pub bounds: Vec<Type>,
    pub span: Span,
}

/// Variance annotation on a generic type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    Invariant,
    /// `in T`
    Contravariant,
    /// `out T`
    Covariant,
}

/// Access modifier for declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// Visible everywhere.
    Pub,
    /// Visible within the same package.
    Internal,
    /// Visible only within the enclosing scope.
    Private,
}

/// Type representation as produced by the parser (before name resolution).
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Named type path: `Int`, `List<String>`.
    Path(TypePath),
    /// `T?` — syntactic sugar for `Option<T>` (expanded by the parser).
    Nullable { inner: Box<Type>, span: Span },
    /// Function type: `fn(Int, Int) -> String`.
    Fn(FnType),
    /// Tuple type `(A, B, C)` — reserved, not used in MVP.
    Tuple(Vec<Type>, Span),
    /// `ref mut T` — mutable reference type.
    RefMut { inner: Box<Type>, span: Span },
    /// `Self::TypeName` — associated type access on Self.
    SelfAssoc { name: SmolStr, span: Span },
}

/// A dot-separated type path (e.g. `java.util.List<String>`).
#[derive(Debug, Clone, PartialEq)]
pub struct TypePath {
    pub segments: Vec<TypePathSegment>,
    pub span: Span,
}

/// One segment of a type path, optionally carrying generic arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct TypePathSegment {
    pub name: SmolStr,
    pub generics: Vec<Type>,
    pub span: Span,
}

/// Function type: `fn(A, B) -> C`.
#[derive(Debug, Clone, PartialEq)]
pub struct FnType {
    pub params: Vec<Type>,
    pub return_type: Box<Type>,
    pub span: Span,
}

/// A braced block of statements with an optional tail expression.
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// Trailing expression without `;` — used as the block's value.
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

/// A statement inside a block.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    /// `let Pattern = expr else { diverge };` — pattern-matching let that diverges on mismatch.
    LetElse(LetElseStmt),
    Expr(Expr),
    /// Expression statement terminated by `;` (value is discarded).
    ExprSemi(Expr),
}

/// `let` / `let mut` variable binding.
#[derive(Debug, Clone)]
pub struct LetStmt {
    pub mutable: bool,
    pub name: SmolStr,
    /// Explicit type annotation, if provided.
    pub ty: Option<Type>,
    pub init: Expr,
    pub span: Span,
}

/// `let Pattern = expr else { diverge };` — refutable pattern binding with diverging else block.
#[derive(Debug, Clone)]
pub struct LetElseStmt {
    /// The binding name extracted from the pattern (for simple cases like `Some(x)`).
    pub name: SmolStr,
    /// Explicit type annotation, if provided.
    pub ty: Option<Type>,
    /// The refutable pattern to match against.
    pub pattern: Pattern,
    /// The expression to match.
    pub expr: Expr,
    /// The else block, which must diverge (return/break/continue/panic).
    pub else_block: Block,
    pub span: Span,
}

/// Expression node — every expression variant in the Valen language.
///
/// Large variants containing `Vec` fields are boxed to reduce the overall enum
/// size (#066). This keeps the inline size closer to the smallest variants
/// (pointer-sized) and reduces memory pressure when many `Expr` nodes exist.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Path(Path),
    Call(Box<CallExpr>),
    MethodCall(Box<MethodCallExpr>),
    Field(FieldAccess),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
    If(Box<IfExpr>),
    Match(Box<MatchExpr>),
    Block(Block),
    Return(ReturnExpr),
    Break(BreakExpr),
    Continue(ContinueExpr),
    For(Box<ForExpr>),
    While(Box<WhileExpr>),
    Loop(LoopExpr),
    Lambda(Box<LambdaExpr>),
    Range(RangeExpr),
    Try(TryExpr),
    StringInterp(Box<StringInterpExpr>),
    Safe(SafeExpr),
    IfLet(Box<IfLetExpr>),
    WhileLet(Box<WhileLetExpr>),
    /// `.Variant` or `.Variant(args)` — enum variant shorthand with inferred enum type.
    VariantShorthand(VariantShorthandExpr),
    /// `lhs |> rhs` — pipeline operator, desugar to first-arg insertion.
    Pipeline(Box<PipelineExpr>),
    /// `[expr, expr, ...]` — list literal.
    ListLiteral(Box<ListLiteralExpr>),
    /// `#{key: value, ...}` — map literal.
    MapLiteral(Box<MapLiteralExpr>),
    /// `unsafe { block }` or `unsafe expr` — bypasses safety checks.
    Unsafe(UnsafeExpr),
    /// `expr as Type` — type cast expression.
    Cast(CastExpr),
    /// `*expr` — dereference a `ref mut` value.
    Deref(DerefExpr),
    /// `ref mut expr` — create a mutable reference.
    RefMutCreate(RefMutExpr),
}

/// Literal value (integer, float, string, etc.).
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Integer literal. Stored as `i64` so the AST can represent both `Int` and
    /// `Long` ranges without loss, but the language type is 32-bit (`Int`).
    /// Range validation (must fit `i32`) is enforced during type checking in HIR.
    Int(i64, Span),
    Long(i64, Span),
    Float(f32, Span),
    Double(f64, Span),
    Char(char, Span),
    String(SmolStr, Span),
    Bool(bool, Span),
    /// The unit literal `()`.
    Unit(Span),
    /// The `null` literal — only valid inside `unsafe` blocks.
    Null(Span),
}

impl Literal {
    /// Returns the source span of this literal.
    pub fn span(&self) -> Span {
        match self {
            Literal::Int(_, s)
            | Literal::Long(_, s)
            | Literal::Float(_, s)
            | Literal::Double(_, s)
            | Literal::Char(_, s)
            | Literal::String(_, s)
            | Literal::Bool(_, s)
            | Literal::Unit(s)
            | Literal::Null(s) => *s,
        }
    }
}

impl Expr {
    /// Returns the source span of this expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(l) => l.span(),
            Expr::Path(p) => p.span,
            Expr::Call(c) => c.span,
            Expr::MethodCall(m) => m.span,
            Expr::Field(f) => f.span,
            Expr::Binary(b) => b.span,
            Expr::Unary(u) => u.span,
            Expr::Assign(a) => a.span,
            Expr::If(i) => i.span,
            Expr::Match(m) => m.span,
            Expr::Block(b) => b.span,
            Expr::Return(r) => r.span,
            Expr::Break(b) => b.span,
            Expr::Continue(c) => c.span,
            Expr::For(f) => f.span,
            Expr::While(w) => w.span,
            Expr::Loop(l) => l.span,
            Expr::Lambda(l) => l.span,
            Expr::Range(r) => r.span,
            Expr::Try(t) => t.span,
            Expr::StringInterp(s) => s.span,
            Expr::Safe(s) => s.span,
            Expr::IfLet(i) => i.span,
            Expr::WhileLet(w) => w.span,
            Expr::VariantShorthand(v) => v.span,
            Expr::Pipeline(p) => p.span,
            Expr::ListLiteral(l) => l.span,
            Expr::MapLiteral(m) => m.span,
            Expr::Unsafe(u) => u.span,
            Expr::Cast(c) => c.span,
            Expr::Deref(d) => d.span,
            Expr::RefMutCreate(r) => r.span,
        }
    }
}

impl Type {
    /// Returns the source span of this type.
    pub fn span(&self) -> Span {
        match self {
            Type::Path(p) => p.span,
            Type::Nullable { span, .. } => *span,
            Type::Fn(f) => f.span,
            Type::Tuple(_, span) => *span,
            Type::RefMut { span, .. } => *span,
            Type::SelfAssoc { span, .. } => *span,
        }
    }
}

impl Pattern {
    /// Returns the source span of this pattern.
    pub fn span(&self) -> Span {
        match self {
            Pattern::Wildcard(s) => *s,
            Pattern::Literal(l) => l.span(),
            Pattern::Binding(b) => b.span,
            Pattern::Path(p) => p.span,
            Pattern::Struct(s) => s.span,
            Pattern::Tuple(_, s) => *s,
            Pattern::Range(r) => r.span,
            Pattern::Or(_, s) => *s,
            Pattern::At(a) => a.span,
            Pattern::VariantShorthand(v) => v.span,
        }
    }
}

/// A value-level path (e.g. `foo.bar`, `Shape::Circle`).
#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub span: Span,
}

/// One segment of a value-level path.
#[derive(Debug, Clone, PartialEq)]
pub struct PathSegment {
    pub name: SmolStr,
    /// `true` if preceded by `::` (variant access like `Shape::Circle`),
    /// `false` if preceded by `.` (package path like `java.util.List`).
    pub double_colon: bool,
    pub generics: Vec<Type>,
    pub span: Span,
}

/// Function or constructor call expression.
#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    /// Explicit generic type arguments: `ArrayList<String>()`.
    pub generics: Vec<Type>,
    pub args: Vec<CallArg>,
    pub span: Span,
}

/// A single argument in a function call, optionally named.
#[derive(Debug, Clone)]
pub struct CallArg {
    /// Named argument label: `greet(msg = "hi")`.
    pub name: Option<SmolStr>,
    pub value: Expr,
    pub span: Span,
}

/// Method call expression: `receiver.method(args)`.
#[derive(Debug, Clone)]
pub struct MethodCallExpr {
    pub receiver: Box<Expr>,
    pub method: SmolStr,
    pub generics: Vec<Type>,
    pub args: Vec<CallArg>,
    pub span: Span,
}

/// Field access expression: `receiver.field`.
#[derive(Debug, Clone)]
pub struct FieldAccess {
    pub receiver: Box<Expr>,
    pub field: SmolStr,
    pub span: Span,
}

/// Binary operation expression: `lhs op rhs`.
#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
    pub span: Span,
}

/// Binary operator kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    /// Reference equality (`===`).
    RefEq,
    /// Reference inequality (`!==`).
    RefNe,
}

/// Unary operation expression: `op expr`.
#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub expr: Box<Expr>,
    pub span: Span,
}

/// Unary operator kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    /// Arithmetic negation (`-`).
    Neg,
    /// Logical negation (`!`).
    Not,
}

/// Assignment expression: `target = value` or `target += value`.
#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub target: Box<Expr>,
    /// `None` = plain `=`, `Some(Add)` = `+=`, etc.
    pub op: Option<BinaryOp>,
    pub value: Box<Expr>,
    pub span: Span,
}

/// `if` / `if-else` expression.
#[derive(Debug, Clone)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then_branch: Block,
    pub else_branch: Option<Box<Expr>>,
    pub span: Span,
}

/// `match` expression with exhaustive pattern arms.
#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

/// A single arm of a `match` expression: `pattern [if guard] => body`.
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

/// Pattern used in `match` arms and `let` bindings.
#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    /// `_` — matches anything, binds nothing.
    Wildcard(Span),
    Literal(Literal),
    Binding(BindingPattern),
    Path(Path),
    Struct(StructPattern),
    Tuple(Vec<Pattern>, Span),
    Range(RangePattern),
    /// `p1 | p2` — or-pattern.
    Or(Vec<Pattern>, Span),
    /// `name @ pattern` — bind the matched value while destructuring.
    At(AtPattern),
    /// `.Variant` or `.Variant(fields)` — enum variant shorthand pattern.
    VariantShorthand(VariantShorthandPattern),
}

/// Variable binding pattern (e.g. `x` or `mut x`).
#[derive(Debug, Clone, PartialEq)]
pub struct BindingPattern {
    pub name: SmolStr,
    pub mutable: bool,
    pub span: Span,
}

/// Destructuring pattern for enum variants or data classes.
#[derive(Debug, Clone, PartialEq)]
pub struct StructPattern {
    pub path: Path,
    pub fields: Vec<StructPatternField>,
    /// `true` if `..` (rest) is present.
    pub rest: bool,
    pub span: Span,
}

/// A named field inside a struct/enum destructuring pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct StructPatternField {
    pub name: SmolStr,
    /// Sub-pattern; `None` means shorthand binding (field name = variable name).
    pub pattern: Option<Pattern>,
    pub span: Span,
}

/// Range pattern for matching ranges of values (e.g. `1..10`, `0..=255`).
#[derive(Debug, Clone, PartialEq)]
pub struct RangePattern {
    pub start: Option<Literal>,
    pub end: Option<Literal>,
    pub inclusive: bool,
    pub span: Span,
}

/// `name @ pattern` — binds the whole matched value while destructuring.
#[derive(Debug, Clone, PartialEq)]
pub struct AtPattern {
    pub name: SmolStr,
    pub pattern: Box<Pattern>,
    pub span: Span,
}

/// `.Variant` or `.Variant(args)` — enum variant shorthand expression.
#[derive(Debug, Clone)]
pub struct VariantShorthandExpr {
    pub variant_name: SmolStr,
    pub args: Vec<CallArg>,
    pub span: Span,
}

/// `lhs |> rhs` — pipeline expression.
#[derive(Debug, Clone)]
pub struct PipelineExpr {
    pub lhs: Expr,
    pub rhs: Expr,
    pub span: Span,
}

/// `[expr, expr, ...]` — list literal expression.
#[derive(Debug, Clone)]
pub struct ListLiteralExpr {
    pub elements: Vec<Expr>,
    pub span: Span,
}

/// `#{key: value, ...}` — map literal expression.
#[derive(Debug, Clone)]
pub struct MapLiteralExpr {
    pub entries: Vec<(Expr, Expr)>,
    pub span: Span,
}

/// `.Variant` or `.Variant(fields)` — enum variant shorthand pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct VariantShorthandPattern {
    pub variant_name: SmolStr,
    pub fields: Vec<StructPatternField>,
    pub rest: bool,
    pub span: Span,
}

/// `return` expression with optional value.
#[derive(Debug, Clone)]
pub struct ReturnExpr {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

/// `break` expression with optional value (for `loop` blocks).
#[derive(Debug, Clone)]
pub struct BreakExpr {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

/// `continue` expression — skips to the next loop iteration.
#[derive(Debug, Clone)]
pub struct ContinueExpr {
    pub span: Span,
}

/// `for var in iter { body }` loop expression.
#[derive(Debug, Clone)]
pub struct ForExpr {
    pub var: SmolStr,
    pub iter: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

/// `while cond { body }` loop expression.
#[derive(Debug, Clone)]
pub struct WhileExpr {
    pub cond: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

/// Range expression: `start..end` or `start..=end`.
#[derive(Debug, Clone)]
pub struct RangeExpr {
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub inclusive: bool,
    pub span: Span,
}

/// Infinite `loop { body }` expression — exits via `break`.
#[derive(Debug, Clone)]
pub struct LoopExpr {
    pub body: Block,
    pub span: Span,
}

/// Lambda (closure) expression: `|params| body`.
#[derive(Debug, Clone)]
pub struct LambdaExpr {
    pub params: Vec<LambdaParam>,
    pub return_type: Option<Type>,
    pub body: Box<Expr>,
    pub span: Span,
}

/// A parameter of a lambda expression, with optional type annotation.
#[derive(Debug, Clone)]
pub struct LambdaParam {
    pub name: SmolStr,
    pub ty: Option<Type>,
    pub span: Span,
}

/// `expr?` — Result/Option propagation
#[derive(Debug, Clone)]
pub struct TryExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// `f"Hello, {name}!"`
#[derive(Debug, Clone)]
pub struct StringInterpExpr {
    pub parts: Vec<StringInterpPart>,
    pub span: Span,
}

/// A fragment of a string interpolation — either literal text or an embedded expression.
#[derive(Debug, Clone)]
pub enum StringInterpPart {
    /// Literal text segment.
    Text(SmolStr),
    /// Interpolated expression: `{expr}`.
    Expr(Expr),
}

/// `safe { java_call() }` — wraps Java exceptions into a Result explicitly.
#[derive(Debug, Clone)]
pub struct SafeExpr {
    pub block: Block,
    pub span: Span,
}

/// `unsafe { block }` or `unsafe expr` — bypasses safety checks.
#[derive(Debug, Clone)]
pub struct UnsafeExpr {
    pub body: Box<Expr>,
    pub span: Span,
}

/// `expr as Type` — type cast expression.
#[derive(Debug, Clone)]
pub struct CastExpr {
    pub expr: Box<Expr>,
    pub target_ty: Type,
    pub span: Span,
}

/// `*expr` — dereference a `ref mut` value.
#[derive(Debug, Clone)]
pub struct DerefExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// `ref mut expr` — create a mutable reference.
#[derive(Debug, Clone)]
pub struct RefMutExpr {
    pub expr: Box<Expr>,
    pub span: Span,
}

/// `if let Pattern = expr { then } else { else }`.
#[derive(Debug, Clone)]
pub struct IfLetExpr {
    pub pattern: Pattern,
    pub expr: Box<Expr>,
    pub then_branch: Block,
    pub else_branch: Option<Box<Expr>>,
    pub span: Span,
}

/// `while let Pattern = expr { body }`.
#[derive(Debug, Clone)]
pub struct WhileLetExpr {
    pub pattern: Pattern,
    pub expr: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

// ---------------------------------------------------------------------------
// Display implementations for key AST types (#067)
// ---------------------------------------------------------------------------

impl std::fmt::Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Int(v, _) => write!(f, "{v}"),
            Literal::Long(v, _) => write!(f, "{v}L"),
            Literal::Float(v, _) => write!(f, "{v}f"),
            Literal::Double(v, _) => write!(f, "{v}"),
            Literal::Char(c, _) => write!(f, "'{c}'"),
            Literal::String(s, _) => write!(f, "\"{s}\""),
            Literal::Bool(b, _) => write!(f, "{b}"),
            Literal::Unit(_) => write!(f, "()"),
            Literal::Null(_) => write!(f, "null"),
        }
    }
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::RefEq => "===",
            BinaryOp::RefNe => "!==",
        };
        write!(f, "{s}")
    }
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
        }
    }
}

impl std::fmt::Display for Visibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Visibility::Pub => write!(f, "pub"),
            Visibility::Internal => write!(f, "internal"),
            Visibility::Private => write!(f, "private"),
        }
    }
}

impl std::fmt::Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Path(tp) => {
                for (i, seg) in tp.segments.iter().enumerate() {
                    if i > 0 {
                        write!(f, ".")?;
                    }
                    write!(f, "{}", seg.name)?;
                    if !seg.generics.is_empty() {
                        write!(f, "<")?;
                        for (j, g) in seg.generics.iter().enumerate() {
                            if j > 0 {
                                write!(f, ", ")?;
                            }
                            write!(f, "{g}")?;
                        }
                        write!(f, ">")?;
                    }
                }
                Ok(())
            }
            Type::Nullable { inner, .. } => write!(f, "{inner}?"),
            Type::Fn(ft) => {
                write!(f, "fn(")?;
                for (i, p) in ft.params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") -> {}", ft.return_type)
            }
            Type::Tuple(tys, _) => {
                write!(f, "(")?;
                for (i, t) in tys.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            Type::RefMut { inner, .. } => write!(f, "ref mut {inner}"),
            Type::SelfAssoc { name, .. } => write!(f, "Self::{name}"),
        }
    }
}
