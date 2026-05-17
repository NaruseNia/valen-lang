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

`impl` は trait 実装と inherent impl（型固有メソッド追加）の2形式をサポートする。

### trait impl

`impl Trait for Type { ... }` で trait のメソッドを型に実装する。

### inherent impl

`impl Type { ... }` で型に直接メソッドを追加する。class body でのメソッド定義と同等だが、型定義の外に書ける。enum / data class にメソッドを追加する主要な手段。

```valen
impl Vec2 {
    fn length(self) -> Float { ... }
    fn scale(self, factor: Float) -> Vec2 { ... }
}
```

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

- `Trait` が現在の **module** で定義されている
- **または** `Type` の outermost nominal type constructor が現在の **module** に所有されている

**所有単位は module** — package / module / compile unit の三者同一視は行わない。

**stdlib 例外:** `valen.core` および `valen.std.*` パッケージからの impl は、foreign trait を foreign type に実装できる（Java コレクション連携用）。ユーザーコードには適用されない。

- `package` は source 階層と名前空間（§10.1）
- `module` はビルドターゲット内の意味的所有単位（§10.2）。orphan rule / `sealed permit` 範囲 / `internal` 可視性はすべて module ID に従う
- `compile unit` は物理的な単位で、仕様には現れない（実装側で決まる）

詳細は [§10.2 module](10-modules.md) を参照。

**禁止:**
- foreign trait for foreign type（例：`impl java.util.List for java.lang.String` 不可）
- typealias を介した所有権回避（`type MyList = java.util.List<Int>` に対する impl 不可）
- blanket impl（`impl<T: Foo> Bar for T`）は MVP 全面禁止、std 限定

**一意性:**
- 同一 trait/type 対に対する impl はグローバル一意
- downstream module での再定義禁止

**trait 充足ルール:**
- trait method の実装は **`impl Trait for Type { ... }` ブロック内でのみ** 成立する
- class 本体の method が trait の method と同名・同シグネチャであっても、trait 充足にはならない（両者は独立）
- class 本体に同名 method がある場合、`value.foo()` は class 本体 method を優先解決する

**衝突解決:**
- class 本体 member（method / associated function）が適用可能なら最優先
- trait method の候補が複数で曖昧になる場合は UFCS `Trait::foo(value, args)` で解決
- 詳細なメソッド解決規則は [§5.6](05-classes.md) を参照

## 7.5 sealed trait

`sealed trait` は trait の実装集合を閉じ、exhaustive match を許す。

```valen
sealed trait Expr {
    fn eval(self) -> Int;
}

class Lit {}
class Add {}

impl Expr for Lit {
    fn eval(self) -> Int { 0 }
}

impl Expr for Add {
    fn eval(self) -> Int { 1 }
}

fn process(e: Expr) -> Int {
    match e {
        Lit => 0,
        Add => 1,
    }
}
```

**制約:**
- 実装者は `class` と `data class` のみ（enum は不可）
- 実装者の宣言は `impl SealedTrait for Type { ... }`（trait の一貫性を維持）
- permit 範囲は同一コンパイル単位（将来的に module スコープへ移行）
- default method は非対応（通常 trait と同じ制約）
- supertrait は非対応

**JVM ABI:** sealed interface（`ACC_INTERFACE | ACC_ABSTRACT` + `PermittedSubclasses` attribute）として emit。

**exhaustive check:** enum / sealed class と同様に厳密 exhaustive。実装者が1つでも不足するとコンパイルエラー。wildcard `_` で回避可能。

## 7.6 Associated Type

trait 内で `type Name;` と宣言すると、impl 側で具体型を決定する associated type を定義できる。

```valen
trait Container {
    type Item;
    fn get(self, index: Int) -> Self::Item;
}

impl Container for IntList {
    type Item = Int;
    fn get(self, index: Int) -> Int { /* ... */ }
}
```

- `Self::Output` のように `Self::` で参照
- impl ごとに一意に解決される
- trait 定義側でデフォルト型を指定可能: `type Item = Int;`

## 7.7 derives（自動 trait 実装）

`derives(Trait1, Trait2)` 節を型宣言に付けると、指定した trait の実装がフィールド構造から自動生成される。

```valen
pub data class Entity(pub id: Int) derives(Eq, Hash);

pub enum Color derives(Eq, Hash, Display) {
    Red,
    Green,
    Blue(value: Int),
}

pub class Point(pub x: Float, pub y: Float) derives(Eq) {}
```

### 対応 trait

| trait | 生成メソッド | 動作 |
|-------|------------|------|
| `Eq` | `equals(Object) -> boolean` | フィールド逐次比較 |
| `Hash` | `hashCode() -> int` | 31-multiply-accumulate |
| `Display` | `toString() -> String` | `TypeName(field=value, ...)` 形式 |
| `Clone` | `copy(fields...) -> Self` | 全フィールド指定のコピーコンストラクタ |

### data class の暗黙 derives

`data class` は宣言するだけで `Eq`, `Hash`, `Display`, `Clone` の実装を自動生成する（`derives(...)` を書かなくても生成される）。明示的に `derives(Eq)` と書いても冗長なだけで害はない。

### enum の derives

enum に `derives(Eq)` を付けると、**フィールドを持つ variant ごとに** `equals` メソッドが生成される。unit variant は singleton なので参照比較のみ。

## 7.8 演算子オーバーロード（Phase 1.5 実装済み）

trait ベースの演算子オーバーロード。prelude に定義された演算子 trait を impl することで有効化する。

### 算術演算子

| 演算子 | trait | メソッド |
|--------|-------|---------|
| `+` | `Add<Rhs>` | `fn add(self, rhs: Rhs) -> Self::Output` |
| `-` | `Sub<Rhs>` | `fn sub(self, rhs: Rhs) -> Self::Output` |
| `*` | `Mul<Rhs>` | `fn mul(self, rhs: Rhs) -> Self::Output` |
| `/` | `Div<Rhs>` | `fn div(self, rhs: Rhs) -> Self::Output` |
| `%` | `Rem<Rhs>` | `fn rem(self, rhs: Rhs) -> Self::Output` |

各 trait は `type Output` associated type を持つ。

```valen
impl Add<Vec2> for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2(x = self.x + rhs.x, y = self.y + rhs.y)
    }
}
```

### 単項演算子

| 演算子 | trait | メソッド |
|--------|-------|---------|
| `-x` | `Neg` | `fn neg(self) -> Self::Output` |
| `!x` | `Not` | `fn not(self) -> Self::Output` |

### 比較演算子

| 演算子 | trait | メソッド |
|--------|-------|---------|
| `<` `<=` `>` `>=` | `Ord` | `fn cmp(self, rhs: Self) -> Int` |

`cmp` の戻り値: 負 → `<`、0 → `==`、正 → `>`。

### 等値比較（opt-in）

| 演算子 | trait | メソッド |
|--------|-------|---------|
| `==` `!=` | `Eq` | `fn eq(self, rhs: Self) -> Bool` |

- `impl Eq` がある型 → `Eq::eq` を使用
- `impl Eq` がない型 → 従来通り `.equals()` にフォールバック
- プリミティブ型の `==` は組み込み処理（trait 不要）

### プリミティブ型

`Int`、`Float` 等のプリミティブ型の演算子は組み込みで直接処理される（`iadd` / `fadd` 等）。プリミティブ型に対して演算子 trait を impl する必要はない。
