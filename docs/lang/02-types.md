# 2. 型

## 2.1 プリミティブ名義型

Valen のプリミティブ型は JVM プリミティブに対応する名義型。型チェッカ内部では `Ty::Prim(PrimTy)` として表現される。

- `Int` — JVM `int` / `java.lang.Integer`
- `Long` — JVM `long` / `java.lang.Long`
- `Float` — JVM `float` / `java.lang.Float`
- `Double` — JVM `double` / `java.lang.Double`
- `Bool` — JVM `boolean` / `java.lang.Boolean`
- `Char` — JVM `char` / `java.lang.Character`
- `Byte` — JVM `byte` / `java.lang.Byte`
- `Short` — JVM `short` / `java.lang.Short`
- `String` — JVM `java.lang.String`
- `Unit` — JVM `void`（値位置では `()` リテラル）
- `Nothing` — ボトム型（全型のサブタイプ、関数が正常復帰しないことを示す）

### リテラルのデフォルト型

| リテラル | 型 | 例 |
|----------|------|------|
| 整数 | `Int` | `42`, `0xFF`, `0b1010`, `0o77` |
| 整数 + `L` サフィックス | `Long` | `42L`, `0xFFL`, `0b1010L`, `0o77L` |
| 浮動小数点 | `Double` | `3.14` |
| 浮動小数点 + `f` サフィックス | `Float` | `3.14f` |
| 文字列 | `String` | `"hello"` |
| 文字 | `Char` | `'a'`, `'\n'`, `'\t'` |
| 真偽値 | `Bool` | `true`, `false` |
| ユニット | `Unit` | `()` |

### 数値変換

**暗黙の数値変換は一切行わない。** 全ての数値型変換は明示メソッドで行う。

```valen
let x: Long = 42.toLong();      // OK
let y: Long = 42;               // ERROR: type mismatch, Int != Long
let z: Double = 3.14f.toDouble(); // OK
let w: Float = 42.toFloat();    // OK
```

#### 変換メソッド一覧（stdlib `core.vln` 定義）

各型で利用可能なメソッドは以下の通り。**存在しないメソッドを呼ぶとコンパイルエラー。**

| 型 | 利用可能な変換メソッド |
|------|----------------------|
| `Int` | `.toLong()`, `.toFloat()`, `.toDouble()` |
| `Long` | `.toInt()`, `.toFloat()`, `.toDouble()` |
| `Float` | `.toInt()`, `.toLong()`, `.toDouble()` |
| `Double` | `.toInt()`, `.toLong()`, `.toFloat()` |
| `Byte` | `.toInt()`, `.toLong()`, `.toFloat()`, `.toDouble()`, `.toShort()` |
| `Short` | `.toInt()`, `.toLong()`, `.toFloat()`, `.toDouble()`, `.toByte()` |
| `Char` | `.toInt()`, `.toLong()`, `.toFloat()`, `.toDouble()` |

> **注意:** `.toChar()` は現在どの型にも定義されていない。`.toByte()` は `Short` のみ、`.toShort()` は `Byte` のみで利用可能。

**根拠:** 暗黙 widening は Java/Kotlin で微妙なバグ源。オーバーロード解決を劇的にシンプルにする（型完全一致のみ候補）。

### 数値キャスト（`as`）

明示的な `as` キャストも数値間で利用可能。数値型同士の `as` キャストは全て safe（`unsafe` ブロック不要）。`Char` から数値型へのキャストも safe。

```valen
let x = 42 as Long;    // OK (safe)
let c = 'A' as Int;    // OK (safe)
```

## 2.2 等値比較

| 演算子 | セマンティクス | 脱糖先 |
|--------|---------------|--------|
| `==` | 構造比較 | `.equals()` 呼び出し |
| `!=` | 構造不等 | `!.equals()` 呼び出し |
| `===` | 参照比較 | JVM 参照一致 |
| `!==` | 参照不等 | JVM 参照不一致 |

```valen
let a = "hello";
let b = "hello";
a == b    // true (構造比較)
a === b   // true or false (JVM string interning 依存)
```

## 2.3 Nullable 型（`T?`）

### `T?` と `Option<T>` は別の型

Valen には欠損表現が **2 種類** ある。

| 型 | 内部表現 | 用途 |
|------|----------|------|
| `T?` | `Ty::Nullable(Box<Ty>)` | JVM null を許容する型。Java interop で主に使用 |
| `Option<T>` | `enum Option<T> { Some(T), None }` | Valen ネイティブの欠損表現。ADT |

