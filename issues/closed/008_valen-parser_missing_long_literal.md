---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Lexer missing Long literal suffix (L)

## 概要
Lexer が `L` サフィックス付き整数リテラル（例: `42L`）を LongLit トークンとして生成しない。`42L` は IntLit(42) + Ident("L") としてlexされパースエラーになる。

## 現状
crates/valen-parser/src/lexer.rs: RawTok に LongLit variant なし。regex `[0-9][0-9_]*` は plain integers のみ。TokenKind::LongLit は valen-ast に存在するが lexer が生成しない。

## 改善案
`#[regex(r"[0-9][0-9_]*[Ll]", parse_long)] LongLit(i64)` を IntLit より高い優先度で追加。

## 影響範囲
- crates/valen-parser/src/lexer.rs
