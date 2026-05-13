# VEP-027: Actor / channel

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

ADT と match を活かす message passing パターンとして actor / channel を導入する。

## 設計

enum でメッセージ型を定義し、exhaustive match で処理する actor モデルを提供する。標準ライブラリ機能で十分かどうか、exhaustive match と protocol evolution の相性が検討事項。
