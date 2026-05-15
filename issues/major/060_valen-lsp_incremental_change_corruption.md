---
scope: valen-lsp
severity: major
dimension: concurrency
---

# LSP didChange がインクリメンタル編集を全文置換として処理

## 概要

サーバーは full sync を advertise するが、didChange は range フィールドを無視して最後の content_changes エントリを全文として格納。クライアントが advertised mode に関わらず incremental change を送信した場合、解析状態が壊れる。

## 改善案

受信 change が full-document change であることを検証。range 付き change はログ出力 + reject するか、UTF-16 位置で既存 DocumentState に range apply を実装。

## 影響範囲

- crates/valen-lsp/src/server.rs
