# VEP-009: Anonymous sum types

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

名前付き enum を定義せず、その場で閉じた和型を表現する匿名和型を導入する。小さな parser / visitor / interop adapter での型定義ノイズを削減する。

## 設計

`|Variant1(T) | Variant2(U)|` の形式で匿名和型を表現する。exhaustive match の価値を局所的な型にも広げる。Java ABI への落とし方（匿名 sealed hierarchy / コンパイラ内部型）、public API への露出の可否、名前付き enum への昇格リファクタの容易さが検討事項。
