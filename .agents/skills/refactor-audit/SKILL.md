---
name: refactor-audit
description: |
  Rustプロジェクト全体を網羅的に検査し、発見した問題をIssueとして起票するリファクタリング監査スキル。
  crate単位でサブエージェントを並列起動し、10の検査次元×具体的チェックリストで検査。
  トリガー: "refactor-audit", "監査", "全体レビュー", "コード監査", "audit"
  使用場面: コード品質の網羅的検査、リファクタリング前の問題洗い出し、仕様適合確認
---

# refactor-audit

Rustプロジェクトの全crateを網羅的に検査し、発見した問題をIssue（GitHub or ローカルmd）として起票する監査スキル。コードは一切変更しない（read-only）。

---

## 実行フロー概要

```
1. スキャン対象の決定（引数 or 全crate）
2. Issue起票先の選択（GH Issue / ローカル md / 両方）
3. cargo clippy + cargo test を実行、結果を取得
4. crate↔仕様マッピングを構築、関連仕様を要約
5. crate単位でサブエージェントを並列起動（検査実行）
6. 各サブエージェントの結果（JSON）を集約
7. Issue起票（選択された起票先へ）
8. SUMMARY.md 生成 + ターミナル出力
```

---

## ステップ 1: スキャン対象の決定

引数の解釈:
- **引数なし**: `crates/` 配下の全crateを走査
- **crate名を指定**: 例 `valen-parser` → そのcrateのみ
- **ファイルパスを指定**: 例 `crates/valen-codegen/src/emit.rs` → そのファイルのみ
- **複数指定**: スペース区切りで複数crate/ファイルを指定可能

crateの一覧は以下のコマンドで取得:
```bash
ls -d crates/*/
```

---

## ステップ 2: Issue起票先の選択

スキャン開始前にユーザーに1回だけ確認する:

選択肢:
1. **ローカル md** — `./issues/{seq}_{scope}_{title}.md` に起票
2. **GitHub Issue** — `gh issue create` で起票、`severity:*` + `dimension:*` ラベル自動付与
3. **両方** — ローカル md + GitHub Issue の両方に起票

### ローカル md の連番ルール
```bash
# 既存の最大連番を取得（なければ0）
max_seq=$(ls ./issues/*.md 2>/dev/null | grep -oP '^\d{3}' | sort -rn | head -1)
next_seq=$((max_seq + 1))
```

---

## ステップ 3: cargo ツール実行

スキル冒頭で以下を実行し、結果を変数に保持:

```bash
cargo clippy --workspace --all-targets -- -D warnings 2>&1
cargo test --workspace 2>&1
```

結果は各サブエージェントのプロンプトに含める。

---

## ステップ 4: 仕様マッピングと要約

以下の crate↔仕様 マッピングに基づき、各crateに関連する仕様ファイルを特定し、要約を作成する。

### crate → 仕様ファイル マッピング

| crate | 関連する仕様 (docs/lang/) | 関連する要件定義 (docs/requirements/) |
|---|---|---|
| `valen-ast` | `01-lexical.md`, `02-types.md`, `03-expressions.md` | `REQ-SYNTAX.md`, `REQ-TYPE.md`, `REQ-ADT.md` |
| `valen-parser` | `01-lexical.md`, `02-types.md`, `03-expressions.md`, `04-functions.md`, `05-classes.md`, `06-enum.md`, `07-traits.md`, `08-failure.md`, `09-pattern.md`, `20-annotations.md` | `REQ-SYNTAX.md`, `REQ-TYPE.md`, `REQ-ADT.md`, `REQ-CLASS.md`, `REQ-TRAIT.md`, `REQ-FAIL.md` |
| `valen-hir` | `02-types.md`, `05-classes.md`, `06-enum.md`, `07-traits.md`, `08-failure.md`, `09-pattern.md`, `10-modules.md` | `REQ-TYPE.md`, `REQ-ADT.md`, `REQ-CLASS.md`, `REQ-TRAIT.md`, `REQ-FAIL.md` |
| `valen-codegen` | `16-jvm-target.md`, `02-types.md`, `05-classes.md`, `06-enum.md`, `04-functions.md` | `REQ-EMIT.md`, `REQ-INTEROP.md`, `REQ-ADT.md`, `REQ-CLASS.md` |
| `valen-diagnostics` | `08-failure.md` | `REQ-FAIL.md` |
| `valenc` | (CLI仕様、アーキテクチャドキュメント) | `REQ-TOOL.md` |
| `valen-lsp` | (LSP仕様、外部参照) | `REQ-TOOL.md` |
| `valenfmt` | (フォーマット規約) | `REQ-TOOL.md` |

