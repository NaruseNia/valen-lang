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
