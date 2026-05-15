---
scope: valen-codegen
severity: major
dimension: correctness
---

# Lambda 3+ params silently produces wrong arity

## 概要
3+パラメータのラムダが Function<Object,Object>（1引数のみ）にフォールバック。実行時 LambdaConversionException。

## 改善案
コンパイル時エラーにするか、カスタム FunctionalInterface を実装。

## 影響範囲
- crates/valen-codegen/
