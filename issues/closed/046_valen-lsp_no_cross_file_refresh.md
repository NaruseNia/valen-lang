---
scope: valen-lsp
severity: major
dimension: correctness
---

# Workspace files not re-analyzed on change

## 概要
didChange で変更ドキュメントのみ再解析。参照元ドキュメントの diagnostics が stale に。

## 改善案
変更後に全オープンドキュメントの再解析を実行（MVP 戦略）。

## 影響範囲
- crates/valen-lsp/
