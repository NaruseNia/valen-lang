# VEP-005: Pipeline operator

| 項目 | 内容 |
|------|------|
| ステータス | Accepted |
| Phase | 2 |
| 優先度 | Could |
| 関連 Issue | — |

## 概要

値を左から右へ流す `|>` 演算子を導入する。ネストした関数適用や `map` / `filter` 連鎖の可読性を向上させる。

## 設計

`x |> f` は `f(x)` に脱糖する。`x |> T::method(arg)` は `T::method(x, arg)` に脱糖する方向。UFCS との整合性、メソッドチェーンとの二重化、`Result` / `Option` の `?` との優先順位が主な検討事項。
