---
scope: valen-parser
severity: major
dimension: error_handling
---

# Integer overflow silently becomes lexer error

## 概要

parse_int が .ok() でオーバーフローを None に変換。ユーザーは「整数リテラルオーバーフロー」ではなく汎用 Error トークンを受け取る。

## 改善案

二次バリデーションパスまたはターゲット diagnostic を追加。

## 影響範囲

- crates/valen-parser/src/lexer.rs:L199-L201