**`T?` は `Option<T>` の糖衣構文ではない。** 両者は型システム内で完全に別の型として扱われる。

### `T?` の JVM マッピング

`T?` は JVM 上ではボックス化されたプリミティブ（参照型）にマッピングされる。

| Valen 型 | JVM 型 |
|-----------|--------|
| `Int?` | `java/lang/Integer` |
| `Long?` | `java/lang/Long` |
| `Float?` | `java/lang/Float` |
| `Double?` | `java/lang/Double` |
| `Bool?` | `java/lang/Boolean` |
| `Char?` | `java/lang/Character` |
| `Byte?` | `java/lang/Byte` |
| `Short?` | `java/lang/Short` |
| `String?` | `java/lang/String`（元から参照型のため変化なし） |

### `?` 演算子

`?`（try 演算子）は **`Option<T>` と `Result<T, E>`** にのみ適用可能。**`T?`（Nullable）には使えない。**

```valen
fn get_value() -> Option<Int> {
    Option::Some(42)
}

fn example() -> Option<Int> {
    let v = get_value()?;  // OK: Option<Int> に ? 適用
    Option::Some(v + 1)
}
```

`?` 演算子使用時、関数の戻り値型は対象と同じラッパー型でなければならない:
- `Option<T>` に `?` → 関数は `Option<..>` を返す必要あり
- `Result<T, E>` に `?` → 関数は `Result<..>` を返す必要あり

### `null` リテラル

`null` リテラルは `unsafe` ブロック内でのみ使用可能。型は `Nothing?`（nullable bottom type）で、任意の `T?` に代入できる。通常の Valen コードでは `null` は使えず、値の不在は `Option<T>` で表現する。

```valen
unsafe {
    let x: String? = null;  // OK
}
let y: String? = null;  // ERROR: null outside unsafe
```

### `T!`（プラットフォーム型）

**未実装（将来検討）。** ユーザーコードでは記述不可。

## 2.4 ジェネリクス

- `<T>` 形式、宣言時に `in`/`out` variance 注釈を構文上は記述可能
- 非 reified 型パラメータは erasure（JVM 互換）
- `reified` 型パラメータは `inline fn` 内で使用可能（§4.8 参照）

### Variance 注釈（`in`/`out`）

**構文上は受理されるが、現在の型チェッカでは意味的効果なし。** パーサが `Variance::Covariant`（`out`）/ `Variance::Contravariant`（`in`）として解析し保持するが、型チェック時に variance 制約の検証は行われない。将来のフェーズで強制する予定。

```valen
// 構文上は有効だが、variance は現在強制されない
class Box<out T>(val value: T) {}
trait Consumer<in T> {
    fn accept(self, item: T) -> Unit;
}
```

### reified 型パラメータ

`reified T` はジェネリクスパラメータに付与する修飾子で、コールサイトで具体型に置換される。JVM の型消去を回避し、実行時に型情報を利用できる。

**制約:**
- `inline fn` 内でのみ使用可能（非 inline fn では `reified` 使用時コンパイルエラー）
- class、data class、enum、trait の型パラメータには使えない（コンパイルエラー）
- 同一関数内で reified と非 reified の型パラメータを混在可能

**構文:**
```valen
inline fn <reified T> isInstance(value: Any) -> Bool {
    value is T
}

inline fn <reified T, U> mixed(value: Any, other: U) -> Bool {
    value is T  // T は reified → OK
    // value is U はコンパイルエラー（U は非 reified）
}
```

**reified T で許可される操作:**

| 操作 | 構文 | JVM codegen | 実装状態 |
|------|------|-------------|----------|
| 型チェック | `value is T` | `instanceof ConcreteType` | ✅ 実装済 |
| キャスト | `value as T` | `checkcast ConcreteType` | ✅ 実装済 |
| クラス取得 | `T::class` | `ldc ConcreteType.class` | ❌ **未実装** |

**Java interop:** Java 側からの呼び出し時は `reified` が無効になり、通常の型消去が適用される。

### 式位置での明示的型引数

コンストラクタ・関数・メソッド呼び出しで型引数を明示的に指定できる。

```valen
let list = ArrayList<String>();           // コンストラクタ
let map = HashMap<String, Int>();
let nested = HashMap<String, List<Int>>(); // ネストも可
let x = parse<Int>("42");                 // 関数呼び出し
let items = iter(list).collect<List<String>>(); // メソッド呼び出し
```

