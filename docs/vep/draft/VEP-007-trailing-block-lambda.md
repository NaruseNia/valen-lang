# VEP-007: Trailing block lambda

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

最後の引数がラムダのとき、括弧の外へ出せる trailing block 構文を導入する。DSL 構築とリソース管理に有効。

## 設計

`f(args) { block }` の形式で最後のラムダ引数を括弧の外に出す。receiver lambda とセットで導入するか、通常ラムダだけで始めるかを検討。制御構文に見える API の許容範囲、Java SAM 変換との組み合わせが検討事項。
