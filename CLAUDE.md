# CLAUDE.md

Claude Code 向け作業ガイドライン。このファイルは Valen 言語プロジェクトのコントリビューション方針と設計哲学の要約を記述。

## プロジェクト概要

**Valen** — JVM ターゲットの新規言語、設計段階。`valenc` (Rust) + Gradle plugin + LSP + fmt の構成。

**芯（必ず守る4点）**
- 強ADT（sum type with payload）
- exhaustive match（Rust フルセット）
- trait ベース抽象（orphan rule 厳格）
- 整合した失敗モデル（Option/Result/Exception/panic の役割分離）

**ポジショニング:** Java/Kotlin 資産に乗る、ADT 中心の JVM 言語。Kotlin 超え主張しない、補完ポジション。

## 設計原則

1. **折衷案は却下** — 「Kotlin + Rust のいいとこ取り」にしない。何かを選んで何かを捨てる
2. **Phase 境界を守る** — MVP は核の証明に集中、便利機能は Phase 1.5+ へ
3. **Rust 語彙を使うが意味論は JVM/OOP** — `self/mut/trait/impl/?` を使うが、所有権・借用は導入しない
4. **Java 親和優先、Rust 厳密優先のどちらかを選ぶ場面では、新機能は Rust 厳密側、既存資産連携は Java 側**
5. **仕様を記述するときは実装都合を出さない** — 例：`Int` は「JVM 整数型に対応する名義型」まで、boxing は実装メモ

## 確定事項（2026-05 時点）

- **要件定義:** [docs/requirements/overview.md](docs/requirements/overview.md) — 10 スコープ、45 要件（REQ-{SCOPE}-{SEQ} 形式）
- **言語仕様:** [docs/LANGUAGE_SPEC.md](docs/LANGUAGE_SPEC.md) → [docs/spec/](docs/spec/) 配下の詳細仕様
- **アーキテクチャ:** [docs/specifications/architecture.md](docs/specifications/architecture.md)
- **実装計画:** [docs/implementation/plan.md](docs/implementation/plan.md) — 17 タスク、5 マイルストーン、トレーサビリティマトリクス
- **Phase 計画:** [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md)
- **grill-me 4巡確定事項:** [docs/spec/18-open-questions.md](docs/spec/18-open-questions.md) 末尾の決定サマリ表
- Codex 3巡レビューの採用履歴は memory の `project_valen_codex_review.md` を参照

## ディレクトリ構成

```
valen-lang/
├── crates/
│   ├── valen-ast/          # AST 型定義、TokenKind、Span
│   ├── valen-diagnostics/  # エラー・警告の共通構造体
│   ├── valen-parser/       # logos lexer + hand-written RD parser
│   ├── valen-hir/          # 名前解決・型検査・coherence・exhaustiveness
│   ├── valen-codegen/      # JVM bytecode 生成 (ristretto_classfile)
│   ├── valenc/             # コンパイラ CLI
│   ├── valen-lsp/          # LSP サーバー
│   └── valenfmt/           # コードフォーマッタ
├── docs/
│   ├── requirements/       # 要件定義書（REQ-{SCOPE}-{SEQ}）
│   ├── specifications/     # アーキテクチャ仕様
│   ├── implementation/     # 実装計画・タスク（TASK-{SEQ}）
│   ├── spec/               # 言語仕様詳細（01-lexical 〜 20-annotations）
│   ├── LANGUAGE_SPEC.md    # 仕様インデックス
│   └── IMPLEMENTATION_PLAN.md # Phase 計画
├── gradle-plugin/          # Gradle プラグイン（Kotlin、未着手）
├── stdlib/                 # Valen stdlib（.vln、未着手）
├── examples/               # サンプルコード（未着手）
├── CLAUDE.md
└── LICENSE
```

## 開発方針

- コミットメッセージは英語、タイトル short、本文で「何を」「なぜ」を1-2段落。conventional prefix (`feat:` `fix:` `chore:` `build:` `docs:` `refactor:` `test:`)
- PR は MVP 機能1単位で小さく
- Codex 3巡レビューの「82/100」判定は設計凍結ではない — 実装中に発見した仕様穴は遠慮なく上げる
- enum bytecode emit 戦略は実装前に必ず検証実験を走らせる（Java pattern switch / Jackson / reflection / Gradle incremental）

## コミット前チェックリスト

コードに触れた後は必ず以下を通してからコミット。1つでも失敗したら修正する。skip 禁止。

```sh
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
cargo build --workspace  # リリース前は --release でも確認
```

- `cargo check` は型と借用チェック、最速のゲート
- `cargo clippy -D warnings` で warning も error 扱い
- `cargo fmt --check` は差分のみ検出、フォーマット崩れたら `cargo fmt --all` で修正
- `cargo test` は integration テスト含む
- エラーやテスト失敗を握り潰さない。`#[allow(...)]` や `#[ignore]` を貼るのは最終手段、貼るときはコメントで理由を書く

Git hook で自動化したい場合は `.git/hooks/pre-commit` に上記を仕込む（ローカル設定、コミット対象外）。

## 不明点の扱い

- 仕様が曖昧、Codex レビューで触れていない、複数解釈可能 — いずれも **ユーザに聞く**
- 「多分こうだろう」で進めない、特にコア4軸（ADT / match / trait / 失敗モデル）に関わる判断は確認必須
- 既存のメモリ（`memory/`）や仕様書（`docs/LANGUAGE_SPEC.md`）で答えが見つからない時点で質問する
- 質問する時は選択肢を提示して判断を促す（AskUserQuestion 推奨）、自由記述で聞き返さない
- 実装途中で設計矛盾を発見したら、勝手に直さず先に報告する。MVP/Phase 境界を越える変更は特に要相談

## よくある判断

- 「Kotlin に既にあるから Valen にも」→ 芯4点を補強しないなら却下
- 「Rust にあるから Valen にも」→ JVM 意味論と齟齬がないか確認
- 「拡張関数を入れたい」→ `Phase 1.5 で再評価` に送る、MVP では trait impl + UFCS
- 「named args と default args 両方欲しい」→ MVP は named のみ、default は Phase 1.5

## 参考：Codex による判定基準

過去3巡のCodexレビューで繰り返された指摘軸：
1. 思想の二重化（2つの同じ概念が並存していないか）
2. 差別化の芯が細いか太いか
3. MVP の広すぎ
4. キーワード選択の一貫性
5. interop 境界でのルール明確化

新機能提案時は自己診断として上記5軸でチェック。

## ドキュメント整合性

作業完了時、以下のドキュメントに影響する変更がないか確認し、必要なら更新するかユーザーに確認すること:

- `docs/requirements/` — 要件の追加・変更・ステータス更新（Draft → Done 等）
- `docs/specifications/` — アーキテクチャ変更（crate 追加、パイプライン変更等）
- `docs/implementation/plan.md` — タスク完了、新規タスク追加、マイルストーン進捗
- `docs/spec/` — 言語仕様の変更・追加
- `docs/IMPLEMENTATION_PLAN.md` — Phase 進捗の更新
- `docs/spec/18-open-questions.md` — 仕様課題の解決・追加

特に parser/HIR/codegen の実装が進んだ際は、対応する TASK の完了や REQ のステータス更新を忘れずに行う。
