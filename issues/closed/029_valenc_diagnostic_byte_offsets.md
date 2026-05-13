---
scope: valenc
severity: major
dimension: spec_coverage
---

# Diagnostic output uses byte offsets instead of line:column

## 概要

診断出力が生のバイトオフセット（`42..55`）を表示。REQ-TOOL-001 は `行:列` 形式の構造化フォーマットを要求。

## 改善案

行インデックスを構築し Span バイトオフセットを line:col に変換。codespan-reporting や miette の採用を検討。

## 影響範囲

- crates/valenc/src/main.rs
