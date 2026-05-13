# VEP-021: Nullability trust modes

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 2 |
| 優先度 | Should |
| 関連 Issue | — |

## 概要

Java の `@NonNull` / `@Nullable` をどこまで信用するかを設定可能にする nullability trust modes を導入する。

## 設計

3 段階のモード: default（すべて `T?`）、strict annotations（信頼できる annotation package のみ `T`）、unsafe trust（classpath annotation を全面採用）。Valen の失敗モデルを壊さない default を維持しつつ、build tool で trust list を明示する方式。
