# VEP-035: struct (value type)

| 項目 | 内容 |
|------|------|
| ステータス | Draft |
| Phase | 3 |
| 優先度 | Could |
| 関連 Issue | — |
| 依存 VEP | VEP-018 (Valhalla) |

## 概要

JVM value class にマップされる軽量値型を `struct` キーワードで導入する。全フィールド immutable、継承不可、参照同一性 (`===`) なし。Java 25+ target 専用。

## 設計

```valen
struct Point(x: Float, y: Float);
struct Color(r: Int, g: Int, b: Int);
```

- 全フィールド immutable 強制（`mut` 不可）
- 継承不可（`class` extends も `sealed` も不可）
- 振る舞いは `impl Trait for Struct` で外付けのみ（本体メソッドなし）
- `===` / `!==` はコンパイルエラー（no identity）
- equals / hashCode / toString / copy を自動生成（data class と同様）
- Java 21 target ではコンパイルエラー。Java 25+ で Valhalla value class として emit
- data class との違い: struct は identity なし + Valhalla emit。data class は通常の reference type
