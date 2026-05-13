# VEP-020: Java annotation authoring

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 1.5 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

Valen コードから Java annotation を付与・宣言できるようにする。Java framework 連携に必要な基盤機能。

## 設計

`@Deprecated("use newApi")` のように Java annotation を Valen コードに付与する。`@` を Java annotation、`#[...]` を Valen compiler attribute として分離する方向。retention / target / repeatable の指定、annotation 引数に許す式の範囲が検討事項。
