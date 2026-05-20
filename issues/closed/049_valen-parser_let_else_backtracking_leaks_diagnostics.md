---
scope: valen-parser
severity: major
dimension: correctness
---

# Let Else Backtracking Leaks Diagnostics

## 概要

let-else backtracking restores position but not diagnostics, causing spurious errors.

## 現状

crates/valen-parser/src/parser.rs:L1047-L1061

## 問題点

Failed let-else attempt leaves stale diagnostics.

## 改善案

Save and restore diagnostics count on backtrack.

## 影響範囲

- crates/valen-parser/src/parser.rs:L1047-L1061

## 関連ファイル

(none)
