---
scope: valen-ast
severity: major
dimension: correctness
---

# Span::merge silently corrupts in release builds

## 概要

Span::merge が file_id チェックに debug_assert_eq を使用。リリースビルドで異なるファイルの span をマージすると、破損した span が黙って生成される。

## 改善案

通常の assert! に変更（merge はホットパスではない）。

## 影響範囲

- crates/valen-ast/src/span.rs:L37-L46
