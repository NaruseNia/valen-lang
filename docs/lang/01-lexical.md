# 1. 字句構文

## 1.1 拡張子・エンコーディング
- ソースファイル：`.vln`
- エンコーディング：UTF-8
- 改行：LF / CRLF 両方受容、正規化はツール任意

## 1.2 キーワード
```
fn let mut self return
if else match
class data enum trait impl
pub internal private
open abstract sealed override
package import
for in while loop break continue
true false
as safe unsafe ref annotation typealias type
inline reified
```

予約キーワード（識別子として使用不可）：`suspend async await yield`

コンテキストキーワード（特定の位置でのみキーワード、他は識別子）：`data`（`data class` の位置でのみキーワード）、`reified`（ジェネリクスパラメータ位置でのみキーワード）

JVM 予約語（Valen では使わないが、識別子としても使用不可）：
```
static void this super null
throw try catch finally extends implements
```

`new` は Valen のキーワードではなく、識別子として使用可能。

### `as` キーワードの用途

`as` は以下の2つの用途で使用する。

1. **import alias:**
   ```valen
   import java.util.HashMap as HMap;
   ```

2. **型キャスト式（§3, §8 参照）:**
   ```valen
   let x: Long = 42 as Long;                    // 安全な widening
   let pos: Position = unsafe { obj as Position }; // ダウンキャストは unsafe 必須
   ```

### `unsafe` / `ref` キーワードの用途

- `unsafe` — 安全性保証を bypass するブロック式・関数修飾子（§8.5 参照）
- `ref` — `ref mut` でミュータブル参照を作成する（§2.8 参照）。`ref` 単体では使用しない

### 演算子

`===` / `!==`（参照比較）を使用可能。§2.2 参照。

**`@`** は annotation 用の予約 sigil（§20 参照）。現在 Valen コード内で annotation を書くことはサポートしていないため、`@` を識別子前に置くとパーサエラーとなる。

**Valen 仕様で使わないキーワード:** `static` は導入しない。instance method と associated function の区別は `self` レシーバの有無のみで行う（§5.1 参照）。

## 1.3 リテラル

### 整数リテラル

整数リテラルは 10 進のほか、16 進・2 進・8 進のプレフィクス表記をサポートする。
アンダースコア (`_`) による桁区切りが可能。

| 基数 | プレフィクス | 例 | 型 |
|------|-------------|------|------|
| 10 進 | なし | `42`, `1_000_000` | `Int` |
| 16 進 | `0x` / `0X` | `0xFF`, `0x1A_2B` | `Int` |
| 2 進 | `0b` / `0B` | `0b1010`, `0b1111_0000` | `Int` |
| 8 進 | `0o` / `0O` | `0o77`, `0o755` | `Int` |

`L` / `l` サフィックスを付けると `Long` 型になる。

```valen
let a = 0xFF;         // Int: 255
let b = 0xFFL;        // Long: 255
let c = 0b1010;       // Int: 10
let d = 0o77;         // Int: 63
let e = 0o77L;        // Long: 63
```

整数リテラル（`L` サフィックスなし）は `Int` 型で、i32 範囲（-2,147,483,648 〜 2,147,483,647）に収まる必要がある。範囲外はコンパイルエラー。

### 文字列補間（f-string）

`f"..."` で文字列補間リテラルを記述する。`{expr}` の位置に式を埋め込める。

```valen
let name = "Alice";
let msg = f"Hello, {name}!";           // "Hello, Alice!"
let calc = f"1 + 2 = {1 + 2}";         // "1 + 2 = 3"
```

エスケープ: `\{` / `\}` でリテラルの `{` / `}` を記述。

**制限:** 補間式内でブロック式 `{ ... }` やネストした f-string は使用不可。変数参照、フィールドアクセス、メソッドチェーン、二項演算などの単純な式を使用すること。

## 1.4 識別子

- `[a-zA-Z_][a-zA-Z0-9_]*`
- 型は PascalCase（`UserProfile`、`Shape`）
- 関数・メソッド・変数は camelCase（`findUser`、`myValue`）
- パッケージは lowercase.dot（`com.example.app`）
- snake_case を使うとコンパイラが warning を出す（エラーにはならない）

## 1.5 コメント
```valen
// 単行コメント
/* ブロックコメント */
/// ドキュメントコメント（将来 rustdoc 相当）
```

## 1.6 セミコロン

Valen の `;` は Rust 流の 3 分類に従う。

1. **文末は `;` 必須** — `let` / `return` / 単純な式文（`foo();` など）
2. **ブロック式は statement position で `;` 省略可** — `if` / `match` / `for` / `while` / `loop` / `{}` の直後
3. **余分な `;` は empty statement として許容** — `if cond { ... };` は合法（fmt で除去推奨）

**値として使うときの挙動:**

- ブロック末尾に置かない式はブロックの値にならず、文として評価される
- ブロック式を値として右辺に置く場合、文末の `;` は必須：
  ```valen
  let x = if y > 0 { y } else { -y };  // ; は let 文の終端
  ```
- ブロック式を statement position に置き、あえて `;` を付けると戻り値を unit 化して捨てる：
  ```valen
  let _ = if c { side_effect() };  // unit 化明示
  ```

**statement position の定義:** 関数本体・ブロックの中で、トップレベルに配置される要素の位置。`let` の右辺や引数は statement position ではない。
