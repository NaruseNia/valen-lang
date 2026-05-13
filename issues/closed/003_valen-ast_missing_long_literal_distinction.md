---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# Missing Long literal distinction (42L suffix)

## 概要

仕様 §2.1 で `42L` は Long 型と定義されるが、TokenKind/Literal に Long と Int の区別がない。型チェッカーが正しい型を割り当てられない。

## 改善案

Long(i64, Span) バリアントを追加するか、サフィックスフィールドを追加。

## 影響範囲

- crates/valen-ast/src/lib.rs:L309
- crates/valen-ast/src/token.rs:L11
