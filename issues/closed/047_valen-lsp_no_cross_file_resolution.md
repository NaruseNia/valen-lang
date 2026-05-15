---
scope: valen-lsp
severity: major
dimension: correctness
---

# Each document analyzed in isolation

## 概要
analyze_document が単一ファイルのみ parse+resolve。他ファイルの定義を参照不能、name-not-found エラー。

## 改善案
マルチファイル HIR 構築戦略の導入。全ワークスペースのシンボルテーブルをマージ。

## 影響範囲
- crates/valen-lsp/
