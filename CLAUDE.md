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

## 確定事項（2026-04 時点）

- MVPスコープ、Phase計画は [docs/IMPLEMENTATION_PLAN.md](docs/IMPLEMENTATION_PLAN.md) を参照
- 構文・型システム詳細は [docs/LANGUAGE_SPEC.md](docs/LANGUAGE_SPEC.md) を参照
- Codex 3巡レビューの採用履歴は memory の `project_valen_codex_review.md` を参照

## ディレクトリ構成（予定）

```
valen-lang/
├── compiler/             # valenc（Rust）
│   ├── Cargo.toml
│   └── src/
├── gradle-plugin/        # Gradle プラグイン（Kotlin）
│   └── build.gradle.kts
├── lsp/                  # LSP server（Rust）
│   └── Cargo.toml
├── fmt/                  # valenfmt（Rust）
│   └── Cargo.toml
├── stdlib/               # Valen stdlib（.vln）
├── examples/             # サンプルコード
├── docs/                 # 仕様・実装計画
│   ├── LANGUAGE_SPEC.md
│   └── IMPLEMENTATION_PLAN.md
├── README.md
├── CLAUDE.md
└── LICENSE
```

## 開発方針

- コミットメッセージは日本語OK、概要 + 理由を1-2文
- PR は MVP 機能1単位で小さく
- Codex 3巡レビューの「82/100」判定は設計凍結ではない — 実装中に発見した仕様穴は遠慮なく上げる
- enum bytecode emit 戦略は実装前に必ず検証実験を走らせる（Java pattern switch / Jackson / reflection / Gradle incremental）

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
