# 10. 可視性・モジュール

## 10.1 package

```valen
package com.example.foo;

import java.util.HashMap;
import java.util.List;
```

- Java 風、ファイル先頭に package 宣言
- ファイルシステム階層と一致（Java と同様）

## 10.2 可視性修飾子

| 修飾子 | 意味 |
|--------|------|
| `pub` | 公開（どこからでも見える） |
| `internal` | 同一モジュール内 |
| `private` | declaration-private（クラス内・トップレベル内、Kotlin 流） |

デフォルトは `internal`（MVP）。

## 10.3 スコープ演算子

- `::` — enum variant、static-like member
- `.` — package path、type path、値メンバ

```valen
java.util.HashMap         // package path
Shape::Circle(r = 5.0)    // enum variant
User::from_name("Alice")  // static-like associated function
user.name                 // value member
```
