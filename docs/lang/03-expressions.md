# 3. 式と文

## 3.1 式指向

すべてのブロックは式。

```valen
let x: Int = if y > 0 { y } else { -y };
let classify = match n {
    0 => "zero",
    1..=9 => "small",
    _ => "large",
};
```

## 3.2 ブロック

```valen
let result = {
    let a = compute_a();
    let b = compute_b();
    a + b  // ← ; なし、これがブロックの値
};
```

### セミコロン省略規則

ブロック状の式（`if`, `if let`, `match`, `for`, `while`, `while let`, `loop`, `safe`）は、文の位置で末尾 `;` を省略できる。パーサが自動的に `ExprSemi` として処理する。

```valen
fn example() {
    if x > 0 {
        do_something();
    }                // ← ; 不要

    match n {
        0 => a(),
        _ => b(),
    }                // ← ; 不要

    for i in 0..10 {
        process(i);
    }                // ← ; 不要

    let y = x + 1;  // ← 通常の文は ; 必須
}
```

## 3.3 return

早期 return には `return expr;` を使う。ブロック末尾の式が関数の戻り値にもなる。

```valen
fn f(x: Int) -> Int {
    if x < 0 { return -x }  // statement position、; 省略
    x * 2                    // ← 末尾式、値として返る
}
```

ブロック式をそのまま戻り値にすることもできる：

```valen
fn abs(x: Int) -> Int {
    if x < 0 { -x } else { x }  // if 式が関数の値
}
```

## 3.4 演算子の優先順位

下から上へ優先度が高くなる。同一レベルは特記なければ左結合。

| レベル | 演算子 | 結合性 | 説明 |
|--------|--------|--------|------|
| 1 | `=` `+=` `-=` `*=` `/=` `%=` | 右結合 | 代入・複合代入 |
| 2 | `\|>` | 左結合 | パイプライン |
| 3 | `\|\|` | 左結合 | 論理 OR |
| 4 | `&&` | 左結合 | 論理 AND |
| 5 | `\|` | 左結合 | ビット OR |
| 6 | `^` | 左結合 | ビット XOR |
| 7 | `&` | 左結合 | ビット AND |
| 8 | `==` `!=` `===` `!==` | 左結合 | 等値・参照等値 |
| 9 | `<` `<=` `>` `>=` | 左結合 | 比較 |
| 10 | `..` `..=` | — | 範囲（非結合） |
| 11 | `<<` `>>` | 左結合 | ビットシフト |
| 12 | `+` `-` | 左結合 | 加減算 |
| 13 | `*` `/` `%` | 左結合 | 乗除算・剰余 |
| 14 | `-` `!` `*` | — | 単項前置（否定・論理NOT・参照外し） |
| 15 | `.field` `.method()` `()` `?` `as Type` | 左結合 | 後置（フィールド・メソッド呼出・関数呼出・try・キャスト） |

## 3.5 代入式

### 単純代入

`target = value` で変数やフィールドに値を代入する。代入は式だが、値は `Unit`。

```valen
let mut x = 0;
x = 42;
obj.field = value;
```

### 複合代入

`+=` `-=` `*=` `/=` `%=` は対応する二項演算と代入を組み合わせた略記。

```valen
let mut n = 10;
n += 5;   // n = n + 5
n -= 3;   // n = n - 3
n *= 2;   // n = n * 2
n /= 4;   // n = n / 4
n %= 3;   // n = n % 3
```

## 3.6 算術・比較演算子

標準の算術演算子 `+` `-` `*` `/` `%` と比較演算子 `<` `<=` `>` `>=` `==` `!=` を提供する。

```valen
let sum = a + b;
let is_positive = x > 0;
let equal = a == b;
```

## 3.7 ビット演算子

整数型に対するビット演算。

| 演算子 | 説明 |
|--------|------|
| `&` | ビット AND |
| `\|` | ビット OR |
| `^` | ビット XOR |
| `<<` | 左シフト |
| `>>` | 右シフト |

```valen
let flags = 0b1010 & 0b1100;   // 0b1000
let combined = a | b;
let flipped = x ^ 0xFF;
let shifted = n << 2;
```

## 3.8 参照等値演算子

`===` と `!==` はオブジェクトの**同一性**（identity）を比較する。JVM 上の参照比較に対応。

```valen
let a = create_obj();
let b = a;
let c = create_obj();

a === b;  // true — 同一オブジェクト
a === c;  // false — 別オブジェクト
a !== c;  // true
```

`==` / `!=` は**構造的等値**（`equals`）、`===` / `!==` は**参照等値**（同一参照か）。

## 3.9 論理演算子

`&&`（論理 AND）と `||`（論理 OR）は短絡評価。

```valen
if x > 0 && y > 0 {
    // 両方正
}
if a || b {
    // どちらか true
}
```

`!` は単項論理否定。

