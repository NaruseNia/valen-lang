# VEP-029: if let / while let

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 2 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

単一パターンの簡易 match として `if let` / `while let` 構文を導入する。

## 設計

`if let Some(x) = maybe_x { use(x); }` の形式で単一パターンのマッチと分岐を簡潔に記述する。芯を強める構文糖衣として `let-else` と並んで分類されている。
