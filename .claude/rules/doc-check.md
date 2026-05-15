# ドキュメント整合性チェック

コードに触れたセッションでコミットを作成する**直前**に、以下の手順を実行する。

## 発火条件

`crates/` 配下のファイルに変更がある場合（docs のみの変更では発火しない）。

## 手順

1. `git diff --cached --name-only` でステージ済みの変更ファイル一覧を取得
2. 変更された crate を特定し、以下のマッピングで影響する docs を判定:

| 変更 crate / ファイル | 確認すべき docs |
|---|---|
| `valen-parser` | `docs/lang/01-lexical.md` 〜 `10-modules.md`, `20-annotations.md` |
| `valen-hir` (型検査) | `docs/lang/02-types.md`, `docs/requirements/REQ-TYPE.md` |
| `valen-hir` (coherence) | `docs/lang/07-traits.md`, `docs/requirements/REQ-TRAIT.md` |
| `valen-hir` (exhaustive) | `docs/lang/09-pattern.md`, `docs/requirements/REQ-ADT.md` |
| `valen-codegen` | `docs/internals/architecture.md`, `docs/requirements/REQ-EMIT.md` |
| `valen-ast` (TokenKind 追加) | `docs/lang/01-lexical.md` |
| `valen-lsp` | `docs/requirements/REQ-TOOL.md` |
| `valenfmt` | `docs/requirements/REQ-TOOL.md` |
| 新 crate 追加 | `docs/internals/architecture.md`, `AGENTS.md` のディレクトリ構成 |

3. 影響する docs がある場合、**実際にファイルを読んで**乖離がないか確認
4. 結果に応じて:
   - **乖離なし** → そのままコミット
   - **軽微な乖離**（ステータス更新、既存セクションへの追記） → 修正してコミットに含める
   - **大きな乖離**（新セクション追加、構成変更が必要） → ユーザーに報告し、同一コミットに含めるか別コミットにするか確認

## 追加チェック

- 新しい言語機能を実装した場合: `docs/guide/` の該当章に反映が必要か確認
- タスクが完了した場合: `docs/implementation/plan.md` または `phase-1.5-plan.md` のステータス更新
- 要件を充足した場合: `docs/requirements/overview.md` のステータス列を更新