**注:** `REQ-STDLIB.md` は stdlib crate（未着手）に対応。現存crateへの直接関連は薄いが、型定義やinterop境界で参照が必要な場合がある。

各サブエージェントに渡す前に、関連する**仕様ファイル + 要件定義ファイル**の両方を読み、**そのcrateの実装に直接関係する部分だけを要約**（箇条書き、最大40行程度）して渡す。全文は渡さない。

要件定義から抽出すべき情報:
- 各REQのID・タイトル・優先度・ステータス（Done / Draft / 未着手の区別）
- Must 優先度のREQが実装されているかの確認材料
- REQ間の依存関係（依存先が未実装なら当該REQも未実装で正当）

---

## ステップ 5: サブエージェント並列起動

### spawn ルール

- **1 crate = 1 サブエージェント**
- 全サブエージェントを **同時に** 起動する
- 各サブエージェントは **read-only**（ファイルの変更は行わない）
- 各サブエージェントには以下を渡す:
  - 対象crateの全ソースファイル一覧
  - cargo clippy / cargo test の出力（当該crate関連部分）
  - 関連仕様の要約
  - 関連要件定義の要約（REQ-ID, タイトル, 優先度, ステータス, 依存関係）
  - 検査チェックリスト（後述）
  - 出力フォーマット仕様（後述）

### サブエージェント プロンプトテンプレート

各サブエージェントに渡すプロンプトの構造:

```
あなたは Rust コード監査エージェントです。以下のcrateを網羅的に検査してください。

## 対象crate
{crate名}

## ソースファイル一覧
{ファイルパスのリスト}

## cargo clippy 出力（このcrate関連）
{clippy結果の抜粋}

## cargo test 出力（このcrate関連）
{test結果の抜粋}

## 関連仕様の要約
{仕様の要約テキスト}

## 関連要件定義の要約
{要件定義の要約テキスト — REQ-ID, タイトル, 優先度, ステータス, 依存関係を含む}

## 検査指示

全ソースファイルを **省略せず** 読み、以下の10次元で検査してください。
各ファイルの各関数・各型・各impl・各テストを1つずつ確認すること。
「問題なし」の場合は報告不要、問題がある場合のみ報告。

{検査チェックリスト（後述の全10 dimensionのチェックリスト）}

## severity 判定基準

- critical: コンパイルは通るが実行時に誤った結果を生む。データ破損の可能性。セキュリティ上の問題
- major: 仕様との乖離。テスト欠如による品質リスク。Rustのアンチパターン（unwrap地獄等）
- minor: 非イディオマティックだが動作に問題なし。命名の一貫性欠如。不足しているドキュメント
- enhancement: 「こうすればもっと良い」レベル。パフォーマンス改善の余地。リファクタリング提案

## 出力フォーマット

結果をJSON配列として出力してください。問題が見つからなければ空配列 `[]` を返してください。
**JSON以外のテキストは一切出力しないでください。**

```json
[
  {
    "title": "snake_case_の問題タイトル",
    "scope": "{crate名}",
    "severity": "critical | major | minor | enhancement",
    "dimension": "idiomatic_rust | spec_coverage | test_coverage | fixture_coverage | correctness | error_handling | documentation | naming | design | performance",
    "summary": "何が問題か — 1〜2文で",
    "current_state": "問題の現在のコード・状態を具体的に。ファイルパスと行番号を含める",
    "code_snippet": "問題のあるコード例（該当部分を引用）",
    "problem": "なぜこれが問題なのか — 具体的なリスク・影響",
    "suggestion": "どう直すべきか — 具体的な方針。コード例があれば含める",
    "affected_files": ["path/to/file.rs:L42"],
    "related_files": ["path/to/related.rs:L100-L120"]
  }
]
```
```

---

