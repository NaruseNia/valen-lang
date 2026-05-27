# 10. 可視性・モジュール

## 10.1 package

```valen
package com.example.foo;

import java.util.HashMap;
import java.util.List;
```

- Java 風、ファイル先頭に package 宣言
- ファイルシステム階層と一致（Java と同様）
- **package 宣言は推奨だが、省略してもコンパイルエラーにはならない。** パーサーは package 宣言なしのファイルを受け付け、resolver もエラーを出さない。テスト等では省略されることが多い
- package は source 階層と名前空間を定義する。`internal` 可視性の境界判定にも使用される（§10.3）

### import 構文

```valen
import java.util.List;                    // 単一型 import
import java.util.concurrent.ConcurrentHashMap as CMap;  // alias
```

- `import path.to.Type;` — 単一型の import
- `import path.to.Type as Alias;` — alias 付き import

選択インポート（`import foo.{A, B}`）とグロブインポート（`import foo.*`）は現在サポートしていない。

## 10.2 module（設計済み・未実装）

> **注意:** 本セクションは言語設計ドキュメントである。module システムは設計済みだが、コンパイラに実装されていない。`valenc` CLI に `--module` フラグは存在せず、Gradle plugin もまだ存在しない。現在の `internal` 可視性は module ID ではなく package パスの比較で制御されている（§10.3 参照）。

`module` は **ビルドターゲット内の意味的所有単位**。orphan rule / `sealed permit` 範囲 / `internal` 可視性はすべて module ID に従う（将来実装時）。

**module はビルドツール駆動で決まる** — Valen ソース内に `module` 宣言は**書かない**。

| ビルドモード | module ID の決定方法 |
|---|---|
| Gradle plugin | Gradle subproject 名 = 1 module |
| `valenc` CLI 単体 | `valenc --module <name> src/*.vln`（未実装） |

**module の基本ルール（設計意図）:**

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

| 修飾子 | 意味 | デフォルト |
|--------|------|-----------|
| `pub` | 公開（どこからでも見える） | |
| `internal` | 同一パッケージ内（暫定実装） | ✓ |
| `private` | declaration-private（クラス内・トップレベル内、Kotlin 流） | |

明示指定がない場合のデフォルトは `internal`。パーサーの `parse_visibility()` は `pub` / `internal` / `private` キーワードがなければ `Visibility::Internal` を返す。

### `internal` の現在の実装

module システムが未実装のため、`internal` 可視性は**パッケージパスの比較**で判定される:

```
check_visibility_from_package():
  def_package == accessor_package → 許可
  どちらかが None → 拒否
  パッケージパスが異なる → 拒否
```

- 同一パッケージ内のファイル間では `internal` メンバにアクセス可能
- 異なるパッケージ間ではアクセス不可
- パッケージ宣言なしのファイル同士は互いにアクセス可能（両方 `None`）

> **将来:** module システム実装後は、`internal` の境界が package パスから module ID に変更される予定。

## 10.4 スコープ演算子

- `::` — enum variant、associated function
- `.` — package path、type path、値メンバ

```valen
java.util.HashMap         // package path
Shape::Circle(r = 5.0)    // enum variant
User::from_name("Alice")  // associated function
user.name                 // value member
```

**用語ポリシー:** Valen 仕様では以下の3語のみを使う。

- `method` — `self` レシーバを持つ、値に対して呼び出す関数
- `associated function` — `self` なしで class 本体に定義される、型名義空間の関数
- `enum variant` — enum の variant

`static` / `static-like` / `static member` という語は **Valen 仕様では使わない**。Java interop 説明の中で Java 側の `static` に言及する場合のみ例外。
