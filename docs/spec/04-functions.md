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

## 4.3 デフォルト引数（Phase 1.5）

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

## 4.4 UFCS

メソッド記法 `xs.map(f)` が第一級、UFCS `map(xs, f)` は同一解決系の補助記法。

```valen
trait Mappable<T> {
    fn map<U>(self, f: fn(T) -> U) -> Mappable<U>;
}

// 両方とも同じ trait を解決
xs.map(|x| x * 2);
map(xs, |x| x * 2);  // UFCS
```
