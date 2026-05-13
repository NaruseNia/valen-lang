# VEP-002: Effect-like try block

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

`Result` / `Option` / Java exception 境界をブロック単位で扱う `try` 構文を導入する。`?` の伝播先をブロックで明示し、異なる失敗コンテキストの混在を防ぐ。

## 設計

`try Result<T, E> { ... }` / `try Option<T> { ... }` の形式で、ブロック内の `?` 演算子の伝播先を明示する。将来の effect system への足場として設計する。`try` が単なるブロック式か専用の effect boundary かは要検討。`Result<T, E>` の `E` 同一型ルール維持の是非も検討事項。