## ステップ 6: 結果の集約

全サブエージェントの結果を受け取り:

1. 各サブエージェントの出力からJSON配列をparse
2. 全結果を1つの配列にマージ
3. severity 順にソート（critical → major → minor → enhancement）
4. 連番を振る（001, 002, ...）

JSON parseに失敗した場合は、そのサブエージェントの出力をそのままテキストとして扱い、手動で構造化するか、ユーザーに報告する。

---

## ステップ 7: Issue 起票

### ローカル md 起票

ファイルパス: `./issues/{seq}_{scope}_{title}.md`

テンプレート:
```markdown
---
scope: {scope}
severity: {severity}
dimension: {dimension}
---

# {title（human-readable形式に変換）}

## 概要

{summary}

## 現状

{current_state}

```{言語}
{code_snippet}
```

## 問題点

{problem}

## 改善案

{suggestion}

## 影響範囲

{affected_files をリスト形式で}

## 関連ファイル

{related_files をリスト形式で}
```

### GitHub Issue 起票

```bash
gh issue create \
  --title "{title（human-readable形式）}" \
  --body "{上記テンプレートと同じ内容}" \
  --label "severity:{severity}" \
  --label "dimension:{dimension}"
```

ラベルが存在しない場合は事前に作成する:
```bash
gh label create "severity:critical" --color "B60205" --description "実行時バグ・データ破損" 2>/dev/null
gh label create "severity:major" --color "D93F0B" --description "仕様乖離・テスト欠如" 2>/dev/null
gh label create "severity:minor" --color "FBCA04" --description "非イディオマティック" 2>/dev/null
gh label create "severity:enhancement" --color "0E8A16" --description "改善提案" 2>/dev/null
gh label create "dimension:idiomatic_rust" --color "1D76DB" 2>/dev/null
gh label create "dimension:spec_coverage" --color "1D76DB" 2>/dev/null
gh label create "dimension:test_coverage" --color "1D76DB" 2>/dev/null
gh label create "dimension:fixture_coverage" --color "1D76DB" 2>/dev/null
gh label create "dimension:correctness" --color "1D76DB" 2>/dev/null
gh label create "dimension:error_handling" --color "1D76DB" 2>/dev/null
gh label create "dimension:documentation" --color "1D76DB" 2>/dev/null
gh label create "dimension:naming" --color "1D76DB" 2>/dev/null
gh label create "dimension:design" --color "1D76DB" 2>/dev/null
gh label create "dimension:performance" --color "1D76DB" 2>/dev/null
```

---

## ステップ 8: SUMMARY.md 生成

`./issues/SUMMARY.md` を生成し、ターミナルにも出力:

```markdown
# Refactor Audit Summary

**Date:** {実行日時}
**Target:** {対象crateリスト}
**Files scanned:** {ファイル数}
**Lines scanned:** {行数}

## Overview

| Severity | Count |
|----------|-------|
| critical | {n} |
| major | {n} |
| minor | {n} |
| enhancement | {n} |
| **Total** | **{n}** |

## By Dimension

| Dimension | Count |
|-----------|-------|
| idiomatic_rust | {n} |
| spec_coverage | {n} |
| test_coverage | {n} |
| fixture_coverage | {n} |
| correctness | {n} |
| error_handling | {n} |
| documentation | {n} |
| naming | {n} |
| design | {n} |
| performance | {n} |

## By Crate

| Crate | Critical | Major | Minor | Enhancement | Total |
|-------|----------|-------|-------|-------------|-------|
| {crate} | {n} | {n} | {n} | {n} | {n} |
| ... | | | | | |

## Issue List

| # | Severity | Dimension | Scope | Title |
|---|----------|-----------|-------|-------|
| 001 | {severity} | {dimension} | {scope} | [{title}]({ファイルパス or GH URL}) |
| ... | | | | |

## Filed To

- {起票先の情報}
```

---

## 検査チェックリスト（全10 dimension）

### 1. idiomatic_rust

