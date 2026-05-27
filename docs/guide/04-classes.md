# クラスとデータクラス

## class の基本

Valen の class は primary constructor を持ちます。コンストラクタのパラメータは class 名の直後の括弧に書きます。

```valen
class User(pub name: String, mut age: Int) {
    fn greet(self) -> String {
        f"Hello, {self.name}!"
    }
}
```

### primary constructor のパラメータ修飾

パラメータには `pub` と `mut` の組み合わせで可視性と可変性を制御します。

| 修飾 | 外部からの読み取り | 外部からの書き換え | 内部での変更 |
|------|--------------------|--------------------|--------------|
| 無修飾 | 不可 | 不可 | 不可 |
| `pub` | 可能 | 不可 | 不可 |
| `mut` | 不可 | 不可 | 可能 |
| `pub mut` | 可能 | 可能 | 可能 |

記述順序は `pub mut`（可視性が先、`mut` が後）です。

```valen
class Config(
    pub name: String,          // public 読み取り専用
    pub mut retries: Int,      // public 読み書き可能
    mut internal_state: Int,   // private 可変
    secret: String,            // private 不変
) {
    fn update(mut self) {
        self.internal_state = self.internal_state + 1;
    }
}
```

```valen
let config = Config(
    name = "app",
    retries = 3,
    internal_state = 0,
    secret = "key",
);

println(config.name);       // OK: pub なので読める
config.retries = 5;         // OK: pub mut なので書ける
// config.secret            // ERROR: private field
```

### インスタンスの生成

`new` キーワードはありません。class 名を関数のように呼び出します。名前付き引数を使えます。

```valen
let user = User(name = "Alice", age = 30);
```

## メソッドと associated function

class のボディには直接メソッドを書きます。Rust の `impl Type { ... }` ブロックは存在しません。

### メソッド（instance method）

第一引数に `self` を取る関数がメソッドです。ドット記法で呼び出します。

```valen
class Counter(pub mut count: Int) {
    fn increment(mut self) {
        self.count = self.count + 1;
    }

    fn current(self) -> Int {
        self.count
    }
}

let mut c = Counter(count = 0);
c.increment();
println(c.current());  // 1
```

- `self` — 不変レシーバ（フィールドの読み取りのみ）
- `mut self` — 可変レシーバ（フィールドの変更が可能）

### associated function

`self` を取らない関数は associated function です。`Class::function()` の形式で呼び出します。`static` キーワードは存在しません。

```valen
class User(pub name: String, mut age: Int) {
    // associated function — self なし
    fn from_name(name: String) -> User {
        User(name = name, age = 0)
    }

    // メソッド — self あり
    fn greet(self) -> String {
        f"Hello, {self.name}!"
    }
}

let user = User::from_name("Bob");  // associated function の呼び出し
println(user.greet());               // メソッドの呼び出し
```

Java 開発者の方へ: associated function は Java の `static` メソッドに相当しますが、Valen では `static` キーワードを使わず、`self` の有無だけで区別します。

## data class

`data class` は値を保持するためのクラスです。以下のメソッドが自動生成されます。

- `equals` — 全フィールドの構造比較
- `hashCode` — 全フィールドのハッシュ値
- `toString` — `ClassName(field1=value1, field2=value2)` 形式
- `copy` — 一部フィールドを変更したコピーの生成

```valen
data class Point(pub x: Float, pub y: Float);

data class User(pub name: String, pub email: String);
```

```valen
let p1 = Point(x = 1.0f, y = 2.0f);
let p2 = Point(x = 1.0f, y = 2.0f);

p1 == p2        // true — 構造比較（equals）
println(p1);    // Point(x=1.0, y=2.0) — toString

let p3 = p1.copy(x = 3.0f);  // x だけ変更したコピー
println(p3);    // Point(x=3.0, y=2.0)
```

### data class の制約

- data class は常に **final** です。`open` や `abstract` を付けることはできません
- data class は他の class の親になれません（superclass として使えない）
- ただし、`sealed` / `open` / `abstract` な class を継承することは可能です
- trait の実装（`impl Trait for DataClass`）は可能です

```valen
// OK: sealed class のサブタイプとして data class を使う
sealed class Shape;

data class Circle(pub r: Float) : Shape();
data class Rect(pub w: Float, pub h: Float) : Shape();
```

## 継承

Valen の class はデフォルトで **final** です。継承させるには明示的に `open`、`abstract`、`sealed` のいずれかを指定する必要があります。

### open class

