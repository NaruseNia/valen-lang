# 型システム

## プリミティブ名義型

Valen は以下のプリミティブ名義型を持っています。

| 型 | 説明 | 例 |
|------|------|------|
| `Int` | 整数 | `42` |
| `Long` | 長整数 | `42L` |
| `Float` | 単精度浮動小数点 | `3.14f` |
| `Double` | 倍精度浮動小数点 | `3.14` |
| `Char` | 文字 | `'a'` |
| `Bool` | 真偽値 | `true`, `false` |
| `Byte` | バイト | — |
| `Short` | 短整数 | — |
| `String` | 文字列 | `"hello"` |
| `Unit` | 値なし（Java の `void` に相当） | — |
| `Nothing` | 到達不能型（常に異常終了する関数の戻り値型） | — |

これらは JVM 上のプリミティブ型やラッパー型に対応しますが、Valen の仕様としてはそれを意識する必要はありません。

## リテラルのデフォルト型

数値リテラルのデフォルト型は以下のルールで決まります。

```valen
let a = 42;       // Int
let b = 42L;      // Long（L サフィックス）
let c = 3.14;     // Double
let d = 3.14f;    // Float（f サフィックス）
let e = true;     // Bool
let f = "hello";  // String
```

Java や Kotlin と異なり、サフィックスなしの整数は常に `Int`、サフィックスなしの小数は常に `Double` です。`Long` や `Float` が必要な場合はサフィックスを付けてください。

### 16 進・2 進・8 進リテラル

整数リテラルは 10 進のほか、プレフィクスで基数を指定できます。アンダースコア (`_`) で桁区切りも可能です。

```valen
let hex = 0xFF;          // Int: 255
let bin = 0b1010;        // Int: 10
let oct = 0o77;          // Int: 63
let hex_long = 0xFFL;    // Long: 255
let grouped = 0xFF_FF;   // Int: 65535
```

## 数値変換 — 暗黙変換は一切なし

Valen では **暗黙の数値変換を一切行いません**。Java/Kotlin では `int` から `long` への代入が暗黙に行われますが、Valen ではコンパイルエラーになります。

```valen
let x: Long = 42;              // ERROR: type mismatch, Int != Long
let y: Long = 42.toLong();     // OK: 明示変換
let z: Double = 3.14f.toDouble(); // OK: Float → Double の明示変換
let w: Float = 42.toFloat();   // OK: Int → Float の明示変換
```

利用できる変換メソッドは以下の通りです。

- `.toInt()`
- `.toLong()`
- `.toFloat()`
- `.toDouble()`
- `.toByte()`
- `.toShort()`
- `.toChar()`

**なぜ暗黙変換がないのか:**  暗黙の widening は Java/Kotlin で微妙なバグの原因になります。また、暗黙変換を排除するとオーバーロード解決が大幅にシンプルになります（型が完全一致する候補のみ選ばれる）。明示変換はコード量が少し増えますが、数値型の不一致が常にコンパイル時に検出されるメリットがあります。

## 等値比較

Valen の比較演算子は、Kotlin と同様に構造比較と参照比較を分けています。

| 演算子 | 意味 | Java での相当 |
|--------|------|---------------|
| `==` | 構造比較（`.equals()` 呼び出し） | `Objects.equals(a, b)` |
| `!=` | 構造不等 | `!Objects.equals(a, b)` |
| `===` | 参照比較 | `a == b`（参照の `==`） |
| `!==` | 参照不等 | `a != b`（参照の `!=`） |

```valen
let a = "hello";
let b = "hello";
a == b     // true — 文字列の内容が同じ
a === b    // JVM の string interning に依存、結果は不定
```

Java 開発者の方へ: Valen の `==` は Java の `.equals()` に相当します。参照比較が必要な場面（稀）では `===` を使ってください。

## null と欠損値 — Option に一本化

Valen には `null` リテラルがありません。値が存在しない可能性を表現するには `Option<T>` を使います。

```valen
let found: Option<Int> = Some(42);
let missing: Option<Int> = None;
```

### T? は Option<T> の糖衣構文

型の末尾に `?` を付けると `Option<T>` の省略記法として使えます。

```valen
fn find_user(id: Int) -> User? {
    // User? は Option<User> と同じ
    if id == 1 {
        Some(User(name = "Alice"))
    } else {
        None
    }
}
```

### Option の使い方

`Option` の中身を取り出すには `match` を使います。

```valen
let result = find_user(1);

match result {
    Some(user) => println(f"Found: {user.name}"),
    None => println("User not found"),
}
```

**Java 開発者の方へ:** `Option<T>` は Java の `Optional<T>` に似ていますが、Valen では型システムに深く統合されています。`null` が型を汚染することはなく、`Option` の中身を取り出すには必ず `match` でチェックする必要があるため、NullPointerException に相当するバグが構造的に防止されます。

### null リテラルは使えない

```valen
let x: String = null;   // ERROR: null リテラルは存在しない
let y: String? = null;  // ERROR: None を使ってください
let z: String? = None;  // OK
```

Java ライブラリとの相互運用時に Java 側から `null` が来る場合は、FFI 境界で `Option` に変換されます。

## 型推論

Valen はローカル変数の型を推論しますが、関数シグネチャでは型を明示する必要があります。

```valen
// ローカル変数: 型推論が働く
let x = 42;              // x: Int
let msg = f"count: {x}"; // msg: String

// 関数シグネチャ: パラメータ型と戻り値型は明示必須
fn add(a: Int, b: Int) -> Int {
    a + b   // 関数ボディ内は推論が働く
}
```

型推論が十分な情報を持たない場合は、型注釈を付けてください。

```valen
let items: List<Int> = List();  // ジェネリクスの型パラメータが推論できない場合
```

## typealias

既存の型に別名を付けることができます。新しい型は生成されません。

```valen
typealias UserId = Int;
typealias Handler = fn(String) -> Result<Unit, AppError>;
```

`typealias` は単なる別名なので、`UserId` と `Int` は完全に互換です。orphan rule の判定上、`typealias` は元の型の所有権を持ちません（`impl Trait for UserId` で foreign trait を実装することはできません）。

## `ref mut T` — ミュータブル参照

`ref mut T` は変数への可変参照を作る型です。関数に渡した先で呼び出し元の変数を変更したい場合や、Lambda で外側の変数を書き換えたい場合に使います。

### 基本的な使い方

`ref mut expr` で参照を作り、`*r` で読み取り、`*r = expr` で書き込みます。

```valen
fn increment(x: ref mut Int) -> Unit {
    *x = *x + 1;
}

let mut n = 10;
increment(ref mut n);
// n は 11 になっている
```

### Lambda でのキャプチャ

Lambda 内で外側の変数を変更するには、事前に `ref mut` で参照を作成します。自動キャプチャは行われないため、参照の作成は常に明示的です。

```valen
let mut count = 0;
let r = ref mut count;
let inc = || { *r = *r + 1; };
inc();
// count は 1 になっている
```

### 注意事項

- `ref mut T` と `T` は別の型です。暗黙変換はありません
- Java メソッドに `ref mut T` を渡すことはできません（コンパイルエラー）
- GC がメモリ管理するため Rust のようなダングリング参照は発生しません。ただし aliasing と data race の責任はプログラマにあります

## 次のステップ

- [ジェネリクス](03-generics.md) — 型パラメータ、bounds、variance
- [クラスとデータクラス](04-classes.md) — class, data class, 継承
