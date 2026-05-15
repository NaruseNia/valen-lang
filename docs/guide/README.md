# Valen ガイド

Valen 言語のユーザー向けガイドとコントリビュータ向けリファレンス。

## 目次

### 言語ガイド

- [01. はじめに](01-getting-started.md) — インストール、Hello World、基本構文
- [02. 型システム](02-types.md) — プリミティブ型、Option、型推論、typealias
- [03. ジェネリクス](03-generics.md) — 型パラメータ、bounds、variance
- [04. クラス](04-classes.md) — class、data class、継承、sealed class
- [05. Enum とパターンマッチ](05-enum-and-match.md) — ADT、exhaustive match
- [06. Trait と Impl](06-traits.md) — trait 定義、impl、orphan rule、UFCS
- [07. 失敗モデル](07-failure-model.md) — Option、Result、?演算子、safe ブロック
- [08. Java 連携](08-java-interop.md) — import、safe ブロック、@valen.Closed

### コントリビュータ向け

- [09. コンパイラアーキテクチャ](09-compiler-architecture.md) — パイプライン、crate 構成、ビルド方法

## 正式言語仕様

正式な言語仕様は [docs/lang/](../lang/) 配下を参照してください。本ガイドは仕様の理解を助ける実用的な解説です。
