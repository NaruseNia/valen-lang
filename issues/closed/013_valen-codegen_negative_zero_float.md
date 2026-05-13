---
scope: valen-codegen
severity: minor
dimension: correctness
---

# PushFloat/PushDouble -0.0 incorrectly emitted as +0.0

## 概要
`== 0.0` が +0.0 と -0.0 の両方にマッチ。-0.0f が fconst_0 (+0.0) として emit される。

## 改善案
`n.to_bits() == 0.0f32.to_bits()` で符号ビットを区別。

## 影響範囲
- crates/valen-codegen/src/emit.rs:L549, L561
