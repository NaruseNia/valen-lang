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
- 無修飾 = private field（class 内部のみ `self.name` で参照可、外部からは不可視）
- `pub name: String` — public 読み取り専用 field
- `pub mut age: Int` — public 可変 field
- `internal` / `private` の個別指定は **Phase 1.5+**（MVP では `pub` のみ許可）

**method と associated function:**

- class 本体に直接書く。`impl Class { ... }` という inherent impl block は**存在しない**
- `fn method(self) -> T { ... }` — instance method、`user.greet()` で呼ぶ
- `fn assoc(x: T) -> U { ... }`（`self` なし）— associated function、`User::assoc(x)` で呼ぶ
- `static` キーワードは存在しない。instance / associated の区別は `self` の有無のみ
- trait 実装だけが別書き：`impl Trait for User { ... }` （§7.2 参照）

**final / open / abstract / sealed:**

- class はデフォルト final
- 継承させたいときは `open` / `abstract` / `sealed` を明示
- 推移はしない。`open class A` の下に `class B : A` を置くとき、`B` からさらに継承させるなら `open class B : A` と個別に opt-in が必要

## 5.2 data class

```valen
data class Point(x: Float, y: Float);
data class User(pub name: String, pub email: String);
```

- 自動生成：`equals` / `hashCode` / `toString` / `copy`（MVP 必須）
- `data class` は常に **final**
- `open` / `abstract` / `sealed` を付与**できない**
- `data class` は **superclass になれない**（継承元として使えない）
- ただし `sealed` / `open` / `abstract` superclass を継承することは可能（sealed permit の葉として使える）
- `impl Trait for DataClass` で trait 実装は可

## 5.3 継承

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
    abstract fn area(self) -> Float
}
```

**method の override:**

- `open fn` opt-in：`open class` 内であっても method はデフォルト final、`open fn` を明示したもののみ override できる
- `override fn` 必須：親 method / trait default method を上書きするときは `override fn` を書く、付け忘れはコンパイルエラー
- override 対象：「同一シグネチャを親 method または trait requirement として充足する」場合に限る

**super 呼び出し:**

- `super.foo()` は **class 親の method のみ**
- trait default method を呼び出したいときは UFCS `Trait::foo(self)` を使う（`::` パス演算子経由）
- 単一 class 継承なので `super` に曖昧性はない

## 5.4 sealed class

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

`enum` との使い分けは [§6. enum（ADT）](06-enum.md) を参照。

## 5.6 メソッド解決規則

`value.foo(args)` を解決するとき、Valen コンパイラは次の手順で呼び出し先を決める。

1. **候補集合の形成** — `value` の名義型の class 本体 member および in-scope な trait method のうち、receiver を調整した後に **名前と signature（arity / 型制約）が適用可能なもの** を集める
2. **class 本体優先** — class 本体に適用可能な member があれば、それを最優先で採用
3. **trait 候補** — class 本体に候補がない場合、in-scope な trait method に落ちる
4. **曖昧性エラー** — trait 候補が複数あって一意に決まらない場合はコンパイルエラー

**曖昧性の解消**は UFCS で書く：

```valen
Trait::foo(value, args...)
```

`Class::foo(args...)` は associated function（`self` なし）の呼び出しに限る — class 本体の instance method を UFCS で呼ぶ必要があるときも `Class::method(value, ...)` ではなく `value.method(...)` を使う。

### override fn 必須条件

class 本体 method が `override fn` を要求されるのは以下のいずれか：

- 親 class の `open fn` と**同一 signature**を持つ
- class が implement している trait の requirement（abstract method または default method）と**同一 signature**を持つ

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

## 5.7 associated function と top-level fn の使い分け

**associated function は class 本体に、top-level fn はファイル直下に** 書く。両者は名前解決で暗黙合流しない：

- `parse(x)` → top-level fn 解決
- `User::parse(x)` → `User` の associated function 解決

**規範（strict でない推奨）:**

- **associated function に向く** — 型の private invariant / field に触れる構築系、canonical constructor、`from_*` / `parse` / `zero` / `default` 系ファクトリ
- **top-level fn に向く** — 複数の型に対称に振る舞うアルゴリズム、型所有を持たないユーティリティ、pure function

強制ではないため、コンパイルエラーにはしない（fmt / lint レベルで指摘する程度）。ただし、同一ファイルに `parse(s)` と `User::parse(s)` を並置するのは設計の赤信号。

## 5.8 MVP 除外（Phase 1.5+ 送り）

- `init { ... }` ブロック
- secondary constructor（`constructor(...) { ... }` 相当）
- field override（`override val` 相当）
- `sealed trait`
- nested / inner class
- primary constructor param の `internal` / `private` 個別指定
- annotation 宣言構文／annotation 読み取り API
