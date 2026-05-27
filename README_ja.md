# Valen

**OOの上にADTを足すのでなく、ADTを中核に据えてJVMへ落とす言語。**

[English](README.md) | 日本語

Valen は Java/Kotlin 資産に乗る、ADT 中心の JVM 言語です。強い代数的データ型、exhaustive な `match`、trait ベースの抽象、整合した `Option`/`Result` 失敗モデル — この4点を芯として、Java と Kotlin の既存エコシステムを壊さずに表現します。

Valen は Kotlin 超えを主張しません。補完的な選択肢として、「ADT が本当に強い JVM 言語」を最小限の形で提供することを目標にしています。

---

## Hello, Valen

```valen
package com.example.hello;

import java.util.List;

data class User(name: String, mut age: Int);

enum Shape {
    Circle(r: Float),
    Rect(w: Float, h: Float),
    Point,
}

trait Area {
    fn area(self) -> Float;
}

impl Area for Shape {
    fn area(self) -> Float {
        match self {
            Shape::Circle(r) => 3.14159 * r * r,
            Shape::Rect(w, h) => w * h,
            Shape::Point => 0.0,
        }
    }
}

fn main() {
    let shapes: List<Shape> = List.of(
        Shape::Circle(r = 5.0),
        Shape::Rect(w = 3.0, h = 4.0),
        Shape::Point,
    );

    for s in shapes {
        println(f"area = {s.area()}")
    }
}
```

## 特徴

- **代数的データ型と exhaustive match** — `enum` は Rust 型の ADT、`match` は構造分解・ガード・範囲・or パターン・`@` 束縛・exhaustive check をサポート
- **整合した失敗モデル** — `Option = 欠如`、`Result = 回復可能失敗`、`Exception = FFI 境界の異常`、`panic = 契約違反`。役割が明確に分離、`?` 演算子で early return
- **trait ベース抽象** — orphan rule 厳格、同一 trait/type 対はグローバル一意、sealed trait、演算子オーバーロード
- **Java 完全相互運用** — `import java.util.List;`、`safe { }` で Java exception 境界を明示、classpath 認識コンパイル
- **インライン関数と reified ジェネリクス** — `inline fn` と `reified` 型パラメータで実行時型情報にアクセス
- **モダン構文** — `fn`, `let` / `let mut`, `match`, `::` (enum variant) + `.` (member), `f"文字列補間"`
- **JVM 21 baseline / 25 opt-in** — Valhalla 等の新機能は `--target 25` でオプトイン
- **ツーリング** — LSP サーバー（補完・ホバー・定義ジャンプ・診断・セマンティックハイライト）、コードフォーマッタ

## インストール

### スクリプト（Linux / macOS）

```sh
curl -fsSL https://raw.githubusercontent.com/NaruseNia/valen-lang/main/install.sh | bash
```

`valenc` と `valen-lsp` が `~/.valen/bin` にインストールされます。PATH に追加してください：

```sh
export PATH="$HOME/.valen/bin:$PATH"
```

### ソースからビルド

```sh
cargo install --path crates/valenc
cargo install --path crates/valen-lsp
```

### GitHub Release

[Releases](https://github.com/NaruseNia/valen-lang/releases) からビルド済みバイナリをダウンロード。Linux x64、macOS x64/arm64、Windows x64 に対応。

## 使い方

```sh
# .vln ファイルを .class にコンパイル
valenc compile src/main.vln -o out/

# 型チェックのみ（コード生成なし）
valenc check src/main.vln

# Java で実行
java -cp out/ com.example.Main

# ソースフォーマット
valenfmt src/main.vln
```

## ドキュメント

- [言語仕様](docs/LANGUAGE_SPEC.md) — 言語リファレンス
- [ユーザーガイド](docs/guide/) — チュートリアル形式の入門
- [コンパイラアーキテクチャ](docs/guide/09-compiler-architecture.md) — コントリビュータ向け内部構造

## ライセンス

[Apache License 2.0](LICENSE)
