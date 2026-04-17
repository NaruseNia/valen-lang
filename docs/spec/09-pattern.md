# 9. パターンマッチ

## 9.1 フルセット

```valen
match value {
    0 => "zero",                          // リテラル
    1..=9 => "small",                     // 範囲
    10 | 20 | 30 => "round",              // or パターン
    n if n < 0 => "negative",             // ガード
    Shape::Circle(r) => f"circle r={r}",  // 構造分解
    Shape::Rect(w, h) => f"rect {w}x{h}", // 複数フィールド
    p @ User(name = "admin", ..) => admin_action(p),  // @束縛 + rest
    _ => "other",
}
```

## 9.2 exhaustive check

- Valen `enum` / `sealed class` hierarchy：**厳密 exhaustive**（非網羅はコンパイルエラー）
- Java 型：**`@closed` アノテーション付きのみ exhaustive**、他は普通の分岐（exhaustive check なし）

```valen
@closed
sealed interface Color  // Java 側定義

match color {
    Color.Red => ...,
    Color.Blue => ...,
    Color.Green => ...,  // 網羅しないとコンパイルエラー
}
```
