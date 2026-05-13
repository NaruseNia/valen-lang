---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# No generics parsing for fn/class/trait/enum/impl

## 概要

`fn map<T, U>(...)` のようなジェネリクス宣言がパースできない。全 generics フィールドが Vec::new() にハードコード。REQ-TYPE-006 (Must) で必須。

## 改善案

`<T, U: Bound>` 構文をパースする parse_generic_params を実装。

## 影響範囲

- crates/valen-parser/src/parser.rs:L158
