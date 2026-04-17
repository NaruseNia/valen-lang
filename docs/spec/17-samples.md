# 17. サンプル

## 17.1 Hello, Valen

```valen
package com.example.hello;

fn main() {
    println("Hello, Valen!")
}
```

## 17.2 エラー処理

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

## 17.3 trait + impl

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
