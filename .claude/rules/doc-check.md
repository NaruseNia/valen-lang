# ドキュメント・LSP 整合性チェック

`crates/` 配下のコードを変更したら、**コミットを作る前に**以下の3チェックを実行する。
「あとで」「まとめて」は禁止。変更のたびに毎回実行すること。

## チェック 1: 言語仕様 (`docs/lang/`)

`crates/` の変更が言語の挙動を変える場合、対応する言語仕様を**実際に読んで**乖離を確認し、必要なら修正する。

| 変更内容 | 確認する仕様 |
|----------|-------------|
| レキサー（トークン追加・変更） | `docs/lang/01-lexical.md` |
| 型システム（型追加・推論変更） | `docs/lang/02-types.md` |
| 式・文の追加・変更 | `docs/lang/03-expressions.md` |
| 関数（引数・戻り値・修飾子） | `docs/lang/04-functions.md` |
| クラス・data class | `docs/lang/05-classes.md` |
| enum・ADT | `docs/lang/06-enum.md` |
| trait・impl・coherence | `docs/lang/07-traits.md` |
| 失敗モデル（Option/Result/?/safe/unsafe） | `docs/lang/08-failure.md` |
| パターンマッチ・exhaustiveness | `docs/lang/09-pattern.md` |
| モジュール・import・package | `docs/lang/10-modules.md` |
| アノテーション | `docs/lang/20-annotations.md` |
| 新しい言語機能 | `docs/LANGUAGE_SPEC.md` のインデックスにも追記 |

## チェック 2: ユーザーガイド (`docs/guide/`)

ユーザーが書くコードに影響する変更は、ガイドにも反映する。ガイドは「その機能を初めて使う人」向け。

| 変更内容 | 確認するガイド |
|----------|--------------|
| 型・リテラル | `docs/guide/02-types.md` |
| ジェネリクス | `docs/guide/03-generics.md` |
| クラス・data class | `docs/guide/04-classes.md` |
| enum・match | `docs/guide/05-enum-and-match.md` |
| trait・impl | `docs/guide/06-traits.md` |
| Option/Result/?/safe | `docs/guide/07-failure-model.md` |
| Java interop | `docs/guide/08-java-interop.md` |
| コンパイラ内部 | `docs/guide/09-compiler-architecture.md` |

## チェック 3: LSP 影響確認

parser または HIR に変更を加えた場合、LSP が壊れていないか確認する。

- **新しい AST ノードを追加した場合**: `crates/valen-lsp/src/` で新ノードが処理されているか確認。未処理なら最低限 fallback 動作するか検証
- **新しいキーワード・構文を追加した場合**: completion 候補に追加が必要か確認
- **型推論・名前解決を変更した場合**: hover / goto-definition が壊れていないか確認
- **確認方法**: `cargo check -p valen-lsp` が通ることを最低限確認。可能なら VSCode で手動検証

## 追加チェック（該当する場合のみ）

- **タスクが完了した場合**: `docs/implementation/comprehensive-plan.md` のステータス更新。該当する `docs/implementation/plan.md` や `phase-1.5-plan.md` も更新
- **要件を充足した場合**: `docs/requirements/overview.md` のステータス列を更新
- **新 crate 追加**: `docs/internals/architecture.md` と `AGENTS.md` のディレクトリ構成を更新
- **要件定義に影響する変更**: 該当する `docs/requirements/REQ-*.md` を更新

## 結果の扱い

- **乖離なし** → そのままコミット
- **軽微な乖離**（既存セクションの修正・追記） → 修正してコミットに含める
- **大きな乖離**（新セクション追加が必要） → ユーザーに報告し、同一コミットに含めるか別コミットにするか確認
