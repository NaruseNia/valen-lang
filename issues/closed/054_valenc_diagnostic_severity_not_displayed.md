---
scope: valenc
severity: major
dimension: error_handling
---

# Diagnostic Severity Not Displayed

## 概要

CLI output omits severity (Error/Warning/Hint). All diagnostics look the same.

## 現状

crates/valenc/src/main.rs:L179-L202

## 問題点

Users can't distinguish errors from warnings.

## 改善案

Include diag.severity in output format.

## 影響範囲

- crates/valenc/src/main.rs

## 関連ファイル

(none)
