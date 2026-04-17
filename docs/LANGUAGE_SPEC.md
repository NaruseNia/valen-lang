# Valen Language Specification

Version: 0.1-draft (Codex 3巡レビュー後、哲学一本化スコア 82/100)
Last updated: 2026-04-17

---

## 0. 芯と哲学

Valen の芯は以下の4点。新機能提案はこの芯を補強するかで評価する。

1. **強 ADT**（sum type with payload）
2. **exhaustive match**（Rust フルセット）
3. **trait ベース抽象**（orphan rule 厳格）
4. **整合した失敗モデル**（Option / Result / Exception / panic の役割分離）

**芯の一文:** 「OO の上に ADT を足すのでなく、ADT を中核に据えて JVM へ落とす」

---

## 1. 字句構文

### 1.1 拡張子・エンコーディング
- ソースファイル：`.vln`
- エンコーディング：UTF-8
- 改行：LF / CRLF 両方受容、正規化はツール任意

### 1.2 キーワード
```
fn let mut self return
if else match
class data enum trait impl
pub internal private
open abstract sealed
package import
for in while loop
true false
as
```

予約語（将来用）：`suspend async await yield typealias`

### 1.3 識別子
- `[a-zA-Z_][a-zA-Z0-9_]*`
- 型は CamelCase 推奨、関数と変数は snake_case 推奨（強制なし）

### 1.4 コメント
```valen
// 単行コメント
/* ブロックコメント */
/// ドキュメントコメント（将来 rustdoc 相当）
```

### 1.5 セミコロン

- 文末は `;` 必須
- **ブロック末尾の式のみ `;` 省略で値返却**
- 教え方：「`;` は式を文として終える記号。ブロック末尾に置かない式は値になる」
- `if` / `match` / `block` はすべて式、`;` を付けると unit 化

---

## 2. 型

### 2.1 プリミティブ名義型

- `Int` — JVM 上の整数型に対応する名義型（実装詳細として int/Integer を切り替えるが仕様では保証しない）
- `Long`, `Float`, `Double`, `Char`, `Bool`, `Byte`, `Short`, `String`, `Unit`, `Nothing`

### 2.2 null / 欠損

Valen の欠損表現は **`Option<T>` に一本化**。

- `T?` は `Option<T>` の糖衣構文
- `T!` は **内部型のみ**（ユーザ記述不可、IDE 警告として表示のみ）
- `null` リテラルは使えない（Java 相互運用時にのみ経由）

### 2.3 ジェネリクス

- `<T>` 形式、宣言時に `in`/`out` variance 指定可
- erasure（JVM 互換）
- `reified` 型パラメータは Phase 2（MVP は普通のジェネリクス）

### 2.4 typealias

```valen
typealias UserId = Int;
```

**所有権を生まない** — orphan rule 判定上、typealias は元の型の所有として扱われない。

---

## 3. 式と文

### 3.1 式指向

すべてのブロックは式。

```valen
let x: Int = if y > 0 { y } else { -y };
let classify = match n {
    0 => "zero",
    1..=9 => "small",
    _ => "large",
};
```

### 3.2 ブロック

```valen
let result = {
    let a = compute_a();
    let b = compute_b();
    a + b  // ← ; なし、これがブロックの値
};
```

### 3.3 return

早期 return には `return expr;` を使う。ブロック末尾の式が関数の戻り値にもなる。

```valen
fn f(x: Int) -> Int {
    if x < 0 { return -x; };
    x * 2  // ← 末尾式
}
```

---

## 4. 関数

### 4.1 定義

```valen
fn add(a: Int, b: Int) -> Int {
    a + b
}
```

- トップレベル関数可
- 返り値が `Unit` なら `-> Unit` 省略可

### 4.2 名前付き引数（MVP）

```valen
fn greet(msg: String, count: Int) -> String { /* ... */ }

greet(msg = "hi", count = 3);
```

### 4.3 デフォルト引数（Phase 1.5）

```valen
// Phase 1.5 以降
fn greet(msg: String = "hi", count: Int = 1) -> String { /* ... */ }
```

MVP では overload で代替：

```valen
fn greet(msg: String, count: Int) -> String { /* ... */ }
fn greet(msg: String) -> String { greet(msg, 1) }
fn greet() -> String { greet("hi", 1) }
```

### 4.4 UFCS

メソッド記法 `xs.map(f)` が第一級、UFCS `map(xs, f)` は同一解決系の補助記法。

```valen
trait Mappable<T> {
    fn map<U>(self, f: fn(T) -> U) -> Mappable<U>;
}

// 両方とも同じ trait を解決
xs.map(|x| x * 2);
map(xs, |x| x * 2);  // UFCS
```

---

## 5. クラス

### 5.1 class

```valen
class User(name: String, mut age: Int);
```

