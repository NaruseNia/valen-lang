# 7. trait / impl

## 7.1 trait 定義

```valen
trait Area {
    fn area(self) -> Float;
}

trait Display {
    fn display(self) -> String;
}
```

## 7.2 impl

`impl` は **trait 実装専用**。inherent impl block（`impl Type { ... }`）は存在しない — class の instance method / associated function は class 本体に直接書く（§5.1 参照）。

```valen
impl Area for Shape {
    fn area(self) -> Float {
        match self {
            Shape::Circle(r) => 3.14159 * r * r,
            Shape::Rect(w, h) => w * h,
            Shape::Point => 0.0,
        }
    }
}
```

## 7.3 レシーバ

- 明示 `self`（`fn f(self)`）
- 可変 `mut self`（`fn f(mut self)`）
- 借用 `&self` / `&mut self` は**導入しない**（所有権なし）

## 7.4 orphan rule / coherence

`impl Trait for Type` を許可する条件：

- `Trait` が現在のコンパイル単位で定義されている
- **または** `Type` の outermost nominal type constructor が現在の compile unit に所有されている

所有単位：compile unit（MVP では package = module = compile unit として扱う）

**禁止:**
- foreign trait for foreign type（例：`impl java.util.List for java.lang.String` 不可）
- typealias を介した所有権回避（`type MyList = java.util.List<Int>` に対する impl 不可）
- blanket impl（`impl<T: Foo> Bar for T`）は MVP 全面禁止、std 限定

**一意性:**
- 同一 trait/type 対に対する impl はグローバル一意
- downstream module での再定義禁止

**衝突解決:**
- inherent impl 優先
- trait impl 同士の衝突はコンパイルエラー、明示的な `as Trait` キャストが必要

## 7.5 演算子オーバーロード

Phase 1.5+。trait ベース：

```valen
trait Add<Rhs> {
    type Output;
    fn add(self, rhs: Rhs) -> Self::Output;
}

impl Add<Vec2> for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2(x = self.x + rhs.x, y = self.y + rhs.y)
    }
}
```
