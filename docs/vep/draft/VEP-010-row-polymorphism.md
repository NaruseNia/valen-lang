# VEP-010: Row polymorphism / open record

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

匿名 record や部分 record を扱う row polymorphism を導入する。JavaBean / JSON / DB row との相性向上が狙い。

## 設計

`{ name: String, age: Int, ... }` のような open record 型を引数に取れるようにする。nominal type 中心の方針との衝突、Java interop での reflection 依存度、trait 制約による代替可能性が主な検討事項。特に慎重に扱う機能として分類されている。
