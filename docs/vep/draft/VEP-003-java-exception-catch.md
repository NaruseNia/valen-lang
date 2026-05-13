# VEP-003: Java Exception catch expression (safe catch)

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

Java ライブラリの Exception を明示的に捕捉し、Valen の `Result` / ユーザ定義エラーへ変換する構文を導入する。Java の throw/catch モデルを Valen 内に広げるのではなく、interop 境界で Exception を閉じ込めることが目的。

## 設計

有力案は `safe { ... } catch ...` を Java 境界専用構文とする方式。成功時は `Ok(value)`、catch arm は `Err(domain_error)` を明示的に返す。Valen 内では `throw` 文を導入せず、Java ラッパー層で Exception を Valen の ADT エラーへ畳み込む用途に限定する。catch arm は declaration order で評価し、subtype から supertype の順を要求する方向。
