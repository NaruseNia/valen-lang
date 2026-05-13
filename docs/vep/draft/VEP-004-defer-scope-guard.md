# VEP-004: defer / scope guard

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

リソース解放を `Result` や panic と整合させるための `defer` 構文を導入する。スコープ終了時に登録した処理を実行する。

## 設計

`defer expr;` でスコープ終了時の処理を登録する。Java `AutoCloseable` との対応、`defer` 中の失敗の扱い（握りつぶし / panic / 合成 Result）、`using` / `try-with-resources` 風構文との比較が主な検討事項。
