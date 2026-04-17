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
for in while loop
true false
as
```

予約語（将来用）：`suspend async await yield typealias`

**`@`** は annotation 用の予約 sigil（§20 参照）。MVP では Valen コード内で annotation を書けないため、`@` を識別子前に置くとパーサエラーとなる。

**Valen 仕様で使わないキーワード:** `static` は導入しない。instance method と associated function の区別は `self` レシーバの有無のみで行う（§5.1 参照）。

## 1.3 識別子
- `[a-zA-Z_][a-zA-Z0-9_]*`
- 型は CamelCase 推奨、関数と変数は snake_case 推奨（強制なし）

## 1.4 コメント
```valen
// 単行コメント
/* ブロックコメント */
/// ドキュメントコメント（将来 rustdoc 相当）
```

## 1.5 セミコロン

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
