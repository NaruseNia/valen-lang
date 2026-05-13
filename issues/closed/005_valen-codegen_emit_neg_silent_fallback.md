---
scope: valen-codegen
severity: critical
dimension: correctness
---

# emit_neg silent fallback to ineg for invalid types

## 概要

emit_neg がサポート外の型に対して黙って Ineg にフォールバックする。emit_arith と同じ問題。

## 現状

crates/valen-codegen/src/emit.rs:L712

```rust
_ => Instruction::Ineg,
```

## 改善案

`unreachable!("emit_neg called with unsupported type: {ty:?}")` に置き換える。

## 影響範囲

- crates/valen-codegen/src/emit.rs:L712