型引数は省略可能（型推論が十分な場合）。`ArrayList()` と `ArrayList<String>()` は両方有効。

## 2.5 サブタイプ規則

型チェッカ内部の `is_subtype` 関数が以下の規則を判定する:

| 規則 | 説明 |
|------|------|
| 反射律 | `T` は `T` のサブタイプ |
| Any | 全ての型は `Any` のサブタイプ |
| Nullable | `T` は `T?` のサブタイプ |
| Nothing | `Nothing` は全ての型のサブタイプ |
| TypeParam | 具体型は `TypeParam` にマッチする（コールサイト解決） |

```valen
let x: Any = 42;           // Int → Any（暗黙アップキャスト）
let y: Int? = 42;           // Int → Int?（暗黙アップキャスト）
```

ダウンキャストは `as` で明示的に行い、`unsafe` コンテキストが必要。

## 2.6 typealias

```valen
typealias UserId = Int;
typealias StringList = List<String>;
```

ジェネリック typealias も可能:
```valen
typealias Pair<A, B> = Pair<A, B>;
```

**所有権を生まない** — orphan rule 判定上、typealias は元の型の所有として扱われない。

> **注意:** coherence チェック（orphan rule）は実装済みだが、typealias 展開後のターゲット型に基づいて判定される。typealias 自体は orphan rule 上の所有権を持たない。

## 2.7 newtype

`newtype` は内部型をラップする新しい独立した型を作成する。`typealias` と異なり、orphan rule で自モジュール所有の型として扱われる。

```valen
newtype EntityId = Int;
newtype ComponentName = String;

let eid = EntityId(42);           // コンストラクタ構文
```

- コンストラクタ `TypeName(value)` でラップ（型チェッカで引数の型を検証済み）
- JVM 上では `value` フィールドと `value()` getter メソッドを持つ final class として生成される
- `impl Eq for EntityId { ... }` が可能（自モジュール所有型のため）

### `.value()` メソッド

codegen レベルでは `value()` getter メソッドが JVM bytecode に生成される。ただし **型チェッカレベルでの `.value()` メソッド解決は未実装**。現状、codegen が生成する `value()` メソッドに対して型チェッカがメソッド呼び出しを認識しないため、型チェック時にエラーになる可能性がある。

## 2.8 Any 型

`Any` は `java.lang.Object` に対応するトップ型。全ての型は暗黙に `Any` のサブタイプであり、アップキャストは暗黙に行われる。

```valen
let x: Any = 42;           // Int → Any（暗黙アップキャスト）
let y: Any = "hello";      // String → Any
fn accept(value: Any) {}   // 任意の型を受け取る
```

- `Any` へのアップキャストは暗黙（boxing される）
- ダウンキャストは `unsafe` コンテキスト内で `as` キャストにより行う

## 2.9 `ref mut T` — ミュータブル参照型

`ref mut T` は `T` への可変参照を表す型。`Ty::RefMut(Box<Ty>)` として表現され、`T` とは別の型であり暗黙変換は存在しない。

### 操作

| 構文 | 意味 |
|------|------|
| `ref mut expr` | 可変参照の作成 |
| `*r` | 参照の読み取り（deref） |
| `*r = expr` | 参照先への書き込み |

```valen
fn increment(x: ref mut Int) -> Unit {
    *x = *x + 1;
}

let mut n = 10;
increment(ref mut n);
// n == 11
```

### Lambda での使用

Lambda 内で外側の変数を変更するには、明示的に `ref mut` で参照を作成してキャプチャする。自動キャプチャは行わない。

```valen
let mut count = 0;
let r = ref mut count;
let inc = || { *r = *r + 1; };
```

### JVM 実装

プリミティブ型は専用ラッパークラス、オブジェクト型はジェネリッククラスで実装する。

**注意:** `ref mut Byte`、`ref mut Short`、`ref mut Char` は全て `valen/core/IntRef` にマッピングされる（`Int` と共有）。

| Valen 型 | JVM クラス | 備考 |
|-----------|-----------|------|
| `ref mut Int` | `valen/core/IntRef` | |
| `ref mut Byte` | `valen/core/IntRef` | Int と共有 |
| `ref mut Short` | `valen/core/IntRef` | Int と共有 |
| `ref mut Char` | `valen/core/IntRef` | Int と共有 |
| `ref mut Long` | `valen/core/LongRef` | |
| `ref mut Float` | `valen/core/FloatRef` | |
| `ref mut Double` | `valen/core/DoubleRef` | |
| `ref mut Bool` | `valen/core/BoolRef` | |
| `ref mut T`（object） | `valen/core/Ref` | |

