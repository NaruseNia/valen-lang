---
scope: valen-ast
severity: enhancement
dimension: performance
---

# Large Expr enum size — consider boxing large variants

## 概要
Expr enum に 22 バリアントあり、Block 等の大きなバリアントがサイズを支配。小さなバリアント（Literal, Continue）のメモリ浪費。

## 改善案
`std::mem::size_of::<Expr>()` で測定し、大きいバリアントを Box 化。

## 影響範囲
- crates/valen-ast/src/lib.rs:L282-L305
