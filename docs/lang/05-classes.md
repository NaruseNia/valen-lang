# 5. クラス

## 5.1 class

```valen
class User(pub name: String, mut age: Int) {
    fn greet(self) -> String {
        f"Hello, {self.name}!"
    }

    // associated function — self レシーバなし、呼び出しは `User::...`
    fn from_name(name: String) -> User {
        User(name = name, age = 0)
    }
}
```

**primary constructor:**

- 必須、パラメータは `pub` 有/無 × `mut` 有/無 の組み合わせ
- 結合順は `pub mut foo: T`（可視性先、`mut` 後）
- 無修飾 = **internal** field（同一パッケージ内で `self.name` として参照可、パッケージ外からは不可視）
- `pub name: String` — public 読み取り専用 field
- `pub mut age: Int` — public 可変 field
- `private name: String` — class 内部のみ参照可
- コンストラクタパラメータごとに `pub` / `internal` / `private` を個別に指定可

**method と associated function:**

class 本体に直接書くことも、`impl` ブロックで後付けすることもできる。

```valen
// class 本体内に定義
class Foo(pub x: Int) {
    fn bar(self) -> Int { self.x }
}

// inherent impl ブロックで後付け
impl Foo {
    fn baz(self) -> Int { self.x * 2 }
}
```

- `fn method(self) -> T { ... }` — instance method、`user.greet()` で呼ぶ
- `fn assoc(x: T) -> U { ... }`（`self` なし）— associated function、`User::assoc(x)` で呼ぶ
- `static` キーワードは存在しない。instance / associated の区別は `self` の有無のみ
- trait 実装は別書き：`impl Trait for User { ... }` （§7.2 参照）
- inherent impl 内の method は class 本体 method と同じ優先度で解決される

**final / open / abstract / sealed:**

- class はデフォルト final
- 継承させたいときは `open` / `abstract` / `sealed` を明示
- 推移はしない。`open class A` の下に `class B : A` を置くとき、`B` からさらに継承させるなら `open class B : A` と個別に opt-in が必要

## 5.2 derives 句

class / data class / enum は `derives(...)` 句でメソッドの自動生成を宣言できる。

```valen
class Foo(pub x: Int, pub y: Int) derives(Eq, Hash) {
    fn bar(self) -> Int { self.x + self.y }
}

data class Point(x: Float, y: Float) derives(Eq, Hash, Display, Clone);

enum Shape derives(Eq, Hash, Display) {
    Circle(r: Float),
    Rect(w: Float, h: Float),
    Point,
}
```

**構文:** `derives(Trait1, Trait2, ...)` — class/data class ではコンストラクタ引数と supertype の後、本体 `{` の前に置く。enum では名前（とジェネリクス）の後、本体 `{` の前に置く。

**利用可能な derive:**

| derive    | 生成メソッド | 備考 |
|-----------|------------|------|
| `Eq`      | `equals(Object) -> Boolean` | 全フィールドの構造比較 |
| `Hash`    | `hashCode() -> Int` | 31-multiply-accumulate アルゴリズム |
| `Display` | `toString() -> String` | `ClassName(field=value, ...)` 形式 |
| `Clone`   | `copy(...) -> Self` | 全フィールドを引数に取り新インスタンスを返す |

data class では `Eq`, `Hash`, `Display`, `Clone` は derives 句なしでも常に自動生成される。derives は class や enum の payload variant に対して明示的に指定する場合に使う。

## 5.3 data class

```valen
data class Point(x: Float, y: Float);
data class User(pub name: String, pub email: String);
```

- 自動生成：`equals` / `hashCode` / `toString` / `copy`
- **自動生成の対象は自身の primary constructor params のみ。** 親 class の state は含めない（Kotlin 同様）
- `data class` は常に **final**
- `open` / `abstract` / `sealed` を付与**できない**
- `data class` は **superclass になれない**（継承元として使えない）
- 構文上 supertype を `: SuperClass(args)` で指定可能だが、**現在の codegen では supertype 情報が HIR lowering 時に失われ、常に `java.lang.Object` を直接継承する**（既知の制限、将来修正予定）
- `impl Trait for DataClass` で trait 実装は可
- `impl DataClass { ... }` で inherent method を追加可

## 5.4 継承

単一 class 継承 + 複数 trait impl。

```valen
open class Animal(pub name: String) {
    open fn speak(self) -> String { "..." }
}

class Dog(pub name: String) : Animal(name) {
    override fn speak(self) -> String { "woof" }

    fn from_name(name: String) -> Dog {
        Dog(name = name)
    }
}

abstract class Shape {
    abstract fn area(self) -> Float;
}
```

**method の override:**

- `open fn` opt-in：`open class` 内であっても method はデフォルト final、`open fn` を明示したもののみ override できる
- `override fn` 必須：親 method / trait default method を上書きするときは `override fn` を書く、付け忘れはコンパイルエラー
- override 対象：「同一シグネチャを親 method または trait requirement として充足する」場合に限る

**abstract method:**

