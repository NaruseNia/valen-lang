---
scope: valen-parser
severity: minor
dimension: error_handling
---

# Describe Token Function Incomplete

## 概要

describe_token returns 'token' for most variants, producing unhelpful error messages.

## 現状

crates/valen-parser/src/parser.rs:L2581-L2609

## 問題点

'expected token' is confusing.

## 改善案

Add descriptions for all TokenKind variants used with expect().

## 影響範囲

- crates/valen-parser/src/parser.rs:L2581-L2609

## 関連ファイル

(none)
