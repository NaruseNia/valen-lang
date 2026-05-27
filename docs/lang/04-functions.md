# 4. 関数

## 4.1 定義

```valen
fn add(a: Int, b: Int) -> Int {
    a + b
}
```

- トップレベル関数可
- 返り値が `Unit` なら `-> Unit` 省略可

## 4.2 名前付き引数（MVP）

```valen
fn greet(msg: String, count: Int) -> String { /* ... */ }

greet(msg = "hi", count = 3);
```

## 4.3 デフォルト引数（Phase 1.5 実装済み）

```valen
fn greet(msg: String = "hi", count: Int = 1) -> String { /* ... */ }

greet()              // msg = "hi", count = 1
greet("yo")          // msg = "yo", count = 1
greet("yo", 3)       // msg = "yo", count = 3
```

- デフォルト値は **任意の式**（リテラル、関数呼び出し等）
- 評価タイミングは **call-site**（呼び出しごとに評価）
- 任意のパラメータ位置にデフォルト値を指定可能（末尾制約なし）
- named args と組み合わせて中間パラメータの省略も可能
- class / data class の ctor パラメータにも使用可能
- trait メソッドにもデフォルト値を指定可能（impl での上書きは不可）

## 4.4 UFCS

メソッド記法 `value.method(args)` が第一級。曖昧性がある場合は **`Trait::method(receiver, args)`** で解消する。これが Valen における唯一の UFCS 形式。

```valen
trait Mappable<T> {
    fn map<U>(self, f: fn(T) -> U) -> Mappable<U>;
}

// 通常のメソッド呼び出し
xs.map(|x| x * 2);

// 曖昧性解消（trait を明示）
Mappable::map(xs, |x| x * 2);
```

**禁止された旧記法:**
- ~~`map(xs, f)` 形式~~ — トップレベル関数と区別不能
- ~~`greet(p)` 形式~~ — 推論任せで破綻する

`foo(args)` は常にトップレベル関数の呼び出しとして解決される。trait method を関数呼び出し風に書くことはできない。

## 4.5 型推論

- **ローカル変数**: 型推論あり。`let x = 42;` は `Int` と推論される
- **関数シグネチャ**: パラメータ型と戻り値型は**明示必須**。省略はコンパイルエラー

```valen
let x = 42;           // x: Int (推論)
let y = f"{x}";       // y: String (推論)
let items = List();    // items: List<???> → 型注釈必要: let items: List<Int> = List();

// fn シグネチャは明示必須
fn add(a: Int, b: Int) -> Int {
    a + b  // ボディ内は推論
}
```

## 4.7 組み込み関数

prelude に含まれる組み込み関数。import 不要で使用可能。

| 関数 | シグネチャ | 説明 | JVM 実装 |
|------|-----------|------|----------|
| `println` | `fn(String) -> Unit` | 標準出力に文字列を出力し改行 | `System.out.println(String)` |
| `print` | `fn(String) -> Unit` | 標準出力に文字列を出力（改行なし） | `System.out.print(String)` |

```valen
println("hello world");          // hello world\n
print("no newline");             // no newline
println(f"count: {x}");          // f-string と組み合わせ
```

## 4.8 インライン関数

`inline fn` はコールサイトに本体をインライン展開する関数。ラムダ引数もインライン化されるため、ボクシングを回避できる。

```valen
inline fn <T> measure(block: fn() -> T) -> T {
    let start = System.nanoTime();
    let result = block();
    println(f"elapsed: {System.nanoTime() - start}ns");
    result
}
```

### 構文

```
inline fn <Params> name(params) -> ReturnType { body }
```

- `inline` キーワードを `fn` の前に付与
- 関数本体は呼び出し側にインライン展開される
- ラムダ引数はデフォルトでインライン化される（ボクシングなし）

### ラムダのインライン化

`inline fn` に渡されたラムダ引数は呼び出し側に展開される。

```valen
inline fn <T> run(block: fn() -> T) -> T {
    block()
}

fn main() {
    let x = run(|| { 42 });
    // block() の本体がここに展開される
}
```

non-local return（ラムダ内の `return` が呼び出し元関数から脱出する動作）は将来対応予定。

### 制約

- **再帰禁止**: `inline fn` が自身を再帰呼び出しするとコンパイルエラー（展開が無限ループになるため）
- `inline fn` の本体変更は呼び出し側の再コンパイルが必要

### Java interop

Java 側からは `inline fn` は通常のメソッドとして見える。`reified` 型パラメータは Java 呼び出し時には無効（型消去される）。
