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
