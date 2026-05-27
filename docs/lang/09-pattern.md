# 9. パターンマッチ

## 9.1 パターンの種類

```valen
match value {
    0 => "zero",                          // リテラル
    1..=9 => "small",                     // 範囲
    10 | 20 | 30 => "round",              // or パターン
    n if n < 0 => "negative",             // ガード
    Shape::Circle(r) => f"circle r={r}",  // 構造分解（enum variant / data class）
    Shape::Rect(w, h) => f"rect {w}x{h}", // 複数フィールド
    .Some(v) => f"shorthand {v}",         // variant shorthand
    p @ User(name: "admin", ..) => admin_action(p),  // @束縛 + rest
    _ => "other",
}
```

### 対応するパターン一覧

| パターン | 構文例 | AST ノード |
|----------|--------|-----------|
| ワイルドカード | `_` | `Pattern::Wildcard` |
| リテラル | `0`, `"hello"`, `true`, `'a'`, `3.14f`, `1L` | `Pattern::Literal` |
| 変数束縛 | `x`, `mut x` | `Pattern::Binding` |
| パス | `Shape::Circle`, `Option::None` | `Pattern::Path` |
| 構造分解 | `Shape::Circle(r)`, `Some(v)` | `Pattern::Struct` |
| 範囲 | `1..10`, `0..=255` | `Pattern::Range` |
| or | `A \| B \| C` | `Pattern::Or` |
| @束縛 | `p @ Some(v)` | `Pattern::At` |
| variant shorthand | `.None`, `.Some(v)` | `Pattern::VariantShorthand` |
| タプル | *(将来予約)* | `Pattern::Tuple` |

### 9.1.1 or パターンの束縛一貫性

or パターン（`p1 | p2 | ...`）の各 alternative が変数を束縛する場合、**すべての alternative が同じ変数名の集合を束縛しなければならない**。束縛する変数名が異なるとコンパイルエラーになる。

```valen
enum Expr {
    Lit(value: Int),
    Neg(value: Int),
    Add(lhs: Int, rhs: Int),
}

match expr {
    Expr::Lit(v) | Expr::Neg(v) => v,   // OK: 両方 `v` を束縛
    Expr::Add(a, b) => a + b,
}

match expr {
    Expr::Lit(v) | Expr::Neg(w) => ..., // ERROR: 変数名が異なる（`v` vs `w`）
}
```

束縛を持たない or パターン（リテラルのみ等）にはこの制約は適用されない。

```valen
match n {
    1 | 2 | 3 => "small",   // OK: 束縛なし
    _ => "other",
}
```

> **実装状況:** パーサーも exhaustiveness checker もこの一貫性検証を行わない（未実装）。将来のセマンティックチェックパスで検証予定。現時点では不一致な束縛を書いてもコンパイルエラーにならない。

### 9.1.2 match guard

`match` arm は pattern の後に `if` 条件を置ける。guard は pattern が一致し、束縛変数が導入された後に評価される。

```valen
match user {
    User(name, age) if age >= 20 => f"adult: {name}",
    User(name, _) => f"minor: {name}",
}
```

guard 式の型は `Bool` でなければならない。guard 内では、その arm の pattern で束縛された名前を参照できる。

guard 付き arm は exhaustive check 上、**無条件に網羅したとは扱わない**。guard が実行時に `false` になりうるためである。exhaustiveness checker は guard 付き arm をスキップする（正しい動作）。

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

### 9.1.3 variant shorthand パターン

`.Variant` / `.Variant(fields)` 形式でコンテキストから enum 型を推論し、`EnumName::Variant` のフルパスを省略できる。

```valen
enum Color { Red, Green, Blue }

fn name(c: Color) -> String {
    match c {
        .Red => "red",
        .Green => "green",
        .Blue => "blue",
    }
}
```

フィールド付き variant:

```valen
enum Shape { Circle(r: Float), Rect(w: Float, h: Float), Point }

match shape {
    .Circle(r) => f"circle r={r}",
    .Rect(w, h) => f"rect {w}x{h}",
    .Point => "point",
}
```

`..`（rest）も使用可能:

```valen
match shape {
    .Circle(..) => "circle",
    _ => "other",
}
```

variant shorthand は名前が大文字で始まる識別子に対してのみパース（`.` の直後の識別子が `[A-Z]` 始まり）。exhaustiveness checker も `VariantShorthand` パターンを正しく認識する。

