---
scope: valen-lsp
severity: minor
dimension: design
---

# Server Rs 3159 Line Monolith

## 概要

server.rs is 3159 lines containing all LSP logic. Hard to navigate and extend.

## 現状

crates/valen-lsp/src/server.rs

## 問題点

Completion(700), hover(60), semantic tokens(60), inlay hints(230), helpers(600) all in one file.

## 改善案

Split into completion.rs, hover.rs, semantic_tokens.rs, inlay_hints.rs.

## 影響範囲

- crates/valen-lsp/src/server.rs

## 関連ファイル

(none)