- [ ] 不要な `clone()` / `to_string()` / `to_owned()` がないか
- [ ] `Option`/`Result` を `unwrap()` / `expect()` で雑に開けていないか（テスト以外）
- [ ] `match` が `_` ワイルドカードで網羅性を逃げていないか（将来のvariant追加で見逃しが起きる）
- [ ] `&String` ではなく `&str` を引数に取っているか
- [ ] `&Vec<T>` ではなく `&[T]` を引数に取っているか
- [ ] Iterator チェーン vs for ループの適切な使い分け
- [ ] `derive` の適切さ（Debug, Clone, PartialEq 等が必要なものに付いているか、不要なものに付いていないか）
- [ ] `pub` の公開範囲が最小か（`pub(crate)` / `pub(super)` で十分な箇所に `pub` がないか）
- [ ] `impl` ブロック内のメソッド順序が慣習に沿っているか（`new` → public → private）
- [ ] `Box<dyn Error>` 等の型消去が適切か、具体的なエラー型を使うべき箇所はないか
- [ ] `if let` / `while let` で簡潔に書ける `match` がないか
- [ ] 不要な `return` 文（最後の式で暗黙returnできる箇所）
- [ ] `use` 文の整理（未使用import、グロブimportの濫用）
- [ ] `clippy` の指摘と一致する問題がないか（cargo clippy 出力を参照）

### 2. spec_coverage

- [ ] 関連仕様に記載された構文・構造が全てASTノード or パーサールールとして実装されているか
- [ ] 仕様に記載された型（プリミティブ型、ユーザー定義型、ジェネリクス等）が型システムに反映されているか
- [ ] 仕様に記載されたセマンティクス（スコープ規則、可視性、mutability等）がHIRで検査されているか
- [ ] 仕様に記載されたJVMマッピング（型→JVM型、enum→class階層等）がcodegen に反映されているか
- [ ] 仕様に記載されたエラーケース（型エラー、スコープエラー等）が diagnostics として実装されているか
- [ ] 仕様の「未定義動作」や「処理系定義」の箇所が明示的に TODO/unimplemented! で示されているか
- [ ] 仕様との乖離がある場合、それが意図的な差分（設計判断）か、実装漏れか
- [ ] **要件定義（REQ-*）のうち Must 優先度かつ Done ステータスのものが、実際にコードで実装されているか**
- [ ] **要件定義のうち Must 優先度かつ Draft/未着手のものについて、依存先REQが未実装なら正当、そうでなければ実装漏れとして報告**
- [ ] **要件定義のステータス（Done/Draft）とコードの実装状態が一致しているか**（Done なのに未実装、または実装済みなのに Draft のまま）
- [ ] **要件定義間の依存関係（依存フィールド）が実装上も正しく反映されているか**（依存先の型/関数を正しく使っているか）

### 3. test_coverage

- [ ] 各 public 関数にユニットテストがあるか
- [ ] 各パーサールールに対するテストがあるか
- [ ] 正常系だけでなく異常系（エラーケース）のテストがあるか
- [ ] 境界値テスト（空文字列、空リスト、最大値等）があるか
- [ ] 回帰テスト（過去に修正したバグに対するテスト）が適切に存在するか
- [ ] テストが意味のあるアサーションをしているか（ただ動くだけでなく結果を検証しているか）
- [ ] `#[ignore]` / `#[should_panic]` が適切に使われているか（理由のコメント付きか）
- [ ] integration テスト（crate 境界を跨ぐテスト）があるべき箇所にあるか

### 4. fixture_coverage

- [ ] `.vln` fixture ファイルがパーサーの各構文要素をカバーしているか
- [ ] codegen の各出力パターン（class, enum, function, expression 等）に対応するfixtureがあるか
- [ ] エラーケース用の fixture（構文エラー、型エラーを含むコード）が十分にあるか
- [ ] 複合的なケース（複数の機能を組み合わせたコード）の fixture があるか
- [ ] fixture のファイル名が内容を適切に表しているか
- [ ] fixture が最新の言語仕様に追従しているか（古い構文のままになっていないか）

### 5. correctness

- [ ] ロジックエラー（off-by-one、境界条件の誤り等）がないか
- [ ] 型変換（as キャスト等）でデータ損失の可能性がないか
- [ ] パニック（`unreachable!()`, `todo!()`, `panic!()`）が本番コードに不適切に残っていないか
- [ ] 整数オーバーフローの可能性がないか
- [ ] 無限ループの可能性がないか
- [ ] パターンマッチの漏れ（将来のvariant追加で壊れるコード）がないか
- [ ] バイトコード生成の正しさ（JVMスタック操作の整合性、descriptor の正確さ）
- [ ] `cargo test` の出力で失敗しているテストはないか