- primary constructor 必須、パラメータは **無修飾子 + `mut` のみ**
- 無修飾 = `val` 相当の read-only フィールド
- `mut` = 可変フィールド
- デフォルト final、継承には `open` / `abstract` / `sealed` キーワード

### 5.2 data class

```valen
data class Point(x: Float, y: Float);
```

自動生成：`equals` / `hashCode` / `toString` / `copy`（MVP 必須）

### 5.3 継承

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

---

## 6. enum（ADT）

### 6.1 定義

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

### 6.2 バリアントアクセス

```valen
let s = Shape::Circle(r = 5.0);
let p = Shape::Point;
```

スコープ演算子 `::` で variant にアクセス。

### 6.3 Java bytecode 表現

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

---

## 7. trait / impl

### 7.1 trait 定義

```valen
trait Area {
    fn area(self) -> Float;
}

trait Display {
    fn display(self) -> String;
}
```

### 7.2 impl

```valen
// trait impl
impl Area for Shape {
    fn area(self) -> Float {
        match self {
            Shape::Circle(r) => 3.14159 * r * r,
            Shape::Rect(w, h) => w * h,
            Shape::Point => 0.0,
        }
    }
}

// inherent impl（trait 無し）
impl User {
    fn from_name(name: String) -> User {
        User(name = name, age = 0)
    }
}
```

### 7.3 レシーバ

- 明示 `self`（`fn f(self)`）
- 可変 `mut self`（`fn f(mut self)`）
- 借用 `&self` / `&mut self` は**導入しない**（所有権なし）

### 7.4 orphan rule / coherence

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

### 7.5 演算子オーバーロード

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

---

## 8. 失敗モデル

### 8.1 役割分離

| 機構 | 用途 |
|------|------|
| `Option<T>` | 値の欠如専用 |
| `Result<T, E>` | 回復可能失敗 |
| `panic` | 契約違反・到達不能・処理継続不能の停止機構 |
| Exception | FFI 境界の異常のみ |

**Valen 内で `throw` 文は禁止**。ドメイン失敗は Option / Result で表現、異常停止は `panic!` を使う。

### 8.2 `?` 演算子

- `Result<T, E>` 上で維持（`Ok(v) → v`、`Err(e) → early return Err(e)`）
- `Option<T>` 上は **戻り値が `Option<U>` の関数内のみ**で使える
- `Option → Result` 暗黙昇格は禁止

```valen
fn find_user(id: Int) -> Result<User, AppError> {
    let row = query(id)?;  // Result 上
    Ok(User::from_row(row))
}

fn first_char(s: String) -> Option<Char> {
    let c = s.chars().first()?;  // Option 上（関数の戻りも Option）
    Some(c.to_uppercase())
}
```

### 8.3 Java exception 境界

**自動ラップなし、明示変換**。

```valen
// 方針 A: unsafe fn で生呼び出し
unsafe fn read_raw(path: String) -> String {
    java.nio.file.Files.readString(java.nio.file.Paths.get(path))  // 例外は素通し
}

// 方針 B: safe ラッパで Result 包装
fn read_safe(path: String) -> Result<String, JavaException> {
    safe { java.nio.file.Files.readString(java.nio.file.Paths.get(path)) }
}

// 方針 C: @catch attribute で opt-in
@catch(IOException)
fn read(path: String) -> Result<String, IOException> {
    java.nio.file.Files.readString(java.nio.file.Paths.get(path))
}
```

MVP では B 方針（`safe { ... }` ブロック）を必須、A / C は Phase 1.5+ で検討。

---

## 9. パターンマッチ

### 9.1 フルセット

```valen
match value {
    0 => "zero",                          // リテラル
    1..=9 => "small",                     // 範囲
    10 | 20 | 30 => "round",              // or パターン
    n if n < 0 => "negative",             // ガード
    Shape::Circle(r) => f"circle r={r}",  // 構造分解
    Shape::Rect(w, h) => f"rect {w}x{h}", // 複数フィールド
    p @ User(name = "admin", ..) => admin_action(p),  // @束縛 + rest
    _ => "other",
}
```

### 9.2 exhaustive check

- Valen `enum` / `sealed class` hierarchy：**厳密 exhaustive**（非網羅はコンパイルエラー）
- Java 型：**`@closed` アノテーション付きのみ exhaustive**、他は普通の分岐（exhaustive check なし）

```valen
@closed
sealed interface Color  // Java 側定義

match color {
    Color.Red => ...,
    Color.Blue => ...,
    Color.Green => ...,  // 網羅しないとコンパイルエラー
}
```

---

## 10. 可視性・モジュール

### 10.1 package

```valen
package com.example.foo;

import java.util.HashMap;
import java.util.List;
```

- Java 風、ファイル先頭に package 宣言
- ファイルシステム階層と一致（Java と同様）

### 10.2 可視性修飾子

