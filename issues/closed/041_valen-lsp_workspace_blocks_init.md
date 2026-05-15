---
scope: valen-lsp
severity: major
dimension: performance
---

# Workspace indexing blocks initialize response

## 概要
initialize ハンドラ内で全 .vln ファイルを読み込み解析。大規模ワークスペースで初期化が遅延。

## 改善案
バックグラウンドタスクに移動。initialized 通知後に非同期インデックス。

## 影響範囲
- crates/valen-lsp/
