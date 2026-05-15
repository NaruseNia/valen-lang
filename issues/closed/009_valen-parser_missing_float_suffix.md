---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Lexer missing Float suffix literal (f)

## 概要
`3.14f` が DoubleLit(3.14) + Ident("f") としてlexされ、単一の FloatLit トークンにならない。

## 現状
crates/valen-parser/src/lexer.rs: 全浮動小数点リテラルが RawTok::FloatLit(f64) → TokenKind::DoubleLit にマップ。`f` サフィックス処理なし。

## 改善案
f サフィックス付き regex variant を追加: `#[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9]+)?[fF]", parse_float_f32)]`

## 影響範囲
- crates/valen-parser/src/lexer.rs
