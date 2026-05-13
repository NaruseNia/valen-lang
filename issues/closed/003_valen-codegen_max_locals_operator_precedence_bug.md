---
scope: valen-codegen
severity: critical
dimension: correctness
---

# lower_method max_locals operator precedence bug

## 概要

lower_method で `if has_self { 1 } else { 0 } + params...sum()` が Rust の演算子優先度により `if has_self { 1 } else { 0 + params...sum() }` とパースされる。self ありメソッドで max_locals=1 となり JVM 検証エラーが発生する。

## 現状

crates/valen-codegen/src/lower.rs:L241-L242

```rust
let max_locals =
    if has_self { 1 } else { 0 } + params.iter().map(|t| t.slot_count()).sum::<u16>();
```

## 問題点

has_self が true の場合、max_locals は 1 になり（パラメータ分を無視）、JVM 検証エラーが発生する。

## 改善案

括弧を追加: `(if has_self { 1u16 } else { 0 }) + params.iter().map(|t| t.slot_count()).sum::<u16>()`

## 影響範囲

- crates/valen-codegen/src/lower.rs:L241-L242
