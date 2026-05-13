---
scope: valenc
severity: major
dimension: design
---

# Massive code duplication between compile and check functions

## 概要

compile() と check() が約80行の診断表示ロジックをほぼ同一でコピー。7箇所に重複。

## 改善案

共通パイプライン関数または diagnostic-emit ヘルパーを抽出。

## 影響範囲

- crates/valenc/src/main.rs
