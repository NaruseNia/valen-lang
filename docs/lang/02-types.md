# 2. 型

## 2.1 プリミティブ名義型

- `Int` — JVM 上の整数型に対応する名義型（実装詳細として int/Integer を切り替えるが仕様では保証しない）
- `Long`, `Float`, `Double`, `Char`, `Bool`, `Byte`, `Short`, `String`, `Unit`, `Nothing`

### リテラルのデフォルト型

| リテラル | 型 | 例 |
|----------|------|------|
| 整数 | `Int` | `42` |
| 整数 + `L` サフィックス | `Long` | `42L` |
| 浮動小数点 | `Double` | `3.14` |
| 浮動小数点 + `f` サフィックス | `Float` | `3.14f` |
| 文字列 | `String` | `"hello"` |
| 真偽値 | `Bool` | `true`, `false` |

### 数値変換

**暗黙の数値変換は一切行わない。** 全ての数値型変換は明示メソッドで行う。

```valen
let x: Long = 42.toLong();      // OK
let y: Long = 42;               // ERROR: type mismatch, Int != Long
let z: Double = 3.14f.toDouble(); // OK
let w: Float = 42.toFloat();    // OK
```

変換メソッド: `.toInt()`, `.toLong()`, `.toFloat()`, `.toDouble()`, `.toByte()`, `.toShort()`, `.toChar()`

**根拠:** 暗黙 widening は Java/Kotlin で微妙なバグ源。オーバーロード解決を劇的にシンプルにする（型完全一致のみ候補）。

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

## 2.3 null / 欠損

Valen の欠損表現は **`Option<T>` に一本化**。

- `T?` は `Option<T>` の糖衣構文
- `T!` は **内部型のみ**（ユーザ記述不可、IDE 警告として表示のみ）
- `null` リテラルは使えない（Java 相互運用時にのみ経由）

## 2.4 ジェネリクス

- `<T>` 形式、宣言時に `in`/`out` variance 指定可
- erasure（JVM 互換）
- `reified` 型パラメータは Phase 2（MVP は普通のジェネリクス）

## 2.5 typealias

```valen
typealias UserId = Int;
```

**所有権を生まない** — orphan rule 判定上、typealias は元の型の所有として扱われない。

## 2.6 newtype

`newtype` は内部型をラップする新しい独立した型を作成する。`typealias` と異なり、orphan rule で自モジュール所有の型として扱われる。

```valen
newtype EntityId = Int;
newtype ComponentName = String;

let eid = EntityId(42);           // コンストラクタ構文
```

- コンストラクタ `TypeName(value)` でラップ
- `.value()` メソッドでアンラップ
- JVM 上では単一フィールド `value` を持つ class として表現
- `impl Eq for EntityId { ... }` が可能（自モジュール所有型のため）

## 2.7 Any 型

`Any` は `java.lang.Object` に対応するトップ型。全ての型は暗黙に `Any` のサブタイプであり、アップキャストは暗黙に行われる。

```valen
let x: Any = 42;           // Int → Any（暗黙アップキャスト）
let y: Any = "hello";      // String → Any
fn accept(value: Any) {}   // 任意の型を受け取る
```

- `Any` へのアップキャストは暗黙（boxing される）
- ダウンキャストは `unsafe`（VEP-001）または `is` チェック（Phase 3）

## 2.8 `ref mut T` — ミュータブル参照型

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

| Valen 型 | JVM クラス | フィールド |
|-----------|-----------|----------|
| `ref mut Int` | `valen/core/IntRef` | `int value` |
| `ref mut Long` | `valen/core/LongRef` | `long value` |
| `ref mut Float` | `valen/core/FloatRef` | `float value` |
| `ref mut Double` | `valen/core/DoubleRef` | `double value` |
| `ref mut Bool` | `valen/core/BoolRef` | `boolean value` |
| `ref mut T`（object） | `valen/core/Ref<T>` | `Object value` |

### Java interop

`ref mut T` は Valen 内部専用。Java メソッドに `ref mut T` を渡すとコンパイルエラーになる。
