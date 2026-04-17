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
