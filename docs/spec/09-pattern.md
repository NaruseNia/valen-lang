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
- Java 型：**`@valen.Closed` アノテーション付きのみ exhaustive**、他は常に open-world

```valen
// Java 側定義（ライブラリ作者が @valen.Closed を付与）
@Closed
sealed interface Color permits Red, Blue, Green

match color {
    Color.Red => ...,
    Color.Blue => ...,
    Color.Green => ...,  // 網羅しないとコンパイルエラー
}
```

## 9.3 `@valen.Closed` 不在時の動作

**Java `sealed` 単独では exhaustive 扱いにしない**。`@valen.Closed` の付与がない Java hierarchy は open-world として扱い、`match` では wildcard arm (`_`) を **必ず要求する**。

```valen
// @valen.Closed なし — wildcard 必須
match javaSealed {
    Foo.A => ...,
    Foo.B => ...,
    _ => ...,  // 省くとコンパイルエラー
}
```

理由: Valen 自身が定義した closed world はコンパイラが完全に把握できるが、Java 定義の closed world は classpath 変動・tooling 差異があり、同じ厳密さを保証できない。annotation による明示 opt-in を要求することで、classpath で permit が増えたときに silently non-exhaustive 化する事故を防ぐ。

詳細は [20. アノテーション](20-annotations.md) を参照。
