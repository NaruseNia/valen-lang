# 6. enum（ADT）

## 6.1 定義

```valen
enum Shape {
    Circle(r: Float),
    Rect(w: Float, h: Float),
    Point,
}
```

- Valen の enum は **Rust 風 ADT**（class と完全分離）
- variant は payload（named fields）を持てる
- 閉じた sum type、match で exhaustive

## 6.2 バリアントアクセス

```valen
let s = Shape::Circle(r = 5.0);
let p = Shape::Point;
```

スコープ演算子 `::` で variant にアクセス。

## 6.3 enum と sealed class の使い分け

**操作で区別する**（「表現したいもの」ではなく「許される操作」で切る）。

| | enum | sealed class |
|---|---|---|
| 位置づけ | ADT、**data の和** | closed OOP hierarchy、**振る舞いの階層** |
| variant / subtype の素性 | payload-holding data container | 独自 state / method / trait impl を持てる |
| 独自 method | **持てない**（trait impl 経由のみ） | 持てる |
| 継承関係 | なし（flat） | 親-子の階層を作れる |
| 可視性差分 | variant ごとの差分 **不可** | 各 subtype で個別 |

**選択基準:**

- **データの和を表す** → `enum`
- **振る舞いの階層を表す** → `sealed class`

`enum` は variant 自体を拡張しない純粋な識別付き和型、`sealed class` は OOP 階層の閉じた形。両方で書ける場面に迷ったら `enum` を先に試し、variant ごとに独自 method / state が必要になった時点で `sealed class` を検討する。

## 6.4 Java bytecode 表現

Valen enum は以下のように bytecode に emit される。

```java
// Shape.valen → Shape.class
public sealed interface Shape permits Shape$Circle, Shape$Rect, Shape$Point {}

public static final record Shape$Circle(double r) implements Shape {}
public static final record Shape$Rect(double w, double h) implements Shape {}

// payload なし variant は singleton
public static final class Shape$Point implements Shape {
    public static final Shape$Point INSTANCE = new Shape$Point();
    private Shape$Point() {}
}
```

**設計決定:**

- payload あり variant → `record`
- payload なし variant → `singleton class`（allocation 節約）
- Valen ABI と Java surface ABI は分離管理

## 6.5 Java ABI 凍結条件（MVP）

以下は MVP 凍結ルール、将来変更しない。

### 6.5.1 binary naming

- variant の Java binary name は **`EnumName$VariantName`**（`$` 区切り）
- Java inner class 風の記法を踏襲、Kotlin sealed hierarchy と互換性が高い
- Java reflection で `Class.forName("com.example.Shape$Circle")` と読める

### 6.5.2 serializer

- **Valen は serializer を提供しない**
- JSON / XML 等の serialization を必要とする場合は、利用者側で Jackson / Gson / その他を設定する責任を持つ
- Valen が特定ライブラリに lock-in しない方針

### 6.5.3 reflection

- 各 variant は通常の Java class（record または singleton class）として反射から見える
- 特別な reflection helper や registry は提供しない

### 6.5.4 trait impl の Java surface 露出

**可視性と連動する。**

| trait 可視性 | Java surface |
|---|---|
| `pub trait` | Java interface として emit、variant record が `implements` で公開 |
| `internal trait` | Valen 内部のみ、Java から見えない |
| `private trait` | Valen 内部のみ、Java から見えない |

これにより Java 側ユーザは Valen 公開 trait だけを安定 API として扱える。

### 6.5.5 互換性ポリシー

trait を追加したときの semver 影響：

- **`pub trait` の追加** → **major** bump
  - Java interface が増え、variant record の binary API が変わる
  - 既存 Java 利用者の binary 互換性が失われる可能性
- **`internal` / `private` trait の追加** → **minor** bump
  - Java surface に見えないので影響しない

variant の追加・削除・payload 変更は常に major。
