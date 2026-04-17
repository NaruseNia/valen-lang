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
open abstract sealed
package import
for in while loop
true false
as
```

予約語（将来用）：`suspend async await yield typealias`

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

- 文末は `;` 必須
- **ブロック末尾の式のみ `;` 省略で値返却**
- 教え方：「`;` は式を文として終える記号。ブロック末尾に置かない式は値になる」
- `if` / `match` / `block` はすべて式、`;` を付けると unit 化
