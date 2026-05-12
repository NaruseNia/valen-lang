---
scope: valenc
severity: major
dimension: correctness
---

# Hardcoded FileId(0) for all input files

## 概要

全入力ファイルに FileId(0) を割り当て。複数ファイルコンパイル時に診断のソース位置が区別不能。

## 改善案

各入力ファイルにユニークな FileId を割り当て。

## 影響範囲

- crates/valenc/src/main.rs
