---
scope: valen-parser
severity: minor
dimension: spec_coverage
---

# Missing bitwise operator parsing

## 概要
lexer トークンと BinaryOp バリアント（BitAnd/BitOr/BitXor/Shl/Shr）が定義されているが、パーサーに優先度レベルがない。

## 改善案
logical と comparison の間にビット演算優先度レベルを追加。

## 影響範囲
- crates/valen-parser/src/parser.rs:L620-L732
