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
    pub span: Span,
}

/// A function parameter with name, type, and mutability.
#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

/// Class declaration with optional primary constructor and body members.
#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub visibility: Visibility,
    pub kind: ClassKind,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    /// Primary constructor parameters.
    pub ctor_params: Vec<CtorParam>,
    pub supertypes: Vec<Type>,
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
    pub visibility: Visibility,
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
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
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub ctor_params: Vec<CtorParam>,
    pub span: Span,
}

/// Enum (algebraic data type) declaration.
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
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

/// Generic type parameter with optional variance annotation and trait bounds.
#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: SmolStr,
    pub variance: Variance,
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
#[derive(Debug, Clone)]
pub enum Type {
    /// Named type path: `Int`, `List<String>`.
    Path(TypePath),
    /// `T?` — syntactic sugar for `Option<T>` (expanded by the parser).
    Nullable { inner: Box<Type>, span: Span },
    /// Function type: `fn(Int, Int) -> String`.
    Fn(FnType),
    /// Tuple type `(A, B, C)` — reserved, not used in MVP.
    Tuple(Vec<Type>),
}

/// A dot-separated type path (e.g. `java.util.List<String>`).
#[derive(Debug, Clone)]
pub struct TypePath {
    pub segments: Vec<TypePathSegment>,
    pub span: Span,
}

/// One segment of a type path, optionally carrying generic arguments.
#[derive(Debug, Clone)]
pub struct TypePathSegment {
    pub name: SmolStr,
    pub generics: Vec<Type>,
    pub span: Span,
}

/// Function type: `fn(A, B) -> C`.
#[derive(Debug, Clone)]
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

/// Expression node — every expression variant in the Valen language.
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Path(Path),
    Call(CallExpr),
    MethodCall(Box<MethodCallExpr>),
    Field(FieldAccess),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
    If(Box<IfExpr>),
    Match(MatchExpr),
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
    StringInterp(StringInterpExpr),
    Safe(SafeExpr),
}

/// Literal value (integer, float, string, etc.).
#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64, Span),
    Long(i64, Span),
    Float(f32, Span),
    Double(f64, Span),
    Char(char, Span),
    String(SmolStr, Span),
    Bool(bool, Span),
    /// The unit literal `()`.
    Unit(Span),
}

/// A value-level path (e.g. `foo.bar`, `Shape::Circle`).
#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub span: Span,
}

/// One segment of a value-level path.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
}

/// Variable binding pattern (e.g. `x` or `mut x`).
#[derive(Debug, Clone)]
pub struct BindingPattern {
    pub name: SmolStr,
    pub mutable: bool,
    pub span: Span,
}

/// Destructuring pattern for enum variants or data classes.
#[derive(Debug, Clone)]
pub struct StructPattern {
    pub path: Path,
    pub fields: Vec<StructPatternField>,
    /// `true` if `..` (rest) is present.
    pub rest: bool,
    pub span: Span,
}

/// A named field inside a struct/enum destructuring pattern.
#[derive(Debug, Clone)]
pub struct StructPatternField {
    pub name: SmolStr,
    /// Sub-pattern; `None` means shorthand binding (field name = variable name).
    pub pattern: Option<Pattern>,
    pub span: Span,
}

/// Range pattern for matching ranges of values (e.g. `1..10`, `0..=255`).
#[derive(Debug, Clone)]
pub struct RangePattern {
    pub start: Option<Literal>,
    pub end: Option<Literal>,
    pub inclusive: bool,
    pub span: Span,
}

/// `name @ pattern` — binds the whole matched value while destructuring.
#[derive(Debug, Clone)]
pub struct AtPattern {
    pub name: SmolStr,
    pub pattern: Box<Pattern>,
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
