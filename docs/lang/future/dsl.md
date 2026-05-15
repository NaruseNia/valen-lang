# 15. DSL / lambda

## 15.1 ラムダ

```valen
let double = |x: Int| x * 2;
let sum = |a: Int, b: Int| -> Int { a + b };
```

## 15.2 クロージャのキャプチャ

ラムダは外部変数を**参照キャプチャ**する。`mut` 変数もキャプチャ可能 — コンパイラが自動的にボックス化（JVM ヒープ上の `Ref<T>` ラッパー）を行う。

```valen
let mut count = 0;
let inc = || { count = count + 1; };
inc();
inc();
// count == 2
```

- キャプチャされた `mut` 変数はラムダ内外で共有される（参照セマンティクス）
- Rust のような `move` キーワードは導入しない（所有権モデルなし）
- Java の effectively final 制約は適用しない

## 15.3 receiver lambda（Phase 1.5 以降）

MVP では receiver lambda (`T.() -> Unit`) を**提供しない**。Phase 1.5 で再評価。