| 修飾子 | 意味 |
|--------|------|
| `pub` | 公開（どこからでも見える） |
| `internal` | 同一モジュール内 |
| `private` | declaration-private（クラス内・トップレベル内、Kotlin 流） |

デフォルトは `internal`（MVP）。

### 10.3 スコープ演算子

- `::` — enum variant、static-like member
- `.` — package path、type path、値メンバ

```valen
java.util.HashMap         // package path
Shape::Circle(r = 5.0)    // enum variant
User::from_name("Alice")  // static-like associated function
user.name                 // value member
```

---

## 11. 並行

### 11.1 MVP

MVP では **virtual thread (Loom) 一本**。

```valen
fn main() {
    let t = virtualThread { expensive_op() };
    t.join();
}
```

### 11.2 Phase 2+

`suspend fn` / async モデルは Phase 2 で再評価。MVP にはない。

---

## 12. 文字列

### 12.1 リテラル

```valen
let s = "hello";
let raw = r"raw\nstring";  // raw literal（\n は文字通り）
```

### 12.2 補間（MVP）

```valen
let name = "Alice";
let age = 30;
let msg = f"Hello, {name}! You are {age} years old.";
let expr = f"2 + 2 = {2 + 2}";
```

### 12.3 複数行（Phase 1.5）

```valen
// Phase 1.5 以降
let doc = f"""
    name: {name}
    age: {age}
""";
```

---

## 13. コレクション / for

### 13.1 MVP の扱い

`List` / `Map` / `Set` は **`java.util` の typealias**：

```valen
typealias List<T> = java.util.List<T>;
typealias Map<K, V> = java.util.Map<K, V>;
typealias Set<T> = java.util.Set<T>;
```

trait 注入は **iteration, map, filter 程度に最小化**（MVP）。

### 13.2 for-in

```valen
for x in xs {
    println(x);
};
```

- `Iterator` trait を実装した型に適用
- `java.lang.Iterable` は自動アダプト

### 13.3 Iterator trait

```valen
trait Iterator<T> {
    fn next(mut self) -> Option<T>;
}
```

---

## 14. メタプログラミング（MVP）

MVP は最小セット：

- `inline fn` は **Phase 1.5**、MVP は普通の fn のみ
- annotation consumption（読み取り）は Phase 1.5
- `reified` 型パラメータは Phase 2

MVP では annotation の宣言と読み取りは `java.lang.reflect` 経由で可能。

---

## 15. DSL / lambda

### 15.1 ラムダ

```valen
let double = |x: Int| x * 2;
let sum = |a: Int, b: Int| -> Int { a + b };
```

### 15.2 receiver lambda（Phase 1.5 以降）

MVP では receiver lambda (`T.() -> Unit`) を**提供しない**。Phase 1.5 で再評価。

---

## 16. ターゲット JVM

- **21 LTS baseline**：virtual thread、sealed、record、pattern matching for switch を活用
- **25 opt-in**：`--target 25` で Valhalla value types 等を有効化
- bytecode 直出力

---

## 17. サンプル

### 17.1 Hello, Valen

```valen
package com.example.hello;

fn main() {
    println("Hello, Valen!")
}
```

### 17.2 エラー処理

```valen
package com.example.app;

import java.util.List;

enum AppError {
    NotFound(id: Int),
    Invalid(reason: String),
}

fn find_positive(xs: List<Int>) -> Result<Int, AppError> {
    for x in xs {
        if x > 0 { return Ok(x); };
    };
    Err(AppError::NotFound(id = 0))
}

fn main() {
    let xs = List.of(-1, -2, 3, 4);
    match find_positive(xs) {
        Ok(n) => println(f"found: {n}"),
        Err(AppError::NotFound(id)) => println(f"not found, id={id}"),
        Err(AppError::Invalid(reason)) => println(f"invalid: {reason}"),
    };
}
```

### 17.3 trait + impl

```valen
trait Greet {
    fn greet(self) -> String;
}

data class Person(name: String);

impl Greet for Person {
    fn greet(self) -> String {
        f"Hello, {self.name}!"
    }
}

fn main() {
    let p = Person(name = "Alice");
    println(p.greet());   // メソッド記法（第一級）
    println(greet(p));    // UFCS（補助記法）
}
```

---

## 18. 今後の仕様課題

実装時に詰める項目：

1. **enum bytecode ABI の実験検証**
   - Java pattern switch
   - Jackson / Gson シリアライゼーション
   - reflection での class 名前解決
   - Gradle incremental compilation
2. **coherence 仕様補則**
   - 所有単位の厳密化（compile unit = module = package の関係）
   - generic nominal type の所有判定例
   - inherent vs trait 衝突時の曖昧性エラー規則
3. **Java overload resolution 規則**
   - Int vs int vs Integer の優先度
   - null 許容位置

---

## 19. ライセンス

Apache License 2.0（仕様・実装とも）