```valen
if !is_valid {
    return;
}
```

## 3.10 範囲式

`start..end`（排他）と `start..=end`（包含）で範囲を生成する。`for` ループだけでなく、独立した式としても使用可能。

```valen
// for ループ内
for i in 0..10 {
    println(f"{i}");
}

// 包含範囲
for i in 0..=9 {
    println(f"{i}");  // 0..10 と同じ結果
}

// 独立した式として
let range = 1..100;
let inclusive_range = 1..=99;
```

`start` と `end` はそれぞれ省略可能（半開範囲）。

## 3.11 for ループ

`for` は Range またはコレクションを反復する。

```valen
// Range
for i in 0..10 {
    println(f"{i}");
}

// Java コレクション（Iterable を実装する型）
import java.util.ArrayList;
let list = ArrayList();
list.add("hello");
list.add("world");
for item in list {
    println(item);
}
```

Java の `Iterable` を実装する型（`ArrayList`, `HashSet`, `LinkedList` 等）は直接 `for` で回せる。要素型は `Any`（`java.lang.Object`）。

## 3.12 while ループ

`while` は条件が `true` の間ループを続ける。

```valen
let mut count = 0;
while count < 10 {
    println(f"{count}");
    count += 1;
}
```

`while` はブロック式。ループ値は常に `Unit`。

## 3.13 loop 式

`loop` は無限ループ。`break` で脱出する。`break expr;` でループから値を返せる。

```valen
let x = loop {
    let n = read_input();
    if n > 0 {
        break n;  // loop 式の値
    }
};
```

## 3.14 if let / while let

`if let` と `while let` はパターンマッチと条件分岐を組み合わせた式。ブロック式として扱われるため、末尾のセミコロンは不要。

```valen
if let Some(value) = opt {
    println(f"found: {value}");
}

if let Some(x) = a {
    use_x(x);
} else {
    fallback();
}

while let Some(item) = iter.next() {
    process(item);
}
```

`if let` は `else if` チェインにも対応。

```valen
if let Some(x) = a {
    use_x(x);
} else if let Some(y) = b {
    use_y(y);
} else {
    fallback();
}
```

## 3.15 let else

`let Pattern = expr else { diverge };` は反駁可能パターンによる束縛。パターンが一致しなかった場合は `else` ブロックが実行される。`else` ブロックは発散（`return` / `break` / `continue` / `panic`）しなければならない。

```valen
let Some(value) = get_option() else {
    return;
};
// ここで value が使える

let Ok(data) = parse(input) else {
    println("parse failed");
    return;
};
```

## 3.16 break / continue

`break` と `continue` は `loop` / `while` / `for` の中で使える。

- `break;` — ループを抜ける
- `break expr;` — ループを抜けつつ値を返す（`loop` 式の値になる）
- `continue;` — 現在のイテレーションをスキップし次へ

```valen
let x = loop {
    let n = read_input();
    if n > 0 {
        break n;  // loop 式の値
    }
    continue;
};

while condition() {
    if skip_this() { continue; }
    process();
}
```

**ラベル付き break:** ネストしたループからのラベル指定脱出（`'outer: for ... { break 'outer; }`）は現在サポートしていない。

## 3.17 match 式

`match` は網羅的パターンマッチ。各アームは `pattern => body` の形式。

```valen
let result = match shape {
    Shape::Circle(r: radius) => 3.14 * radius * radius,
    Shape::Rect(w: width, h: height) => width * height,
    _ => 0.0,
};
```

### match ガード

`pattern if condition => body` の形式で、パターン一致後に追加条件を検査できる。

```valen
match value {
    Some(x) if x > 0 => println("positive"),
    Some(x) if x < 0 => println("negative"),
    Some(_) => println("zero"),
    None => println("nothing"),
}
```

ガード条件 `if condition` はパターンが一致した後に評価される。条件が `false` なら次のアームへ進む。

## 3.18 コレクションリテラル

### リストリテラル

`[expr, ...]` 構文で `List<T>`（`java.util.ArrayList`）を生成する。要素型は最初の要素から推論されるか、ターゲット型から決定される。

```valen
let nums = [1, 2, 3];                     // List<Int>
let empty: List<String> = [];             // 空リストは型アノテーション必須
```

### マップリテラル

`#{key: value, ...}` 構文で `Map<K, V>`（`java.util.HashMap`）を生成する。`#` プレフィックスにより `{}` ブロックとの曖昧性を回避。

```valen
let scores = #{"alice": 100, "bob": 85};  // Map<String, Int>
let empty: Map<String, Int> = #{};        // 空マップは型アノテーション必須
```

## 3.19 パイプライン演算子

`|>` 演算子は代入を除く中置演算子の中で最低優先度であり、左辺の値を右辺の関数呼び出しの第1引数に挿入する。