### 6. error_handling

- [ ] `unwrap()` がテスト以外のコードで使われていないか
- [ ] エラーが適切に伝播されているか（`?` 演算子の使用）
- [ ] カスタムエラー型が適切に定義されているか
- [ ] エラーメッセージが具体的で、ユーザーが問題を特定できる内容か
- [ ] `valen-diagnostics` の共通構造体が一貫して使われているか
- [ ] panic! / unreachable! に到達しうるパスがないか
- [ ] エラーリカバリ（パーサーのエラーリカバリ等）が適切に実装されているか
- [ ] `Result` のエラーバリアントが握り潰されていないか（`let _ = ...` で無視されていないか）

### 7. documentation

- [ ] 各 pub 型に doc comment (`///`) があるか
- [ ] 各 pub 関数に doc comment があるか（引数・戻り値・パニック条件の説明）
- [ ] モジュールレベルの doc comment (`//!`) があるか
- [ ] 複雑なロジックに「なぜ」を説明するコメントがあるか
- [ ] 古い / 不正確なコメントが残っていないか
- [ ] TODO / FIXME / HACK コメントが放置されていないか（対応済みなら削除すべき）
- [ ] doc comment の例（`/// # Examples`）が適切な箇所にあるか

### 8. naming

- [ ] 変数名・関数名・型名がRustの命名規則に沿っているか（snake_case, CamelCase, SCREAMING_SNAKE_CASE）
- [ ] 名前が処理内容を適切に表しているか（略語の濫用、曖昧な名前がないか）
- [ ] 同じ概念に対して異なる名前が使われていないか（例: `token` vs `tok`, `span` vs `location`）
- [ ] 型名とモジュール名の衝突がないか
- [ ] ジェネリック型パラメータの名前が慣習に沿っているか（`T`, `E`, `K`, `V` 等）
- [ ] bool を返す関数が `is_` / `has_` / `can_` 等で始まっているか
- [ ] crate 間で命名の一貫性があるか

### 9. design

- [ ] 単一責任原則に沿っているか（1つのモジュール/構造体が複数の責務を持っていないか）
- [ ] モジュール間の依存関係が適切か（循環依存がないか）
- [ ] 型の設計が不正状態を表現不可能にしているか（make illegal states unrepresentable）
- [ ] `enum` vs `trait object` の選択が適切か
- [ ] ビルダーパターン等のデザインパターンが適切に使われているか
- [ ] crate 間のインターフェース（pub API）が最小限か
- [ ] 共通処理が適切に共有されているか（コードの重複がないか）
- [ ] 将来の拡張性を考慮した設計になっているか（ただし過剰設計でないか）

### 10. performance

- [ ] 不要なヒープアロケーション（String, Vec の不要なコピー）がないか
- [ ] 大きな構造体が不必要にコピーされていないか（&参照で渡すべき箇所）
- [ ] 文字列結合で `format!` の濫用がないか（`push_str` / `write!` の方が効率的な場合）
- [ ] HashMap/BTreeMap の選択が適切か
- [ ] 繰り返し処理で不要な中間コレクション（`collect()` → 再iterate）がないか
- [ ] 大きな enum / struct のサイズが適切か（`Box` で間接化すべき大きなvariant がないか）
- [ ] ホットパス（パーサーのメインループ等）に不要な処理がないか

---

## 注意事項

- **コードは一切変更しない**。このスキルは read-only + Issue起票のみ
- **サボらない**。「問題なさそう」で飛ばさず、全ファイル・全関数を1つずつ確認する
- **具体的に指摘する**。ファイルパスと行番号を必ず含める
- **重複検知はしない**。毎回クリーンスキャンとして実行する
- **severity を正確に判定する**。迷ったら1段階上げる（見逃すより過剰報告の方が安全）
- サブエージェントが JSON parse 不能な出力を返した場合、ユーザーに報告して手動対応を促す
- `./issues/` ディレクトリが存在しない場合は自動作成する
