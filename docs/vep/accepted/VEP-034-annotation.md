# VEP-034: Annotation (declaration + application + runtime)

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 1.5 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

Valen 独自の annotation システムを導入する。宣言（declaration）、適用（application）、実行時参照（runtime）の 3 フェーズを統合的に設計する。

## 設計

Java annotation との互換性を保ちつつ Valen 固有の annotation を定義・適用・実行時参照できるシステムを構築する。`@` を Java annotation / Valen annotation 共通の適用構文とし、retention / target の指定、compile-time と runtime の使い分けを設計する。VEP-020 (Java annotation authoring) と密接に関連。
