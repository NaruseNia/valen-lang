# VEP-012: Intersection constraints (T: A & B)

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

trait 境界を複合的に表現する intersection constraints `T: A & B` を導入する。

## 設計

`fn f<T: Read & Close>(x: T)` のように複数の trait 制約を `&` で結合する。anonymous sum type と union type の混同を避ける設計が重要。Java wildcard / intersection bound との対応も検討事項。
