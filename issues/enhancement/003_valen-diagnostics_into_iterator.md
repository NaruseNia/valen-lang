---
scope: valen-diagnostics
severity: enhancement
dimension: design
---

# No IntoIterator impl for Diagnostics

## 概要
Diagnostics は iter() のみ公開。`for d in &diagnostics` が使えない。

## 改善案
&Diagnostics と Diagnostics に IntoIterator を実装。

## 影響範囲
- crates/valen-diagnostics/src/lib.rs
