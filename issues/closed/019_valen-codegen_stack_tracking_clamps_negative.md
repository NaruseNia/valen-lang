---
scope: valen-codegen
severity: major
dimension: correctness
---

# Stack tracking clamps negative to zero, masking underflow bugs

## 概要

emit_body が負のスタック深度を 0 にクランプし、スタックアンダーフローバグを隠蔽。

## 改善案

debug_assert に置き換え。デバッグビルドで負のスタック深度時にパニック。

## 影響範囲

- crates/valen-codegen/src/emit.rs:L294-L296
