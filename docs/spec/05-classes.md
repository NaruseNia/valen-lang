# 5. クラス

## 5.1 class

```valen
class User(name: String, mut age: Int);
```

- primary constructor 必須、パラメータは **無修飾子 + `mut` のみ**
- 無修飾 = `val` 相当の read-only フィールド
- `mut` = 可変フィールド
- デフォルト final、継承には `open` / `abstract` / `sealed` キーワード

## 5.2 data class

```valen
data class Point(x: Float, y: Float);
```

自動生成：`equals` / `hashCode` / `toString` / `copy`（MVP 必須）

## 5.3 継承

```valen
open class Animal(name: String);

class Dog(name: String) : Animal(name);

abstract class Shape {
    abstract fn area(self) -> Float;
}

sealed class Payment {
    class Card(number: String) : Payment();
    class Cash : Payment();
}
```
