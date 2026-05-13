---
scope: valen-codegen
severity: enhancement
dimension: performance
---

# Excessive .to_string() allocations for constant JVM names

## 概要
"java/lang/Object", "<init>" 等の定数文字列を .to_string() で繰り返しヒープ確保。

## 改善案
SmolStr, Cow<'static, str>, または static 定数を使用。

## 影響範囲
- crates/valen-codegen/src/lower.rs
- crates/valen-codegen/src/data_class_methods.rs
- crates/valen-codegen/src/emit.rs
