# 15. DSL / lambda

## 15.1 ラムダ

```valen
let double = |x: Int| x * 2;
let sum = |a: Int, b: Int| -> Int { a + b };
```

## 15.2 receiver lambda（Phase 1.5 以降）

MVP では receiver lambda (`T.() -> Unit`) を**提供しない**。Phase 1.5 で再評価。
