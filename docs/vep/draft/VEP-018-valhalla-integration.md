# VEP-018: Project Valhalla integration

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

JVM value classes / primitive classes を Valen の `newtype`、小さな record、Option 最適化に活用する Project Valhalla 連携を導入する。

## 設計

`newtype UserId = Int` を value class として emit、`Option<Int>` の boxing 削減、小さな enum / record の flat layout を `--target 25` 以降で opt-in 提供する。JVM バージョンごとの ABI 変動、Java から見た API 安定性、Valhalla 非対応 target への fallback、「仕様上の意味」と「最適化」の分離が検討事項。
