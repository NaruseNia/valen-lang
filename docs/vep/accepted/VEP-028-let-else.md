# VEP-028: let-else

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 2 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

パターン不一致時の早期脱出を簡潔に書ける `let-else` 構文を導入する。

## 設計

`let Some(user) = find_user(id) else { return Err(AppError::NotFound(id)); };` の形式で、パターンマッチの失敗時に diverge する else ブロックを記述する。芯を強める構文糖衣として分類されている。
