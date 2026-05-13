# VEP-033: Operator overload (trait-based)

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 1.5 |
| 優先度 | Must |
| 関連 Issue | — |

## 概要

trait ベースの演算子オーバーロードを導入する。`Add` / `Sub` / `Eq` 等の trait を impl することで演算子の挙動を定義する。

## 設計

Rust と同様に `trait Add { fn add(self, rhs: Self) -> Self; }` のような trait を定義し、`impl Add for MyType` で演算子の挙動を実装する。orphan rule / coherence と整合させた上で、trait ベースの dispatch により型安全な演算子オーバーロードを実現する。
