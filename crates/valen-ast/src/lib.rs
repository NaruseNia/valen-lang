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

#[derive(Debug, Clone)]
pub struct PackageDecl {
    pub path: Vec<SmolStr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub path: Vec<SmolStr>,
    pub alias: Option<SmolStr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub visibility: Visibility,
    pub kind: ClassKind,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub ctor_params: Vec<CtorParam>,
    pub superclass: Option<Type>,
    pub traits: Vec<Type>,
    pub body: Vec<ClassMember>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Final,
    Open,
    Abstract,
    Sealed,
}

#[derive(Debug, Clone)]
pub struct CtorParam {
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ClassMember {
    Field(FieldDecl),
    Method(FnDecl),
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub ty: Type,
    pub mutable: bool,
    pub init: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DataClassDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub ctor_params: Vec<CtorParam>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: SmolStr,
    pub fields: EnumVariantFields,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum EnumVariantFields {
    /// Payload なし: `Shape::Point`
    Unit,
    /// Named fields: `Shape::Circle(r: Float)`
    Named(Vec<EnumField>),
}

#[derive(Debug, Clone)]
pub struct EnumField {
    pub name: SmolStr,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TraitDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub items: Vec<TraitItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum TraitItem {
    AssociatedType(AssocTypeDecl),
    Fn(FnDecl),
}

#[derive(Debug, Clone)]
pub struct AssocTypeDecl {
    pub name: SmolStr,
    pub default: Option<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub generics: Vec<GenericParam>,
    /// `None` = inherent impl、`Some` = trait impl
    pub trait_ref: Option<Type>,
    pub target: Type,
    pub items: Vec<ImplItem>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum ImplItem {
    AssociatedType(AssocTypeDef),
    Fn(FnDecl),
}

#[derive(Debug, Clone)]
pub struct AssocTypeDef {
    pub name: SmolStr,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub visibility: Visibility,
    pub name: SmolStr,
    pub generics: Vec<GenericParam>,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct GenericParam {
    pub name: SmolStr,
    pub variance: Variance,
    pub bounds: Vec<Type>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variance {
    Invariant,
    /// `in T`
    Contravariant,
    /// `out T`
    Covariant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Pub,
    Internal,
    Private,
}

/// 型表現（parser 出力時点、名前解決前）。
#[derive(Debug, Clone)]
pub enum Type {
    /// 単純な名前: `Int`, `User`
    Path(TypePath),
    /// `T?` → `Option<T>` の糖衣（parser で展開される）
    Nullable(Box<Type>),
    /// `fn(Int, Int) -> String`
    Fn(FnType),
    /// `(A, B, C)` は MVP では採用しない（予約）
    Tuple(Vec<Type>),
}

#[derive(Debug, Clone)]
pub struct TypePath {
    pub segments: Vec<TypePathSegment>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TypePathSegment {
    pub name: SmolStr,
    pub generics: Vec<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FnType {
    pub params: Vec<Type>,
    pub return_type: Box<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// ブロック末尾の `;` 無し式（値として返る）
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let(LetStmt),
    Expr(Expr),
    /// 末尾以外の式文（`;` あり）
    ExprSemi(Expr),
}

#[derive(Debug, Clone)]
pub struct LetStmt {
    pub mutable: bool,
    pub name: SmolStr,
    pub ty: Option<Type>,
    pub init: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Path(Path),
    Call(CallExpr),
    MethodCall(MethodCallExpr),
    Field(FieldAccess),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Assign(AssignExpr),
    If(IfExpr),
    Match(MatchExpr),
    Block(Block),
    Return(ReturnExpr),
    For(ForExpr),
    While(WhileExpr),
    Loop(LoopExpr),
    Lambda(LambdaExpr),
    Try(TryExpr),
    StringInterp(StringInterpExpr),
    Safe(SafeExpr),
}

#[derive(Debug, Clone)]
pub enum Literal {
    Int(i64, Span),
    Float(f64, Span),
    String(SmolStr, Span),
    Bool(bool, Span),
    Unit(Span),
}

#[derive(Debug, Clone)]
pub struct Path {
    pub segments: Vec<PathSegment>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: SmolStr,
    /// `::` の後なら true（Shape::Circle のような variant アクセス）、
    /// `.` の後なら false（java.util.List のような path）
    pub turbofish: bool,
    pub generics: Vec<Type>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallExpr {
    pub callee: Box<Expr>,
    pub args: Vec<CallArg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct CallArg {
    /// 名前付き引数: `greet(msg = "hi")`
    pub name: Option<SmolStr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MethodCallExpr {
    pub receiver: Box<Expr>,
    pub method: SmolStr,
    pub generics: Vec<Type>,
    pub args: Vec<CallArg>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct FieldAccess {
    pub receiver: Box<Expr>,
    pub field: SmolStr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct BinaryExpr {
    pub op: BinaryOp,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
    pub span: Span,
}

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
}

#[derive(Debug, Clone)]
pub struct UnaryExpr {
    pub op: UnaryOp,
    pub expr: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone)]
pub struct AssignExpr {
    pub target: Box<Expr>,
    pub value: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then_branch: Block,
    pub else_branch: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard(Span),
    Literal(Literal),
    Binding(BindingPattern),
    Path(Path),
    Struct(StructPattern),
    Tuple(Vec<Pattern>),
    Range(RangePattern),
    Or(Vec<Pattern>),
    /// `name @ pattern`
    At(AtPattern),
}

#[derive(Debug, Clone)]
pub struct BindingPattern {
    pub name: SmolStr,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructPattern {
    pub path: Path,
    pub fields: Vec<StructPatternField>,
    pub rest: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructPatternField {
    pub name: SmolStr,
    pub pattern: Option<Pattern>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct RangePattern {
    pub start: Option<Literal>,
    pub end: Option<Literal>,
    pub inclusive: bool,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct AtPattern {
    pub name: SmolStr,
    pub pattern: Box<Pattern>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ReturnExpr {
    pub value: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForExpr {
    pub var: SmolStr,
    pub iter: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileExpr {
    pub cond: Box<Expr>,
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LoopExpr {
    pub body: Block,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct LambdaExpr {
    pub params: Vec<LambdaParam>,
    pub return_type: Option<Type>,
    pub body: Box<Expr>,
    pub span: Span,
}

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

#[derive(Debug, Clone)]
pub enum StringInterpPart {
    Text(SmolStr),
    Expr(Expr),
}

/// `safe { java_call() }` — Java exception を Result に明示包装
#[derive(Debug, Clone)]
pub struct SafeExpr {
    pub block: Block,
    pub span: Span,
}
