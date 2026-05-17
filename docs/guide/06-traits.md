# trait と impl

Valen の trait は型に振る舞いを追加するための仕組みです。Rust の trait に近い構文ですが、所有権・借用はありません。

## trait の定義

`trait` キーワードで振る舞いの契約（インターフェース）を定義します。

```valen
trait Area {
    fn area(self) -> Float;
}

trait Display {
    fn display(self) -> String;
}
```

trait には1つ以上のメソッドシグネチャを書きます。メソッドの本体は書かず、実装は `impl` ブロックで行います。

## impl による trait 実装

`impl Trait for Type` の構文で、特定の型に trait を実装します。

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

impl Display for Shape {
    fn display(self) -> String {
        match self {
            Shape::Circle(r) => f"Circle(r={r})",
            Shape::Rect(w, h) => f"Rect({w}x{h})",
            Shape::Point => "Point",
        }
    }
}
```

### inherent impl はない

Valen には Rust のような `impl Type { ... }`（inherent impl）はありません。型自身のメソッドは class 本体に直接書きます。`impl` ブロックは trait 実装専用です。

```valen
class User(pub name: String, mut age: Int) {
    // メソッドは class 本体に直接書く
    fn greet(self) -> String {
        f"Hello, {self.name}!"
    }

    // associated function（self なし）
    fn from_name(name: String) -> User {
        User(name = name, age = 0)
    }
}

// trait 実装は impl ブロックで
impl Display for User {
    fn display(self) -> String {
        f"User({self.name}, age={self.age})"
    }
}
```

## レシーバ

trait メソッドのレシーバ（第一引数）は以下の2種類です。

| レシーバ | 意味 |
|---------|------|
| `self` | 不変のインスタンスを受け取る |
| `mut self` | 可変のインスタンスを受け取る |

Rust の `&self` / `&mut self` は存在しません。Valen には所有権・借用の概念がないためです。

```valen
trait Counter {
    fn count(self) -> Int;
    fn increment(mut self);
}

impl Counter for ClickTracker {
    fn count(self) -> Int {
        self.clicks
    }

    fn increment(mut self) {
        self.clicks = self.clicks + 1;
    }
}
```

## orphan rule（孤児ルール）

`impl Trait for Type` を書けるのは、以下のいずれかを満たす場合に限られます。

- **`Trait` が現在のモジュールで定義されている**
- **`Type` の最外の名義型コンストラクタが現在のモジュールで定義されている**

つまり、自分のモジュールが所有していない trait と型の組み合わせに対して実装を書くことはできません。

```valen
// OK: 自分の型に外部 trait を実装
impl Display for MyType {
    fn display(self) -> String { "..." }
}

// OK: 自分の trait を外部型に実装
impl MyTrait for String {
    fn check(self) -> Bool { true }
}

// NG: 両方とも外部 → コンパイルエラー
// impl Display for String { ... }
```

### 禁止されるパターン

- **foreign trait for foreign type**: 両方が外部のモジュールで定義されている組み合わせ
- **typealias を介した所有権回避**: `type MyList = java.util.List<Int>` に対する impl は不可
- **blanket impl**: `impl<T: Foo> Bar for T` は MVP では全面禁止

## UFCS（Universal Function Call Syntax）

複数の trait が同じ名前のメソッドを持つ場合、通常の `.` 呼び出しでは曖昧になることがあります。UFCS で明示的に trait を指定して解決します。

```valen
trait Japanese {
    fn hello(self) -> String;
}

trait English {
    fn hello(self) -> String;
}

impl Japanese for Greeter {
    fn hello(self) -> String { "こんにちは" }
}

impl English for Greeter {
    fn hello(self) -> String { "Hello" }
}

// g.hello() はどちらか曖昧 → コンパイルエラー

// UFCS で解決
let jp = Japanese::hello(g);   // "こんにちは"
let en = English::hello(g);    // "Hello"
```

UFCS の構文は `Trait::method(receiver, args...)` です。レシーバを第一引数として明示的に渡します。

## メソッド解決順序

`value.foo(args)` が呼ばれたとき、コンパイラは以下の優先順位で解決します。

1. **class 本体のメソッド** — class 本体に適用可能なメソッドがあれば最優先
2. **trait メソッド** — class 本体に候補がない場合、in-scope な trait メソッドを探す
3. **曖昧ならエラー** — trait 候補が複数あって一意に決まらない場合はコンパイルエラー

```valen
trait Printable {
    fn show(self) -> String;
}

class Item(pub name: String) {
    // class 本体メソッド
    fn show(self) -> String {
        f"Item: {self.name}"
    }
}

impl Printable for Item {
    fn show(self) -> String {
        f"[Printable] {self.name}"
    }
}

let item = Item(name = "book");
item.show();              // "Item: book" — class 本体が優先
Printable::show(item);    // "[Printable] book" — UFCS で trait 側を呼ぶ
```

**重要:** class 本体のメソッドと trait メソッドが同名・同シグネチャであっても、class 本体のメソッドは trait を充足しません。trait の実装は必ず `impl Trait for Type { ... }` ブロック内で行う必要があります。

## sealed trait

`sealed trait` は trait の実装を閉じた集合に限定し、exhaustive match を可能にします。

```valen
sealed trait Expr {
    fn eval(self) -> Int;
}

