---
scope: valen-codegen
severity: major
dimension: correctness
---

# Non-local callee fallback leaves dangling stack value

## 概要
non-Fn型calleeをスタックに積んだ後 InvokeStatic を emit。InvokeStatic は objectref を消費しないため stack leak。VerifyError。

## 改善案
InvokeVirtual/InvokeInterface を使うか、到達不能として diagnostic/ICE。

## 影響範囲
- crates/valen-codegen/
