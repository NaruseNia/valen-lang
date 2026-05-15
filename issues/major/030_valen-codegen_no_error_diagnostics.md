---
scope: valen-codegen
severity: major
dimension: error_handling
---

# No codegen-level error diagnostics

## 概要
CodegenError が ClassFile と UnresolvedLabel の2 variant のみ。他の失敗は全て panic か silent no-op。

## 改善案
UnsupportedOperation, InvalidType, SlotOverflow, InternalError variant を追加。

## 影響範囲
- crates/valen-codegen/