```valen
// x |> f(a, b) は f(x, a, b) にデシュガーされる
"hello" |> println;                        // println("hello")
data |> process(config) |> format(style);  // format(process(data, config), style)
```

右辺は関数呼び出しまたは関数名でなければならない。チェーン可能（左結合）。

## 3.20 バリアントショートハンド

`.Variant` および `.Variant(args)` は、enum 型が文脈から推論できる場合に enum 名を省略する短縮記法。式とパターンの両方で使用可能。

### 式としての使用

```valen
// 通常の記法
let color: Color = Color::Red;
let opt: Option<Int> = Option::Some(42);

// ショートハンド — 型が推論可能な場合
let color: Color = .Red;
let opt: Option<Int> = .Some(42);
```

### パターンとしての使用

```valen
match color {
    .Red => "red",
    .Green => "green",
    .Blue => "blue",
}

if let .Some(value) = opt {
    println(f"{value}");
}
```

バリアント名は大文字で始まる必要がある。ショートハンドパターンではフィールド分解と `..`（残余）も使用可能。

## 3.21 `?` try 演算子

`expr?` は `Result` / `Option` のエラー伝播演算子。式が `Err` / `None` の場合、呼び出し元関数から早期 return する。

```valen
fn read_config(path: String) -> Result<Config, Error> {
    let content = read_file(path)?;   // Err なら即 return
    let config = parse(content)?;
    Ok(config)
}
```

`?` は後置演算子で、優先度はフィールドアクセス・メソッド呼出と同レベル。

詳細は §8 参照。

## 3.22 `safe` ブロック式

`safe { expr }` は Java 例外を `Result` に変換するブロック式。

```valen
let result: Result<String, Exception> = safe {
    file.readLine()
};
```

### 短縮形

`safe expr` — ブロック `{}` を省略可能。

```valen
let result = safe file.readLine();
```

`safe? expr` — `safe { expr }?` の略記。Java 呼び出しの例外を捕捉しつつ即座に `?` で伝播する。

```valen
fn read_first_line(path: String) -> Result<String, Exception> {
    let line = safe? File(path).readLine();
    Ok(line)
}
```

詳細は §8.5 参照。

## 3.23 `unsafe` ブロック式

`unsafe { expr }` は安全性保証を bypass するブロック式。最後の式の値を返す。短縮形 `unsafe expr` も使用可能。

```valen
let pos: Position = unsafe { obj as Position };
let pos: Position = unsafe obj as Position;  // 短縮形
```

`unsafe fn` の呼び出しには `unsafe` ブロックが必要。

```valen
unsafe fn dangerous() -> Int { /* ... */ }

let x = unsafe { dangerous() };
```

詳細は §8.5 参照。

## 3.24 `as` キャスト式

`expr as Type` で型キャストを行う。数値 widening（`Int` → `Long` 等）は safe、ダウンキャストは `unsafe` 必須。

```valen
let x: Long = 42 as Long;                        // safe widening
let pos: Position = unsafe { obj as Position };   // unsafe downcast
```

`as` は後置演算子であり、メソッド呼出・フィールドアクセスと同レベルの優先度を持つ。

詳細は §8.7 参照。

## 3.25 deref 式

`*expr` で `ref mut T` 型の参照を読み取る。`*expr = value` で参照先に書き込む。

```valen
let r = ref mut n;
let v = *r;       // 読み取り
*r = v + 1;       // 書き込み
```

`*` は単項前置演算子として優先度レベル 14 に位置する。

## 3.26 `ref mut` 式

`ref mut expr` でミュータブル参照を作成する。結果は `ref mut T` 型。

```valen
let mut n = 10;
let r = ref mut n;  // r: ref mut Int
```

詳細は §2.8 参照。

## 3.27 ラムダ式

`|params| body` でラムダ（クロージャ）を作成する。

```valen
let add = |a: Int, b: Int| a + b;
let greet = |name: String| println(f"Hello, {name}!");
let unit = || 42;  // パラメータなし
```

パラメータの型アノテーションは省略可能（推論に依存）。

### 戻り値型アノテーション

`|params| -> Type body` で戻り値型を明示できる。

```valen
let parse = |s: String| -> Int {
    s.toInt()
};
```

### ラムダのアリティ

パラメータ数 0〜2 は `java.util.function` の標準インターフェースにマップされ、3〜22 はコンパイラが自動生成する `valen/core/FunctionN` インターフェースを使用する。

| パラメータ数 | JVM 関数型 |
|-------------|-----------|
| 0 | `java.util.function.Supplier<R>` |
| 1 | `java.util.function.Function<T, R>` |
| 2 | `java.util.function.BiFunction<T, U, R>` |
| 3〜22 | `valen.core.FunctionN<A, B, ..., R>`（コンパイラ生成） |

23 パラメータ以上はコンパイルエラー。

詳細は §4.6 参照。
