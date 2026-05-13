---
scope: valen-codegen
severity: major
dimension: correctness
---

# Return stack_delta incorrect for wide types (Long/Double)

## 概要

JvmOp::Return の stack_delta が全非 void 戻り値で -1 を返すが、Long/Double は 2 スロット占有するため -2 であるべき。

## 改善案

`-(ty.slot_count() as i32)` を使用。

## 影響範囲

- crates/valen-codegen/src/jvm_ir.rs:L301-L306
