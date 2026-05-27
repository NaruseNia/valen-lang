# 実装フロー

`crates/` または `docs/` に変更を加える実装作業に適用する標準フロー。
既存の `doc-check.md` と `codex-review.md` はこのルールに統合済み。

## フロー概要

```
ブランチ作成 → 実装 + 論理単位コミット → [crates/ 時] Codex レビュー → [crates/ 時] ドキュメント・LSP 整合性 → PR 作成 → CI 待ち → マージ
```

## 適用レベル

| 変更対象 | ブランチ | 論理コミット | precommit | Codex レビュー | Doc/LSP 整合 | PR + CI + マージ |
|----------|---------|-------------|-----------|---------------|-------------|-----------------|
| `crates/` あり | ○ | ○ | ○ | ○ | ○ | ○ |
| `docs/` のみ | ○ | ○ | — | — | — | ○ |

---

## ステップ 1: ブランチ作成

- main から分岐: `feat/<機能名>`, `fix/<バグ名>`, `docs/<内容>`, `refactor/<対象>`
- 既存の作業ブランチがある場合はそれに乗る（ユーザーに確認）
- ブランチ名はケバブケース、英語

## ステップ 2: 実装 + 論理単位コミット

- **コミット粒度**: 機能・論理単位で分割（1ファイル1コミットではない）
- **コミットメッセージ**: Conventional Commits 英語 (`feat:` `fix:` `docs:` `refactor:` `test:` `chore:` `build:`)
- **コミット前**: `mise run precommit` を必ず通す（skip 禁止）
  - 失敗したら修正してから新規コミット（amend しない）
- docs のみ変更の場合はここから直接ステップ 5 へ

## ステップ 3: Codex レビュー（crates/ 変更時のみ）

全コミットが完了した後、PR 作成前に実施。

1. `git diff main...HEAD` で変更全体を把握
2. `/codex-cli` スキルを呼び出し、変更箇所のレビューを依頼
3. レビュー指摘があれば修正して追加コミット
4. 重大な指摘がなくなるまで繰り返す

## ステップ 4: ドキュメント・LSP 整合性（crates/ 変更時のみ）

### 4a: 言語仕様 (`docs/lang/`)

言語の挙動を変える変更は、対応する仕様を**実際に読んで**乖離を確認し、必要なら修正。

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

### 4b: ユーザーガイド (`docs/guide/`)

ユーザーが書くコードに影響する変更は、ガイドにも反映。

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

### 4c: LSP 影響確認

parser / HIR に変更を加えた場合:
- 新 AST ノード追加 → `crates/valen-lsp/src/` で処理されているか確認
- 新キーワード・構文追加 → completion 候補に追加が必要か確認
- 型推論・名前解決変更 → hover / goto-definition が壊れていないか確認
- 最低限 `cargo check -p valen-lsp` が通ることを確認

### 4d: 実装計画・要件（該当時のみ）

- タスク完了 → `docs/implementation/comprehensive-plan.md`, `plan.md`, `phase-1.5-plan.md` のステータス更新
- 要件充足 → `docs/requirements/overview.md` のステータス列更新
- 新 crate 追加 → `docs/internals/architecture.md` と `AGENTS.md` のディレクトリ構成更新
- 仕様課題の解決 → `docs/lang/18-open-questions.md` 更新

### 4e: 乖離の扱い

- **乖離なし** → そのまま次ステップへ
- **軽微な乖離** → 修正してコミットに含める
- **大きな乖離**（新セクション追加等） → ユーザーに報告、同一/別コミットか確認

## ステップ 5: PR 作成

1. リモートにプッシュ
2. `gh pr create` でPR作成
   - タイトル: 70文字以内、Conventional Commits prefix
   - 本文: Summary（箇条書き）+ Test plan
3. ドラフトではなく通常PRで作成（ユーザー指示があればドラフト）

## ステップ 6: CI 待ち + マージ

1. `gh pr checks` で CI ステータスを確認
2. CI 失敗 → 修正して追加コミット、プッシュ
3. CI 全パス → ユーザーにマージ確認
4. 確認後 `gh pr merge` でマージ（squash/merge はユーザー指示に従う、デフォルトは merge commit）
