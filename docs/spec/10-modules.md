# 10. 可視性・モジュール

## 10.1 package

```valen
package com.example.foo;

import java.util.HashMap;
import java.util.List;
```

- Java 風、ファイル先頭に package 宣言
- ファイルシステム階層と一致（Java と同様）
- **package は source 階層と名前空間のみ**。所有権・可視性単位としては使わない（それは `module` の責務、§10.2）

## 10.2 module

`module` は **ビルドターゲット内の意味的所有単位**。orphan rule / `sealed permit` 範囲 / `internal` 可視性はすべて module ID に従う。

**module はビルドツール駆動で決まる** — Valen ソース内に `module` 宣言は**書かない**。

| ビルドモード | module ID の決定方法 |
|---|---|
| Gradle plugin | Gradle subproject 名 = 1 module |
| `valenc` CLI 単体 | `valenc --module <name> src/*.vln` |

**module の基本ルール:**

- 同一 module ID に属するソースファイルは複数あってよい（ファイル境界 ≠ module 境界）
- 異なる module は同一 Gradle build / classpath に共存できるが、所有権は別
- `internal` 可視性は同一 module の全ファイルから見える
- `sealed class` の permit 先は同一 module に属する必要がある
- trait / nominal type の所有は module 単位で決まる（orphan rule、§7.4）

**package との関係:**

- 1 module は複数の package を含んでよい（`com.example.foo`, `com.example.bar` が同じ module の中にあっても良い）
- 1 package が複数 module に分割されていても良い（同じ `com.example.foo` が別 module にあっても構文上は合法、ただし実用上は推奨しない）

**compile unit との関係:**

- Valen 仕様には `compile unit` という用語を登場させない
- 物理的な compile 単位はビルドツールの実装詳細

## 10.3 可視性修飾子

| 修飾子 | 意味 |
|--------|------|
| `pub` | 公開（どこからでも見える） |
| `internal` | 同一モジュール内 |
| `private` | declaration-private（クラス内・トップレベル内、Kotlin 流） |

デフォルトは `internal`（MVP）。`internal` の範囲は §10.2 の module に従う。

## 10.4 スコープ演算子

- `::` — enum variant、static-like member
- `.` — package path、type path、値メンバ

```valen
java.util.HashMap         // package path
Shape::Circle(r = 5.0)    // enum variant
User::from_name("Alice")  // static-like associated function
user.name                 // value member
```