class Lit(pub value: Int) {}
class Add(pub left: Expr, pub right: Expr) {}

impl Expr for Lit {
    fn eval(self) -> Int { self.value }
}

impl Expr for Add {
    fn eval(self) -> Int {
        self.left.eval() + self.right.eval()
    }
}
```

sealed trait の実装者は同一コンパイル単位に属する必要があります。実装者は `class` と `data class` のみで、enum は実装者になれません。

sealed trait を match に使うと、enum と同じように網羅性検査が適用されます。

```valen
fn describe(e: Expr) -> String {
    match e {
        Lit(value) => f"リテラル: {value}",
        Add(_, _) => "加算式",
        // Lit と Add を網羅しているので OK
    }
}
```

sealed trait は「振る舞いのインターフェースを持ちつつ、実装を閉じたい」場合に使います。enum との違いは、各実装者が独自のメソッドや状態を持てる点です。

## Associated Type

trait 内で `type Name;` と宣言すると、impl 側で具体型を決める「型の穴」を定義できます。

```valen
trait Container {
    type Item;
    fn get(self, index: Int) -> Self::Item;
}

impl Container for IntList {
    type Item = Int;
    fn get(self, index: Int) -> Int {
        // ...
    }
}
```

`Self::Item` は impl ごとに一意に解決されるため、呼び出し側で型注釈なしに型推論が働きます。

## 演算子オーバーロード

Valen の演算子オーバーロードは trait ベースです。`+` を使いたい型に `impl Add for MyType` を書きます。

### 算術演算子

```valen
data class Vec2(pub x: Float, pub y: Float);

impl Add<Vec2> for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2(x = self.x + rhs.x, y = self.y + rhs.y)
    }
}

// これで + が使える
let a = Vec2(x = 1.0, y = 2.0);
let b = Vec2(x = 3.0, y = 4.0);
let c = a + b;  // Vec2(x = 4.0, y = 6.0)
```

対応する trait は以下の通りです。

| 演算子 | trait | メソッド |
|--------|-------|---------|
| `+` | `Add<Rhs>` | `fn add(self, rhs: Rhs) -> Self::Output` |
| `-` | `Sub<Rhs>` | `fn sub(self, rhs: Rhs) -> Self::Output` |
| `*` | `Mul<Rhs>` | `fn mul(self, rhs: Rhs) -> Self::Output` |
| `/` | `Div<Rhs>` | `fn div(self, rhs: Rhs) -> Self::Output` |
| `%` | `Rem<Rhs>` | `fn rem(self, rhs: Rhs) -> Self::Output` |

各 trait は `type Output` associated type を持ちます。戻り値の型を impl で指定できます。

### 単項演算子

| 演算子 | trait | メソッド |
|--------|-------|---------|
| `-x` (符号反転) | `Neg` | `fn neg(self) -> Self::Output` |
| `!x` (論理否定) | `Not` | `fn not(self) -> Self::Output` |

### 比較演算子

`<` `<=` `>` `>=` を使いたい場合は `Ord` trait を実装します。

```valen
impl Ord for Priority {
    fn cmp(self, rhs: Priority) -> Int {
        self.level - rhs.level
    }
}

// cmp が負 → <, 0 → ==, 正 → >
if taskA < taskB { /* ... */ }
```

### 等値比較（opt-in）

`==` / `!=` はデフォルトで `.equals()` に変換されます。カスタムの等値比較が必要な場合は `Eq` trait を実装できます。

```valen
impl Eq for CaseInsensitiveString {
    fn eq(self, rhs: CaseInsensitiveString) -> Bool {
        self.value.toLowerCase() == rhs.value.toLowerCase()
    }
}
```

`impl Eq` がある型では `Eq::eq` が使われ、ない型では従来通り `.equals()` にフォールバックします。

### プリミティブ型

`Int`、`Float` 等のプリミティブ型の演算子は組み込みで処理されます（trait 経由ではありません）。プリミティブ型に対して演算子 trait を impl する必要はありません。

## derives — trait の自動実装

`derives(Trait1, Trait2)` を型宣言に付けると、trait の実装がフィールド構造から自動生成されます。

```valen
pub data class Entity(pub id: Int) derives(Eq, Hash);

pub enum Shape derives(Eq, Hash, Debug) {
    Circle(r: Float),
    Rect(w: Float, h: Float),
    Point,
}
```

### 対応 trait

| trait | 何が生成される？ |
|-------|----------------|
| `Eq` | `equals` — フィールドを1つずつ比較 |
| `Hash` | `hashCode` — フィールドから一意のハッシュ値を計算 |
| `Debug` | `toString` — `TypeName(field=value)` 形式の文字列表現 |
| `Clone` | `copy` — 全フィールドを指定して複製 |

### data class は暗黙に derive 済み

`data class` は `derives(...)` を書かなくても `Eq`, `Hash`, `Debug`, `Clone` が自動生成されます。これは data class の設計意図（値型としての振る舞い）に基づいています。

```valen
// derives を書かなくても equals/hashCode/toString/copy が使える
pub data class Point(pub x: Float, pub y: Float);
```
