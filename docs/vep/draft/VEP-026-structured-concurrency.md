# VEP-026: Structured concurrency

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

task scope を言語または標準ライブラリで提供し、構造化された並行処理を実現する。

## 設計

`task_scope { let a = async { ... }; let b = async { ... }; combine(a.await?, b.await?) }` の形式で structured concurrency を表現する。Java `StructuredTaskScope` との対応、cancellation を panic / Result / 専用型のどれで表すかが検討事項。
