---
scope: valen-parser
severity: minor
dimension: correctness
---

# Impl Block Functions Always Pub Visibility

## 概要

Functions in impl blocks hardcoded to Visibility::Pub, ignoring user-written visibility.

## 現状

crates/valen-parser/src/parser.rs:L944-L945

## 問題点

private fn in impl block silently becomes pub.

## 改善案

Parse visibility before fn in impl blocks.

## 影響範囲

- crates/valen-parser/src/parser.rs:L928-L946

## 関連ファイル

(none)