### Java interop

`ref mut T` は Valen 内部専用。Java メソッドに `ref mut T` を渡すとコンパイルエラーになる。

## 2.10 `safe {}` ブロック

`safe {}` ブロック内の式は Java 例外をキャッチし、`Result<T, JavaException>` として返す。

```valen
let result = safe {
    riskyOperation()
};
// result: Result<ReturnType, JavaException>
```

`JavaException` は stdlib で以下のように定義:

```valen
pub data class JavaException(
    pub message: String,
    pub class_name: String,
);
```

`JavaException` は `Error` trait を実装する。

## 2.11 Data Class の暗黙 trait 充足

`data class` は以下の trait 境界を **暗黙に充足する**（明示的な `derives(...)` や `impl` は不要）:

- `Eq`
- `Hash`
- `Display`
- `Clone`

```valen
data class Point(pub x: Int, pub y: Int);

fn <T: Eq> compare(a: T, b: T) -> Bool {
    a.eq(b)
}

compare(Point(1, 2), Point(1, 2));  // OK: Point は暗黙に Eq を充足
```

型チェッカの trait 充足判定（`check_type_satisfies_trait`）で、対象型が `DefKind::DataClass` であれば `Eq`/`Hash`/`Display`/`Clone` の境界を自動的に満たすと判定する。

## 2.12 Tuple 型

`Type::Tuple` は AST 上で構文的に予約されている（`(A, B, C)` 形式）が、HIR への lowering 時に `Ty::Error` に変換される。**現在使用不可（将来予約）。**

```valen
// let t: (Int, String) = (42, "hello");  // コンパイルエラー: Tuple 型は未実装
```

代替として `data class` や stdlib の `Pair<A, B>` を使用する:

```valen
let p = Pair(42, "hello");  // Pair<Int, String>
```

## 2.13 stdlib 型一覧

stdlib `core.vln` で定義される主要な型:

### ADT

| 型 | 定義 |
|------|------|
| `Option<T>` | `enum { Some(value: T), None }` |
| `Result<T, E>` | `enum { Ok(value: T), Err(error: E) }` |
| `Ordering` | `enum { Less, Equal, Greater }` |

### Data Class

| 型 | フィールド |
|------|----------|
| `Pair<A, B>` | `first: A`, `second: B` |
| `Range<T>` | `start: T`, `end: T`, `inclusive: Bool` |
| `JavaException` | `message: String`, `class_name: String` |

### コレクション typealias

| Valen 型 | Java 型 |
|-----------|---------|
| `List<T>` | `java.util.List<T>` |
| `Map<K, V>` | `java.util.Map<K, V>` |
| `Set<T>` | `java.util.Set<T>` |

### Trait

| trait | メソッド |
|-------|---------|
| `Eq` | `fn eq(self, other: Self) -> Bool` |
| `Hash` | `fn hash(self) -> Int` |
| `Display` | `fn display(self) -> String` |
| `Clone` | `fn clone(self) -> Self` |
| `Error` | `fn message(self) -> String` |
| `Iterator<T>` | `next`, `map`, `filter`, `fold`, `collect`, `forEach`, `count`, `any`, `all`, `find` |
| `Into<T>` | `fn into(self) -> T` |
| `From<T>` | `fn from(value: T) -> Self` |
| `TryInto<T>` | `fn tryInto(self) -> Result<T, String>` |
| `TryFrom<T>` | `fn tryFrom(value: T) -> Result<Self, String>` |
| `Default` | `fn default() -> Self` |
| `IntoIterator<T>` | `fn intoIter(self) -> Iterator<T>` |
| `Index<Idx>` | `fn index(self, idx: Idx) -> Self` |
| `ToString` | `fn toString(self) -> String` |
| `Ord` | `fn cmp(self, rhs: Self) -> Int` |

### 演算子 trait

| trait | メソッド |
|-------|---------|
| `Add<Rhs>` | `fn add(self, rhs: Rhs) -> Self` |
| `Sub<Rhs>` | `fn sub(self, rhs: Rhs) -> Self` |
| `Mul<Rhs>` | `fn mul(self, rhs: Rhs) -> Self` |
| `Div<Rhs>` | `fn div(self, rhs: Rhs) -> Self` |
| `Rem<Rhs>` | `fn rem(self, rhs: Rhs) -> Self` |
| `Neg` | `fn neg(self) -> Self` |
| `Not` | `fn not(self) -> Self` |
