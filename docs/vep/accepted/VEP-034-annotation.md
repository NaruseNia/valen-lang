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

## 実装済み設計決定（Phase 1.5 TASK-030）

| 項目 | 決定 |
|------|------|
| 宣言構文 | `annotation class Foo(pub x: Int)` — 新キーワード `annotation` |
| パラメータ値 | リテラルのみ（String, Int, Float, Bool, Long, Double, Char） |
| 適用対象 | トップレベル宣言 + フィールド/ctor パラメータ |
| retention | デフォルト RUNTIME。明示指定は Phase 2 |
| 引数構文 | named 引数基本 + 単一パラメータ時の名前省略可 |
| マーカー annotation | 許可、`()` 省略可 |
| JVM emit | `@interface` として emit。`@Retention(RUNTIME)` + `@Target(...)` 自動付与 |
| Java annotation 適用 | 使用可。検証なし（信頼ベース）で emit |
| @Target 指定 | `@Target("type", "field")` meta-annotation 形式 |
