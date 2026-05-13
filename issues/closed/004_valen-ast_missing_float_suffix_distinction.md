---
scope: valen-ast
severity: major
dimension: spec_coverage
---

# Missing Float suffix distinction (3.14f vs 3.14)

## 概要

仕様 §2.1 で `3.14` は Double、`3.14f` は Float と定義されるが、サフィックス情報が保持されない。

## 改善案

サフィックスフィールドを追加するか、Double/Float を別バリアントに分離。

## 影響範囲

- crates/valen-ast/src/lib.rs:L310
- crates/valen-ast/src/token.rs:L12
