---
scope: valen-codegen
severity: major
dimension: fixture_coverage
---

# Missing fixture tests for match expression and string interpolation

## 概要

match 式（コア4本柱の1つ）と文字列補間（複雑な StringBuilder バイトコード）の .vln fixture テストがない。

## 改善案

fn_match.vln と fn_string_interp.vln fixture を追加。

## 影響範囲

- crates/valen-codegen/src/expr.rs:L510-L548
- crates/valen-codegen/src/expr.rs:L766-L807
