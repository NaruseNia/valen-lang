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

### 9.1.1 match guard

`match` arm は pattern の後に `if` 条件を置ける。guard は pattern が一致し、束縛変数が導入された後に評価される。

```valen
match user {
    User(name, age) if age >= 20 => f"adult: {name}",
    User(name, _) => f"minor: {name}",
}
```

guard 式の型は `Bool` でなければならない。guard 内では、その arm の pattern で束縛された名前を参照できる。

guard 付き arm は exhaustive check 上、**無条件に網羅したとは扱わない**。guard が実行時に `false` になりうるためである。

```valen
match n {
    x if x >= 0 => "non-negative",
    // ERROR: negative value が未網羅
}

match n {
    x if x >= 0 => "non-negative",
    _ => "negative",
}
```

or パターンに guard を付ける場合、guard は or パターン全体にかかる。

```valen
match n {
    2 | 4 | 6 if n < 10 => "small even",
    _ => "other",
}
```

## 9.1.2 let-else

`let-else` is a refutable pattern binding that diverges when the pattern does not match. The else block **must** diverge (`return`, `break`, `continue`, or `panic`).

```valen
let Some(health) = world.getComponent(entity, "Health") else { return; };
let Ok(data) = readFile(path) else { panic("read failed"); };
```

The bound variables (`health`, `data`) are available in the enclosing scope after the `let-else` statement. The else block's type must be `Nothing` (the bottom type).

This is syntactic sugar for early-return patterns, avoiding deeply nested `match` blocks:

```valen
// Without let-else:
let health = match world.getComponent(entity, "Health") {
    Option::Some(h) => h,
    Option::None => return,
};

// With let-else:
let Option::Some(health) = world.getComponent(entity, "Health") else { return; };
```

The pattern in `let-else` is always refutable (e.g. `Some(x)`, `Ok(v)`, `Color::Blue(n)`). Using an irrefutable pattern is permitted but unusual.

## 9.2 exhaustive check

- Valen `enum` / `sealed class` / `sealed trait` hierarchy：**厳密 exhaustive**（非網羅はコンパイルエラー）
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

## 9.4 if let / while let

`if let` は単一パターンに対する条件付き分解。`match` の 1-arm 版。

```valen
if let Some(pos) = getComponent(entity, "Position") {
    println(f"x={pos.x}, y={pos.y}");
} else {
    println("no position");
}
```

`else if let` チェーンも可能:

```valen
if let Some(pos) = getComponent(entity, "Position") {
    println(f"pos: ({pos.x}, {pos.y})");
} else if let Some(vel) = getComponent(entity, "Velocity") {
    println(f"vel only");
} else {
    println("no components");
}
```

`while let` はパターンが一致する間ループ:

```valen
while let Some(entity) = iter.next() {
    process(entity);
}
```

**制限（MVP）:** ガード条件 (`if let P = e && cond`) は非対応（Phase 3）。
