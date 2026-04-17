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
- variant は payload を持てる（named fields）
- 閉じた sum type、match で exhaustive

## 6.2 バリアントアクセス

```valen
let s = Shape::Circle(r = 5.0);
let p = Shape::Point;
```

スコープ演算子 `::` で variant にアクセス。

## 6.3 Java bytecode 表現

Valen enum は以下のように bytecode に emit される：

```java
// Shape.valen → Shape.class (bytecode 相当の Java 表現)
public sealed interface Shape permits Shape$Circle, Shape$Rect, Shape$Point {}

public static final record Shape$Circle(double r) implements Shape {}
public static final record Shape$Rect(double w, double h) implements Shape {}

// payload なし variant は singleton（record ではない）
public static final class Shape$Point implements Shape {
    public static final Shape$Point INSTANCE = new Shape$Point();
    private Shape$Point() {}
}
```

**設計決定:**
- payload あり → `record`
- payload なし → `singleton class`（allocation 節約）
- Valen ABI と Java surface ABI は分離管理
- ABI 露出・命名規則は固定、将来変更しない

**要検証項目（実装時):**
- Java pattern switch との互換
- Jackson / Gson のシリアライゼーション
- reflection での class 名前解決
- Gradle incremental compilation
