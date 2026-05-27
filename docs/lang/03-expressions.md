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

## 3.4 for ループ

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

## 3.5 if let / while let

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

## 3.6 break / continue

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

## 3.10 コレクションリテラル

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

## 3.11 パイプライン演算子

`|>` 演算子は最低優先度の中置演算子で、左辺の値を右辺の関数呼び出しの第1引数に挿入する。

```valen
// x |> f(a, b) は f(x, a, b) にデシュガーされる
"hello" |> println;                        // println("hello")
data |> process(config) |> format(style);  // format(process(data, config), style)
```

右辺は関数呼び出しまたは関数名でなければならない。チェーン可能（左結合）。

## 3.12 `unsafe` ブロック式

`unsafe { expr }` は安全性保証を bypass するブロック式。最後の式の値を返す。短縮形 `unsafe expr` も使用可能。

```valen
let pos: Position = unsafe { obj as Position };
let pos: Position = unsafe obj as Position;  // 短縮形
```

詳細は §8.5 参照。

## 3.13 `as` キャスト式

`expr as Type` で型キャストを行う。数値 widening（`Int` → `Long` 等）は safe、ダウンキャストは `unsafe` 必須。

```valen
let x: Long = 42 as Long;                        // safe widening
let pos: Position = unsafe { obj as Position };   // unsafe downcast
```

詳細は §8.7 参照。

## 3.14 deref 式

`*expr` で `ref mut T` 型の参照を読み取る。`*expr = value` で参照先に書き込む。

```valen
let r = ref mut n;
let v = *r;       // 読み取り
*r = v + 1;       // 書き込み
```

## 3.15 `ref mut` 式

`ref mut expr` でミュータブル参照を作成する。結果は `ref mut T` 型。

```valen
let mut n = 10;
let r = ref mut n;  // r: ref mut Int
```

詳細は §2.8 参照。