`abstract fn` は `;` で終端し body を持たない。body を付けるとコンパイルエラー (`ABSTRACT_METHOD_HAS_BODY`)。非 abstract method に body がない場合もエラー (`NON_ABSTRACT_MISSING_BODY`)。

**super 呼び出し:**

- `super.foo()` は **class 親の method のみ**
- trait default method を呼び出したいときは UFCS `Trait::foo(self)` を使う（`::` パス演算子経由）
- 単一 class 継承なので `super` に曖昧性はない

## 5.5 sealed class

```valen
sealed class Payment;

class Card(pub number: String) : Payment();
data class Cash : Payment();
```

- `sealed class` は **closed OOP hierarchy**（振る舞いの階層）
- 各 subtype は独自 state / method / trait impl を持てる
- **permit 対象は `class` と `data class`**（enum / trait / interface は permit 先にしない）
- **permit 範囲は同一 module**
- nested 記法（sealed class 本体に permit 先を書く）は廃止
- subtype は別ファイルでも書けるが、同一 module に属する必要がある

**bytecode マッピング:**

| Valen | JVM bytecode |
|-------|-------------|
| `sealed class Foo` | `abstract class Foo` + `PermittedSubclasses` attribute |
| `sealed trait Foo` | `interface Foo` (`abstract` + `interface`) + `PermittedSubclasses` attribute |

sealed class は JVM の sealed classes (JEP 360/397, JDK 17+) を利用し、`PermittedSubclasses` 属性で許可された子クラスを列挙する。sealed trait は interface として emit され、同様に `PermittedSubclasses` で実装先を制限する。

`enum` との使い分けは [§6. enum（ADT）](06-enum.md) を参照。

## 5.6 class body の制限

class body にはメソッド宣言のみ記述可能。AST には `ClassMember::Field` が定義されているが、**現在のパーサーは class body 内のフィールド宣言をパースしない**。フィールドはコンストラクタパラメータとしてのみ定義する。class body 内で method 以外のトークンに遭遇するとパースエラーになる。

## 5.7 メソッド解決規則

`value.foo(args)` を解決するとき、Valen コンパイラは次の手順で呼び出し先を決める。

1. **候補集合の形成** — `value` の名義型の class 本体 member、inherent impl の method、および in-scope な trait method のうち、receiver を調整した後に **名前と signature（arity / 型制約）が適用可能なもの** を集める
2. **class 本体優先** — class 本体に適用可能な member があれば、それを最優先で採用
3. **inherent impl** — class 本体に候補がない場合、inherent impl の method を探す
4. **trait 候補** — inherent impl にも候補がない場合、in-scope な trait method に落ちる
5. **曖昧性エラー** — trait 候補が複数あって一意に決まらない場合はコンパイルエラー

**曖昧性の解消**は UFCS で書く：

```valen
Trait::foo(value, args...)
```

`Class::foo(args...)` は associated function（`self` なし）の呼び出しに限る — class 本体の instance method を UFCS で呼ぶ必要があるときも `Class::method(value, ...)` ではなく `value.method(...)` を使う。

### override fn 必須条件

class 本体 method が `override fn` を要求されるのは以下の場合のみ：

- 親 class の `open fn` と**同一 signature**を持つ

**class 本体 method は trait を充足しない。** trait 充足は必ず `impl Trait for Type { ... }` ブロックで行う（§7.2）。class 本体に trait と同名・同シグネチャの method があっても、それは trait とは無関係な独立した method である。

signature が異なれば（arity や型制約で区別できれば）、同名でも `override` は不要：

```valen
trait ShowFmt { fn show(self, fmt: Fmt) -> String; }

class User(pub name: String) {
    // Show trait 実装ではない別シグネチャの show、override 不要
    fn show(self) -> String { self.name }
}

impl ShowFmt for User {
    fn show(self, fmt: Fmt) -> String { /* ... */ }
}

// u.show()    → class 本体 method
// u.show(fmt) → trait method
```

## 5.8 associated function と top-level fn の使い分け

**associated function は class 本体または inherent impl に、top-level fn はファイル直下に** 書く。両者は名前解決で暗黙合流しない：

- `parse(x)` → top-level fn 解決
- `User::parse(x)` → `User` の associated function 解決

**規範（strict でない推奨）:**

- **associated function に向く** — 型の private invariant / field に触れる構築系、canonical constructor、`from_*` / `parse` / `zero` / `default` 系ファクトリ
- **top-level fn に向く** — 複数の型に対称に振る舞うアルゴリズム、型所有を持たないユーティリティ、pure function

強制ではないため、コンパイルエラーにはしない（fmt / lint レベルで指摘する程度）。ただし、同一ファイルに `parse(s)` と `User::parse(s)` を並置するのは設計の赤信号。

## 5.9 制限

以下は現在サポートしていない：

- `init { ... }` ブロック
- セカンダリコンストラクタ（`constructor(...) { ... }` 相当）
- フィールドオーバーライド（`override val` 相当）
- ネストクラス / inner class
- class body 内のフィールド宣言（コンストラクタパラメータのみ）
- data class の supertype 継承の codegen（HIR lowering で情報が失われる）
