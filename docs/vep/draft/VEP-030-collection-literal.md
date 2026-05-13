# VEP-030: Collection literal

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

リスト・マップのリテラル構文を導入する。

## 設計

`let xs = [1, 2, 3];` / `let map = {"a": 1, "b": 2};` の形式で collection を生成する。標準 collection の名義型との結びつけ方、Java collection へ落とすか Valen 独自 collection を持つかが検討事項。