### 9.1.4 タプルパターン（将来予約）

`Pattern::Tuple` は AST に定義されているがパーサーにパースロジックはない。現在は使用不可。将来のタプル型サポートに向けて予約されている。

## 9.1.5 let-else

`let-else` は refutable pattern binding で、パターンが一致しない場合 else ブロックで発散する。else ブロックは**必ず発散**（`return`, `break`, `continue`, `panic`）しなければならない。

```valen
let Some(health) = world.getComponent(entity, "Health") else { return; };
let Ok(data) = readFile(path) else { panic("read failed"); };
```

束縛された変数（`health`, `data`）は `let-else` 文以降の囲みスコープで使用可能。else ブロックの型は `Nothing`（bottom type）でなければならない。

これは早期リターンパターンの糖衣構文で、深い `match` ネストを回避する:

```valen
// let-else なし:
let health = match world.getComponent(entity, "Health") {
    Option::Some(h) => h,
    Option::None => return,
};

// let-else あり:
let Option::Some(health) = world.getComponent(entity, "Health") else { return; };
```

`let-else` のパターンは常に refutable（例: `Some(x)`, `Ok(v)`, `Color::Blue(n)`）。irrefutable パターンの使用は許可されるが通常ではない。

## 9.2 範囲パターン

```valen
match n {
    0..10 => "single digit",     // 排他的（0 <= n < 10）
    10..=99 => "double digit",   // 包含的（10 <= n <= 99）
    _ => "large",
}
```

### 実装上の制限

- **開始リテラル**: `IntLit`, `LongLit` を受け付ける（パーサーが `IntLit` / `LongLit` で範囲パターンへ分岐）
- **終了リテラル**: `IntLit` のみ受け付ける。`LongLit` を終了値に書くとパースに失敗する（`parse_range_pattern()` が終了側で `IntLit` のみチェック）
- `Float`, `Double`, `Char`, `String` などの範囲パターンは未サポート

## 9.3 exhaustive check

### 9.3.1 対象型

| 対象 | 網羅性チェック |
|------|--------------|
| Valen `enum` | 全 variant の網羅を要求 |
| `sealed class` | 全サブクラスの網羅を要求 |
| `sealed trait` | 全 implementor の網羅を要求 |
| `Bool` | `true` と `false` 両方の網羅を要求 |
| `Int`, `String` 等 | 網羅性チェックなし |

### 9.3.2 Java 型との連携

- Java `sealed` 単独では exhaustive 扱いにしない
- `@valen.Closed` アノテーション付きの Java sealed hierarchy のみ closed-world として exhaustive check を有効化
- `@valen.Closed` 不在の Java hierarchy は open-world → wildcard arm (`_`) が必須

```valen
// @valen.Closed 付き Java sealed interface
match color {
    Color.Red => ...,
    Color.Blue => ...,
    Color.Green => ...,  // 網羅しないとコンパイルエラー
}

// @valen.Closed なし — wildcard 必須
match javaSealed {
    Foo.A => ...,
    Foo.B => ...,
    _ => ...,  // 省くとコンパイルエラー
}
```

詳細は [20. アノテーション](20-annotations.md) を参照。

### 9.3.3 既知の制限

exhaustiveness checker は**生の AST 上で動作**し、scrutinee の型をローカル変数のアノテーションから再推論する。以下のケースでは型推論が効かず、exhaustiveness check がサイレントにスキップされる:

- 関数の戻り値（`match getColor() { ... }`）
- メソッドチェーン（`match obj.method().field { ... }`）
- 複雑な式（`match if cond { a } else { b } { ... }`）

**パラメータの型アノテーション**と**`let` 束縛の型アノテーション**からのみ scrutinee 型を推論できる。

```valen
fn process(c: Color) {
    match c { ... }  // ✓ パラメータ型アノテーションから Color を推論
}

fn process2() {
    let c: Color = getColor();
    match c { ... }  // ✓ let 型アノテーションから Color を推論
}

fn process3() {
    match getColor() { ... }  // ✗ 型推論できず、check スキップ
}
```

> **設計ノート (#025):** 将来的には型チェック済み HIR（`TypedExpr`）を消費する形にリファクタリングし、すべての scrutinee 型で正確な exhaustiveness check を行う予定。

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

**制限:** ガード条件（`if let P = e && cond`）は現在サポートしていない。
