---
scope: valenfmt
severity: major
dimension: correctness
---

# Comment Extractor Unaware Of Fstrings

## 概要

extract_comments() doesn't handle f-string interpolation. Nested quotes cause scanner misalignment.

## 現状

crates/valenfmt/src/comment.rs:L34-L44

## 問題点

False comment extraction or missed comments with f-strings.

## 改善案

Add f-string awareness with brace depth tracking.

## 影響範囲

- crates/valenfmt/src/comment.rs

## 関連ファイル

(none)
