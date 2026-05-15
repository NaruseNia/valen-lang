---
scope: valen-parser
severity: major
dimension: spec_coverage
---

# Generic variance always Invariant (in/out ignored)

## 概要
ジェネリクスパラメータパース時に variance が常に Invariant にハードコード。`in`/`out` キーワードが無視される。

## 現状
crates/valen-parser/src/parser.rs:L545: `variance: Variance::Invariant` がハードコード。`class Container<out T>` の `out` は型パラメータ名として解釈される。

## 改善案
parse_generic_params() で型パラメータ名の前に `in`/`out` キーワードをチェックし、Variance を設定。

## 影響範囲
- crates/valen-parser/src/parser.rs:L521-L552
