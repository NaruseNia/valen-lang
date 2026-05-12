---
scope: valen-diagnostics
severity: minor
dimension: spec_coverage
---

# Missing hint() convenience method

## 概要
Severity::Hint が存在するが Diagnostics に hint() メソッドがない。ヒントの発行に Diagnostic 手動構築が必要。

## 改善案
error()/warning() と同様の hint() を追加。

## 影響範囲
- crates/valen-diagnostics/src/lib.rs