```valen
open class Animal(pub name: String) {
    open fn speak(self) -> String {
        "..."
    }
}

class Dog(pub name: String) : Animal(name) {
    override fn speak(self) -> String {
        "woof"
    }
}
```

**重要なポイント:**

- class の継承は `: ParentClass(args)` で指定します
- メソッドもデフォルトで final です。override させたいメソッドには `open fn` を指定します
- サブクラスで override するときは `override fn` の記述が必須です（付け忘れはコンパイルエラー）
- `open` は推移しません。`Dog` からさらに継承させたい場合は `open class Dog` と個別に指定が必要です

### abstract class

実体を持たないメソッドを定義できます。

```valen
abstract class Shape {
    abstract fn area(self) -> Float;

    fn describe(self) -> String {
        f"area = {self.area()}"
    }
}

class Circle(pub r: Float) : Shape() {
    override fn area(self) -> Float {
        3.14159 * self.r * self.r
    }
}
```

- `abstract fn` はボディを持たず、サブクラスでの実装が必須です
- abstract class はインスタンス化できません

### override fn の必須条件

`override fn` が必要なのは、親 class の `open fn` と同一シグネチャを持つ場合のみです。

```valen
open class Base {
    open fn process(self) -> String { "base" }
    fn helper(self) -> Int { 42 }  // open でない → override 不可
}

class Derived : Base() {
    override fn process(self) -> String { "derived" }
    // fn helper(self) -> Int { 0 }  // ERROR: helper は open でない
}
```

### super 呼び出し

サブクラスから親 class のメソッドを呼び出すには `super.method()` を使います。

```valen
open class Animal(pub name: String) {
    open fn speak(self) -> String {
        f"I am {self.name}"
    }
}

class Dog(pub name: String) : Animal(name) {
    override fn speak(self) -> String {
        let base = super.speak();
        f"{base}, woof!"
    }
}
```

**注意:** `super` は class の親メソッドのみに使えます。trait メソッドを明示的に呼び出したい場合は UFCS（`Trait::method(self)`）を使います。

## sealed class

`sealed class` は閉じたクラス階層を定義します。サブタイプの集合が固定されるため、`match` で exhaustive（網羅的）なパターンマッチが可能になります。

```valen
sealed class Payment;

class Card(pub number: String, pub expiry: String) : Payment();
data class Cash(pub amount: Int) : Payment();
class BankTransfer(pub account: String) : Payment();
```

```valen
fn describe(payment: Payment) -> String {
    match payment {
        Card(number, _) => f"Card ending in {number}",
        Cash(amount) => f"Cash: {amount}",
        BankTransfer(account) => f"Transfer to {account}",
    }
    // すべてのサブタイプを網羅 → _ パターン不要
}
```

### sealed class の制約

- サブタイプは同一モジュール内で定義する必要があります（別ファイルでも可）
- サブタイプになれるのは `class` と `data class` です
- 各サブタイプは独自のフィールド、メソッド、trait 実装を持てます

### enum との使い分け

`enum` と `sealed class` はどちらも閉じた型の集合を表現しますが、用途が異なります。

| | enum | sealed class |
|------|------|-------------|
| サブタイプごとの独自メソッド | 不可 | 可能 |
| サブタイプごとの trait 実装 | 不可 | 可能 |
| サブタイプからの継承 | 不可 | 可能（open/abstract なら） |
| ユースケース | データの分類（ADT） | 振る舞いの階層 |

```valen
// enum が適切: 純粋なデータの分類
enum Color {
    Red,
    Green,
    Blue,
    Custom(r: Int, g: Int, b: Int),
}

// sealed class が適切: サブタイプごとに異なる振る舞いが必要
sealed class Widget;
class Button(pub label: String) : Widget() {
    fn click(self) { /* ... */ }
}
class TextField(pub value: String) : Widget() {
    fn clear(mut self) { self.value = ""; }
}
```

## デフォルト引数

class のコンストラクタパラメータにもデフォルト値を指定できます。

```valen
class HttpClient(
    pub base_url: String,
    pub mut timeout: Int = 30,
    pub mut retries: Int = 3,
) {
    fn get(self, path: String) -> String {
        // ...
    }
}

let client = HttpClient(base_url = "https://api.example.com");
// timeout = 30, retries = 3 がデフォルトで適用される

let custom = HttpClient(
    base_url = "https://api.example.com",
    timeout = 60,
);
// retries = 3 のみデフォルト適用
```

名前付き引数と組み合わせることで、中間のパラメータを省略して末尾のパラメータだけ指定することもできます。
