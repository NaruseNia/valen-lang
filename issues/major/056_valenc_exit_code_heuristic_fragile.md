---
scope: valenc
severity: major
dimension: error_handling
---

# Exit Code Heuristic Fragile

## 概要

Exit code determined by string matching e.to_string().ends_with('errors'). Fragile.

## 現状

crates/valenc/src/main.rs:L237-L240

## 問題点

Message change breaks exit code classification.

## 改善案

Use dedicated error enum for exit code determination.

## 影響範囲

- crates/valenc/src/main.rs

## 関連ファイル

(none)
