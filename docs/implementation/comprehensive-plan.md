# 包括実装計画: Phase 1.5 完了 + Phase 2

## 概要

ECS テストプロジェクト（`examples/ecs-system`）から発見された Issue 17件、Accepted VEP 8件、および Phase 1/1.5 の残タスク 5件を統合した包括的な実装ロードマップ。

**起点:** 2026-05-17（本計画策定日）
**前提:** Phase 1 ほぼ完了、Phase 1.5 M6/M7 完了、M8 部分完了
**ゴール:** Valen で実用的なプロジェクト（ECS 等）を型安全に書ける状態
**総期間目安:** 10-15 週（2.5-4 ヶ月）

---

## 現状サマリ

| フェーズ | 完了 | 残 | 備考 |
|---------|------|-----|------|
| Phase 1 (MVP) | 15/17 | TASK-013, 016 | Gradle plugin + サンプル |
| Phase 1.5 M6 | 5/5 | — | stub 全潰し完了（StubBody 除去済み） |
| Phase 1.5 M7 | 4/4 | — | 基盤整備完了 |
| Phase 1.5 M8 | 6/7 | TASK-033 | TASK-028 ✅, TASK-031 ✅ |
| M9 | 5/5 | — | ✅ Critical Fixes 完了 |
| M9.5 | 2/2 | — | ✅ Bug Polish 完了 |
| M10 | 4/4 | — | ✅ Practical Usability 完了 |
| 要件 | 42/45 Done | 3 Draft | REQ-EMIT-004, REQ-TOOL-002, REQ-STDLIB-003 |

---

## マイルストーン構成

```
M9: Critical Fixes ──── M9.5: Bug Polish ─┐
                                           ├── M10: Practical Usability
M11: Phase 1/1.5 Closure ─────────────────┤
                                           ▼
                                  M12: Phase 2 Core
                                           │
                                           ▼
                                  M13: Phase 2 Extended
                                           │
                                           ▼
                                  M14: Phase 2 Safety
```

---

## M9: Critical Fixes（1 週）

ECS テストプロジェクトが動作しない原因となるクリティカルバグ、? 演算子の実用問題、および M6 残存 StubBody の掃除。

| タスク | 内容 | Issue | 重度 | 影響 crate | 規模 |
|--------|------|-------|------|-----------|------|
| TASK-034 | enum variant destructuring のバインド変数が未宣言扱い | [#106](https://github.com/NaruseNia/valen-lang/issues/106) | Critical | valen-hir (resolve/type_check) | S |
| TASK-035 | e2e テストが type_check diagnostics を検証していない | [#107](https://github.com/NaruseNia/valen-lang/issues/107) | Critical | valen-codegen (tests) | S |
| TASK-036 | Java 型コンストラクタが式コンテキストで V0301 undeclared | [#108](https://github.com/NaruseNia/valen-lang/issues/108) | Critical | valen-hir (resolve) | M |
| TASK-038 | ? 演算子で Option 早期リターンが動作しない | [#118](https://github.com/NaruseNia/valen-lang/issues/118) | Major | valen-hir (type_check) | M |
| TASK-034a | codegen 残存 StubBody 掃除（3箇所） | — | — | valen-codegen | S |

### TASK-034: enum variant destructuring fix

**問題:** `match` 式で `Color::Blue(value)` のように destructure すると、arm body 内で `value` が `V0301: undeclared variable` になる。

**原因候補:** resolve パスまたは type_check パスで、variant のバインド変数をスコープに登録していない。

**修正方針:**
1. `resolve.rs` の match arm パターン処理でバインド変数をローカルスコープに追加
2. `type_check.rs` で同変数の型を variant フィールド型から推論
3. テストケース: `tests/fixtures/codegen/enum_destructure_bind.vln` 追加

### TASK-035: e2e test type_check verification

**問題:** `compile_fixture_outputs` が `type_check` を実行するが diagnostics を assert していない。型エラーが無視されたまま codegen に進む。

**修正方針:**
1. `e2e_fixtures.rs` に type_check diagnostics の error-level 検証を追加
2. warning は許容、error が 1 件でもあれば fixture を fail
3. 既存 fixture で隠れていた型エラーを洗い出し修正

### TASK-036: Java constructor in expression context

**問題:** `import java.util.ArrayList;` 後、`let list = ArrayList();` が `V0301: undeclared variable ArrayList` になる。型位置では認識されるが、式位置でのコンストラクタ呼び出しとして解決されない。

**修正方針:**
1. resolve パスで foreign type を式位置でもコンストラクタ呼び出しとして解決
2. コンストラクタ引数の型チェックを classpath 情報と照合
3. codegen で `invokespecial <init>` を emit

### TASK-038: ? operator for Option

**問題:** `?` 演算子が `Result<T, E>` では動作するが `Option<T>` では型チェッカーが正しく処理できない。

**修正方針:**
1. type_check の `?` 処理で `Option<T>` の場合のデシュガー（`match { Some(v) => v, None => return None }`）を追加
2. 戻り値型との整合性チェック（関数が `Option<U>` を返すことを検証）
3. `try_operator.vln` fixture を修正・拡充

### TASK-034a: StubBody 掃除

**問題:** Phase 1.5 M6 の完了条件「StubBody が codegen から消滅」が未達。`expr.rs:536`（lambda 3+ params）、`expr.rs:757`、`lower.rs:555` に残存。

**修正方針:**
- lambda 3+ params: 明確なエラーメッセージで拒否、StubBody を除去
- 他: 実装するか、到達不能なら unreachable! に置換

**完了条件:** 全 Critical issue が解消。`mise run test` で全 e2e fixture green。StubBody が codegen から消滅。

---

## M9.5: Bug Polish（0.5-1 週、M9 依存）

Major/Minor バグの修正。実用的なコード記述の前提条件。

| タスク | 内容 | Issue | 重度 | 影響 crate | 規模 |
|--------|------|-------|------|-----------|------|
| TASK-037 | マルチファイルコンパイル時のエラー位置が最初のファイルに帰属 | [#112](https://github.com/NaruseNia/valen-lang/issues/112) | Major | valen-hir, valenc | M |
| TASK-039 | return 文が関数内で使えない可能性の検証・修正 | [#119](https://github.com/NaruseNia/valen-lang/issues/119) | Minor | valen-parser, valen-hir | S |

### TASK-037: Multi-file error attribution

**問題:** 複数 `.vln` ファイルをコンパイル時、resolve/type_check のエラーが全て最初のファイルのパスで報告される。

**修正方針:**
1. HIR lowering で各宣言の source file 情報を保持（`Span` にファイル ID 追加 or source map 導入）
2. diagnostic 出力でファイル ID からパスを復元
3. multi-file テストケースを追加

### TASK-039: return statement verification

**修正方針:**
1. 既存 fixture に `return expr;` / `return;` / 早期 return パターンの追加
2. パーサーが return を正しく扱うか確認、問題があれば修正
3. type_check で return 式の型が関数戻り値型と一致することを検証

**完了条件:** マルチファイルコンパイルで正しいファイル・行番号が報告される。return 文が全パターンで動作。

---

## M10: Practical Usability（1-1.5 週、M9.5 依存）

言語を実用的に書くために必要な基本機能の穴埋め。

| タスク | 内容 | Issue | 重度 | 影響 crate | 規模 |
|--------|------|-------|------|-----------|------|
| TASK-040 | println / print を prelude に追加 | [#116](https://github.com/NaruseNia/valen-lang/issues/116) | Major | valen-hir (prelude), valen-codegen | S |
| TASK-041 | Some/None/Ok/Err を prelude に追加（修飾なし利用） | [#109](https://github.com/NaruseNia/valen-lang/issues/109) | Major | valen-hir (resolve) | S |
| TASK-042 | JVM 予約語 `new` / コンテキストキーワード `data` の識別子許可 | [#113](https://github.com/NaruseNia/valen-lang/issues/113) | Major | valen-parser (lexer) | S |
| TASK-045 | f-string（文字列補間）レキサー + codegen 実装 | [#117](https://github.com/NaruseNia/valen-lang/issues/117) | Minor | valen-parser, valen-codegen | M |

### TASK-040: println / print prelude

**方針:**
- `println(value)` / `print(value)` を prelude 組み込み関数として追加
- codegen: 引数の `toString()` を呼び出し → `System.out.println` / `System.out.print` への直接 emit
- Any 型導入前でも `toString()` ベースで動作可能

### TASK-041: Prelude variant names

**方針:**
- resolve パスの prelude 注入で `Some` → `Option::Some`、`None` → `Option::None`、`Ok` → `Result::Ok`、`Err` → `Result::Err` のエイリアスを登録
- パターンマッチでも修飾なしで使用可能にする
- Rust の prelude と同等の挙動

### TASK-042: JVM reserved word handling

**方針:**
- レキサーで `new` を Valen キーワードから除外（Valen には `new` キーワード不要）
- `data` をコンテキストキーワード化（`data class` の位置でのみキーワード、他は識別子）
- JVM bytecode emit 時に JVM 予約語との衝突をエスケープ（`new` → codegen では別名 or そのまま emit して JVM が受け入れるか検証）

### TASK-045: f-string

**方針:**
1. レキサーに `f"..."` トークン追加
2. `{expr}` の位置でサブ式をパース
3. AST に `StringInterp(parts: Vec<StringPart>)` ノード追加
4. codegen: `invokedynamic StringConcatFactory` で高効率な文字列結合

**完了条件:** `println(f"Entity {id}: pos=({x}, {y})");` がコンパイル・実行可能。

---

## M11: Phase 1/1.5 Closure（2-3 週、M9 と並行可）

既存計画の残タスクを完了し、Phase 1/1.5 をクローズする。

| タスク | 内容 | 依存 | 規模 | 状態 |
|--------|------|------|------|------|
| TASK-028 | 演算子オーバーロード（trait ベース） | TASK-023 ✅ | M | 未着手 |
| TASK-031 | valenfmt 最小実装 | — | M | 未着手 |
| TASK-033 | stdlib 二層化 + 強化 | TASK-023 ✅, TASK-028 | L | 未着手 |
| TASK-050 | コレクション操作（map/filter/forEach/collect） | TASK-033 | M | 新規 |
| TASK-013 | Gradle プラグイン | — | M | 未着手 |
| TASK-016 | サンプルプロジェクト | TASK-013 | M | 未着手 |

### TASK-050: Collection operations

**Issue:** [#114](https://github.com/NaruseNia/valen-lang/issues/114) — コレクション操作の標準的手段がない

**依存:** TASK-033（stdlib 強化）の直後に実行

**方針:**
- TASK-033 で定義される Iterator trait の高階メソッド（`map`, `filter`, `forEach`, `fold`, `collect`, `count`）を実装
- Java コレクションへの Iterator impl は orphan rule の制約で直接不可 → stdlib のアダプタ関数（`iter(list)` → `ValenIterator`）で対応
- Issue #120 の短期対応（stdlib アダプタパターン）もここに含める

**実装順序:** TASK-028 → TASK-033 → TASK-050（依存チェーン）。TASK-031/013/016 は並行可。

**完了条件:**
- `mise run ci` green
- `impl Add for Vector2D` が動作（TASK-028）
- `valenfmt --check` が機能（TASK-031）
- Option/Result/Iterator の高階メソッドが使用可能（TASK-033 + 050）
- `gradle compileValen` でサンプルプロジェクトがビルド・実行可能（TASK-013 + 016）

---

## M12: Phase 2 Core — パターンマッチ拡張 & derive（3-4 週、M9 依存）

ECS テストプロジェクトが示した最大の痛点「match のネスト地獄」と「ボイラープレート」を解消する Phase 2 の核。M11 とは独立して M9 完了後すぐに着手可能。

| タスク | 内容 | Issue / VEP | 優先度 | 影響 crate | 規模 |
|--------|------|-------------|--------|-----------|------|
| TASK-046 | if let / while let | [#111](https://github.com/NaruseNia/valen-lang/issues/111), [VEP-029](https://github.com/NaruseNia/valen-lang/discussions/123) | Must | parser, hir, codegen | L |
| TASK-047 | let-else | [VEP-028](https://github.com/NaruseNia/valen-lang/discussions/124) | Must | parser, hir, codegen | L |
| TASK-048 | derive（Eq, Hash, Debug, Clone） | [VEP-013](https://github.com/NaruseNia/valen-lang/discussions/125) | Must | parser, hir, codegen | L |
| TASK-049 | enum variant 省略構文（.Some(x) 形式） | [#110](https://github.com/NaruseNia/valen-lang/issues/110) | Should | parser, hir (type inference) | M |

**実装順序:** TASK-046/047 → TASK-049（variant shorthand は if let との組み合わせが重要）。TASK-048 は独立して並行可。

### TASK-046: if let / while let

**構文（確定）:** Rust 式 `if let Pattern = expr`
```valen
if let Some(pos) = world.getComponent(entity, "Position") {
    println(f"x={pos.x}, y={pos.y}");
} else if let Some(vel) = world.getComponent(entity, "Velocity") {
    println(f"has velocity but no position");
} else {
    println("no components");
}

while let Some(entity) = iter.next() {
    process(entity);
}
```

**設計決定:**
- **else-if let チェーン**: 全許可（`if let ... else if let ... else if ... else`）
- **AST + HIR 両方に専用ノード**: LSP が元の構文を正確に表現。codegen も専用パス
- **ガード条件** (`if let P = e && cond`): Phase 3 送り

**実装方針:**
1. parser: `if` の後に `let Pattern = expr` を認識する分岐追加
2. AST: `IfLet { pattern, expr, then_block, else_block }` ノード追加
3. HIR: `TypedIfLet` 専用ノード（match への内部 desugar はしない）
4. type_check: パターンバインド変数の型推論、else ブロックの型統一
5. codegen: `IfLet` 専用パス（パターンマッチ → 条件分岐 emit）

### TASK-047: let-else

**構文（確定）:** Rust 式
```valen
let Some(health) = world.getComponent(entity, "Health") else { return; };
let Ok(data) = readFile(path) else { panic("read failed"); };
```

**設計決定:**
- **AST + HIR 両方に専用ノード**: TASK-046 と同じ方針
- else ブロックは必ず diverge（型は `Never`/`Nothing`）

**実装方針:**
1. parser: `let Pattern = expr else { diverge }` を認識
2. AST: `LetElse { pattern, expr, else_block }` ノード追加
3. HIR: `TypedLetElse` 専用ノード
4. type_check: else ブロックの型が `Never`/`Nothing`（diverge）であることを検証
5. codegen: `LetElse` 専用パス

### TASK-048: derive

**構文:**
```valen
#[derive(Eq, Hash, Debug, Clone)]
pub data class Entity(pub id: Int);

#[derive(Eq, Debug)]
pub enum Color { Red, Green, Blue(value: Int) }
```

**設計決定:**
- **data class は暗黙に Eq/Hash/Debug/Clone を持つ**: data class の既存 Java `equals()`/`hashCode()`/`toString()` への trait ブリッジ。新規バイトコード生成なし
- **通常の class / enum**: `derive(Eq)` で Java `equals()` メソッド生成 + Eq trait impl を同時に emit
- **対応 trait（初期セット）:** `Eq`, `Hash`, `Debug`, `Clone`

**実装方針:**
1. parser: `#[derive(...)]` は TASK-030（annotation）基盤で既にパース可能
2. HIR: derive 対象の trait を特定し、フィールド構造から impl を自動生成
3. data class: `equals()` → `Eq::eq()` ブリッジ、`hashCode()` → `Hash::hash()` ブリッジ、`toString()` → `Debug::fmt()` ブリッジ
4. class/enum: フィールド比較コードを生成し Java メソッド + trait impl として emit

### TASK-049: enum variant shorthand

**構文:**
```valen
let color: Color = .Red;
match filter {
    .All(components) => { ... },
    .Any(components) => { ... },
    .None => { ... },
}
// if let との組み合わせ
if let .Some(pos) = world.getComponent(entity, "Position") { ... }
```

**実装方針:**
1. parser: `.Ident` / `.Ident(args)` を新しい式/パターンノードとして認識
2. type_check: 期待される型（expected type）から enum を推論し、variant を解決
3. 対応コンテキスト: `let` 右辺（型アノテーションあり）、match arm パターン、関数引数、if let パターン

**完了条件:** ECS テストプロジェクトの collision.vln が 4 段ネスト match → if let でフラットに書き直せる。derive で Entity の Eq/Hash が自動生成。

---

## M13: Phase 2 Extended — 型システム拡張 & 糖衣構文（3-4 週、M12 依存）

型システムの表現力拡大と開発体験の向上。

| タスク | 内容 | Issue / VEP | 優先度 | 影響 crate | 規模 |
|--------|------|-------------|--------|-----------|------|
| TASK-051 | コレクションリテラル `[1, 2, 3]` / `#{"a": 1}` | [VEP-030](https://github.com/NaruseNia/valen-lang/discussions/126) | Should | parser, hir, codegen | M |
| TASK-052 | パイプライン演算子 `x \|> f` | [VEP-005](https://github.com/NaruseNia/valen-lang/discussions/127) | Should | parser, hir, codegen | S |
| TASK-053 | Refinement / newtype | [VEP-011](https://github.com/NaruseNia/valen-lang/discussions/128) | Should | parser, hir, codegen | L |
| TASK-054 | Intersection constraints（`+` 構文の型チェック実装） | [VEP-012](https://github.com/NaruseNia/valen-lang/discussions/129) | Should | valen-hir (type_check) | S |
| TASK-043 | ジェネリクス付き Java コレクションの型追跡 | [#115](https://github.com/NaruseNia/valen-lang/issues/115) | Major | valen-hir (type_check, classpath) | L |
| TASK-044 | Any 型（java.lang.Object 相当） | [#122](https://github.com/NaruseNia/valen-lang/issues/122) | Enhancement | valen-hir (type system) | M |

### TASK-051: Collection literal

**構文（確定）:**
```valen
let list = [1, 2, 3];                          // List<Int> (ArrayList)
let scores = #{"alice": 100, "bob": 85};        // Map<String, Int> (HashMap)
let empty: List<String> = [];                   // 空リテラルは型アノテーション必須
let emptyMap: Map<String, Int> = #{};           // 空 Map も同様
```

**設計決定:**
- **List リテラル:** `[expr, ...]` — パーサーでブロック式との衝突なし
- **Map リテラル:** `#{key: value, ...}` — `#` プレフィックスで `{}` ブロックとの曖昧性を完全回避
- **Map ペア区切り:** `:` — JSON/Python 風。`#{}` 内なので型アノテーションの `:` とは文法上衝突しない

**実装方針:**
1. parser: `[expr, ...]` をリストリテラル、`#{expr: expr, ...}` を Map リテラルとしてパース
2. 型推論: 要素型の統一、ターゲット型による推論
3. codegen: `ArrayList` / `HashMap` の構築コードに desugar（`new ArrayList()` + `.add()` 連続呼び出し）

### TASK-052: Pipeline operator

**構文（確定）:**
```valen
world.query(filter)
    |> filterAlive(world)   // filterAlive(result, world)
    |> sortByLayer          // sortByLayer(result)
    |> renderAll(renderer); // renderAll(result, renderer)
```

**設計決定:**
- **desugar ルール:** 第1引数挿入（Elixir/F# 方式）。`x |> f(a, b)` → `f(x, a, b)`
- **優先度:** 最低の中置演算子

**実装方針:**
1. parser: `|>` を中置演算子として追加
2. AST: `Pipeline { lhs, rhs_fn, rhs_args }` ノード
3. HIR: desugar（`x |> f(a, b)` → `f(x, a, b)`）
4. codegen: desugar 後は通常の関数呼び出し

### TASK-053: Refinement / newtype

**構文（確定）:**
```valen
newtype EntityId = Int;
newtype ComponentName = String;

let eid = EntityId(42);           // コンストラクタ構文
let eid2: EntityId = 42.into();   // Into/From trait（自動生成）
let raw: Int = eid.into();        // アンラップも Into

impl Eq for EntityId { ... }     // OK: EntityId は自モジュール所有
```

**設計決定:**
- **ラップ:** `EntityId(42)` コンストラクタ構文 + `Into/From<Int>` trait 自動生成
- **アンラップ:** `.into()` によるアンラップ
- **型の独立性:** typealias とは異なり新しい型。orphan rule で自モジュール型扱い
- **JVM 表現:** class として emit（将来 Valhalla value class への移行パスあり）

**実装方針:**
1. parser: `newtype Name = Type;` 構文追加
2. HIR: typealias とは別の新型定義として処理（所有権あり、orphan rule で自モジュール型扱い）
3. codegen: JVM class として emit、`From<Inner>` / `Into<Inner>` の impl を自動生成
4. 暗黙変換なし

### TASK-054: Intersection constraints

**設計決定:** 既存の `+` 構文のまま型チェッカー実装のみ。`&` は追加しない（機能的に同等）。

```valen
fn process<T: System + EventHandler>(system: T, world: World) -> Unit { ... }
pub trait Queryable: Component + Eq { ... }
```

**実装方針:**
1. パーサー変更不要（`+` は既にパース可能）
2. type_check で複数 trait bound の同時検証を実装
3. メソッド探索で複数 trait のメソッドを統合

### TASK-043: Generic Java collection type tracking

**方針:**
1. classpath scanner で Java クラスの generic signature（`Signature` attribute）を解析
2. `ForeignClassInfo` に `type_params` フィールドを追加
3. `HashMap<Int, Position>` の `get()` 戻り値を `Position?`（erasure + null safety）として型付け
4. type_check で Java generics の型パラメータを追跡、メソッド呼び出しの戻り値型をインスタンスの型引数で具体化

### TASK-044: Any type

**方針:**
- `Any` を `java.lang.Object` に対応する組み込み型として prelude に追加
- 全ての型は暗黙に `Any` のサブタイプ
- `Any` へのアップキャストは暗黙、ダウンキャストは `unsafe`（VEP-001）または `is` チェック
- Java interop: Java メソッドの `Object` パラメータ/戻り値は `Any` にマップ

**完了条件:** ECS のクエリフィルタがコレクションリテラルで簡潔に書ける。パイプラインでデータフローが左→右に読める。newtype で Entity ID の型混同を防止。

---

## M14: Phase 2 Safety（1-2 週、M13 依存）

安全性境界の明示。

| タスク | 内容 | VEP | 優先度 | 影響 crate | 規模 |
|--------|------|-----|--------|-----------|------|
| TASK-055 | unsafe block / unsafe fn | [VEP-001](https://github.com/NaruseNia/valen-lang/discussions/130) | Should | parser, hir, codegen | M |

### TASK-055: unsafe block / unsafe fn

**構文:**
```valen
let pos = unsafe { obj as Position };  // unchecked cast
unsafe fn rawAccess(ptr: Long) -> Int { ... }  // unsafe 関数
```

**設計決定 — unsafe 内で許可される操作（初期セット）:**
1. **unchecked cast** (`obj as ConcreteType`): ClassCastException リスクのあるダウンキャスト
2. **Java exception 無視**: safe ブロックなしで Java メソッドを呼び出し、例外を catch しない
3. **null 使用**: `T?` でない型で null を扱う（JNI / Panama 連携で必要）

**safe ブロック（既存）との関係:** safe は `Result` でラップして安全に扱う。unsafe は保証なし（呼び出し側の責任）。

**実装方針:**
1. parser: `unsafe { ... }` ブロック + `unsafe fn` 宣言
2. HIR: unsafe コンテキストフラグ。上記3操作は unsafe コンテキスト外ではコンパイルエラー
3. `unsafe fn` 呼び出し元にも `unsafe` を要求

**完了条件:** unsafe キャストで Object → 具体型への変換が可能。

---

## Phase 3 送り

以下は本計画の対象外。Phase 2 完了後に検討する。

| 項目 | 理由 | 関連 Issue |
|------|------|-----------|
| extension adapter（Java 型への直接 trait impl） | orphan rule の根本変更が必要。短期は stdlib アダプタ (M11 TASK-033)、中期は newtype (M13 TASK-053) でカバー | #120 |
| if let ガード条件 (`if let P = e && cond`) | 構文の複雑さが増す。Phase 2 では非対応 | — |
| `suspend fn` / async | virtual thread 統合戦略の仕様化が必要 | — |
| `reified` 型パラメータ | inline fn の上位機能 | — |
| `inline fn` | Phase 2 では優先度低 | — |
| annotation processing (KSP 相当) | annotation 基盤が先 | — |
| DSL receiver lambda | extension と隔離する設計が必要 | — |

---

## タスク一覧（全量）

### 新規タスク（TASK-034 〜 055）

| ID | 内容 | Issue / VEP | マイルストーン | 依存 | 規模 | 状態 |
|----|------|-------------|--------------|------|------|------|
| TASK-034 | enum variant destructuring fix | #106 | M9 | — | S | ✅ PR #132 |
| TASK-034a | StubBody 掃除（codegen 残存 3箇所） | — | M9 | — | S | ✅ PR #133 |
| TASK-035 | e2e test type_check verification | #107 | M9 | — | S | ✅ PR #136 |
| TASK-036 | Java constructor expression fix | #108 | M9 | — | M | ✅ PR #134 |
| TASK-038 | ? operator for Option | #118 | M9 | — | M | ✅ PR #135 |
| TASK-037 | Multi-file error attribution | #112 | M9.5 | M9 | M | ✅ PR #138 |
| TASK-039 | return statement verification | #119 | M9.5 | M9 | S | ✅ PR #137 |
| TASK-040 | println/print prelude | #116 | M10 | M9.5 | S | ✅ PR #140 |
| TASK-041 | Prelude variant names | #109 | M10 | M9.5 | S | ✅ PR #139 |
| TASK-042 | JVM reserved word handling | #113 | M10 | — | S | ✅ PR #141 |
| TASK-045 | f-string | #117 | M10 | — | M | ✅ PR #142 |
| TASK-050 | Collection operations | #114 | M11 | TASK-033 | M |
| TASK-046 | if let / while let | #111, VEP-029 | M12 | M9 | L |
| TASK-047 | let-else | VEP-028 | M12 | M9 | L |
| TASK-048 | derive | VEP-013 | M12 | TASK-030 ✅ | L |
| TASK-049 | enum variant shorthand | #110 | M12 | TASK-046 | M |
| TASK-051 | Collection literal | VEP-030 | M13 | — | M |
| TASK-052 | Pipeline operator | VEP-005 | M13 | — | S |
| TASK-053 | Refinement / newtype | VEP-011 | M13 | — | L |
| TASK-054 | Intersection constraints（+ 型チェック） | VEP-012 | M13 | — | S |
| TASK-043 | Generic Java collection tracking | #115 | M13 | TASK-025 ✅ | L |
| TASK-044 | Any type | #122 | M13 | — | M |
| TASK-055 | unsafe block / unsafe fn | VEP-001 | M14 | — | M |

### 既存残タスク

| ID | 内容 | マイルストーン | 状態 |
|----|------|--------------|------|
| TASK-013 | Gradle プラグイン | M11 | 未着手 |
| TASK-016 | サンプルプロジェクト | M11 | 未着手 |
| TASK-028 | 演算子オーバーロード | M11 | ✅ 実装済み（M8 時点で完了、e2e テスト green） |
| TASK-031 | valenfmt 最小実装 | M11 | ✅ 実装済み（CLI + 32 テスト + --check モード） |
| TASK-033 | stdlib 二層化 + 強化 | M11 | 未着手 |

---

## トレーサビリティ: Issue → タスク

| Issue | タイトル | 重度 | タスク | マイルストーン |
|-------|---------|------|--------|--------------|
| #106 | enum variant destructuring バインド変数未宣言 | Critical | TASK-034 | M9 |
| #107 | e2e テスト type_check 未検証 | Critical | TASK-035 | M9 |
| #108 | Java コンストラクタ V0301 | Critical | TASK-036 | M9 |
| #109 | Prelude に Some/None/Ok/Err | Major | TASK-041 | M10 |
| #110 | enum variant 省略構文 | Enhancement | TASK-049 | M12 |
| #111 | if let / while let | Major | TASK-046 | M12 |
| #112 | マルチファイルエラー位置 | Major | TASK-037 | M9.5 |
| #113 | JVM 予約語問題 | Major | TASK-042 | M10 |
| #114 | コレクション操作不在 | Major | TASK-050 | M11 |
| #115 | ジェネリクス Java コレクション | Major | TASK-043 | M13 |
| #116 | println/print 不在 | Major | TASK-040 | M10 |
| #117 | f-string 未実装 | Minor | TASK-045 | M10 |
| #118 | ? 演算子 Option 不動作 | Major | TASK-038 | M9 |
| #119 | return 文不動作 | Minor | TASK-039 | M9.5 |
| #120 | Java 型 trait impl | Enhancement | (Phase 3) | — |
| #122 | Any 型 | Enhancement | TASK-044 | M13 |

## トレーサビリティ: VEP → タスク

| VEP | Discussion | タイトル | Phase | 優先度 | タスク | マイルストーン |
|-----|------------|---------|-------|--------|--------|--------------|
| VEP-028 | #124 | let-else | 2 | Must | TASK-047 | M12 |
| VEP-029 | #123 | if let / while let | 2 | Must | TASK-046 | M12 |
| VEP-013 | #125 | derive | 2 | Must | TASK-048 | M12 |
| VEP-030 | #126 | Collection literal | 2 | Should | TASK-051 | M13 |
| VEP-005 | #127 | Pipeline operator | 2 | Should | TASK-052 | M13 |
| VEP-011 | #128 | Refinement / newtype | 2 | Should | TASK-053 | M13 |
| VEP-012 | #129 | Intersection constraints | 2 | Should | TASK-054 | M13 |
| VEP-001 | #130 | unsafe block / unsafe fn | 2 | Should | TASK-055 | M14 |
| VEP-031 | #143 | Mutable Reference (ref mut T) | 2+ | Should | — | TBD |

## トレーサビリティ: タスク → 要件

| タスク | 新規/更新される要件 |
|--------|-------------------|
| TASK-034 | REQ-ADT-002 (exhaustive match の品質) |
| TASK-036 | REQ-INTEROP-001 (Java import) |
| TASK-037 | REQ-TOOL-001 (valenc CLI) |
| TASK-038 | REQ-FAIL-003 (? 演算子) |
| TASK-040 | REQ-STDLIB-001 (valen.core) |
| TASK-041 | REQ-STDLIB-001 (valen.core) |
| TASK-043 | REQ-INTEROP-001, REQ-TYPE-006 |
| TASK-044 | REQ-TYPE-001（新規: Any 型追加） |
| TASK-045 | REQ-SYNTAX-001（f-string 追加） |
| TASK-046 | 新規: REQ-SYNTAX-004（if let / while let） |
| TASK-047 | 新規: REQ-SYNTAX-005（let-else） |
| TASK-048 | 新規: REQ-TRAIT-007（derive） |
| TASK-049 | 新規: REQ-ADT-004（variant shorthand） |
| TASK-050 | REQ-STDLIB-002 (valen.collections) |
| TASK-051 | 新規: REQ-SYNTAX-006（collection literal） |
| TASK-052 | 新規: REQ-SYNTAX-007（pipeline operator） |
| TASK-053 | 新規: REQ-TYPE-009（newtype） |
| TASK-054 | REQ-TYPE-006（intersection constraint 追加） |
| TASK-055 | 新規: REQ-FAIL-006（unsafe block） |

---

## 構文設計サマリ（grill-me 確定事項）

本計画策定時の grill-me セッションで確定した構文設計:

| # | 論点 | 決定 | 関連タスク |
|---|------|------|-----------|
| S1 | if let 構文 | Rust 式 `if let Pattern = expr` | TASK-046 |
| S2 | else-if let チェーン | 全許可（`if let ... else if let ... else if ... else`） | TASK-046 |
| S3 | if let / let-else の AST/HIR 表現 | AST + HIR 両方に専用ノード（desugar しない） | TASK-046, 047 |
| S4 | if let ガード | Phase 3 送り（`if let P = e && cond` は非対応） | — |
| S5 | Map リテラル構文 | `#{"key": value}` — `#` プレフィックスで `{}` との曖昧性回避 | TASK-051 |
| S6 | List リテラル構文 | `[1, 2, 3]` — 衝突なし | TASK-051 |
| S7 | Pipeline desugar | 第1引数挿入: `x \|> f(a, b)` → `f(x, a, b)` | TASK-052 |
| S8 | newtype ラップ/アンラップ | コンストラクタ `EntityId(42)` + `Into/From` trait 自動生成 | TASK-053 |
| S9 | derive × data class | data class は暗黙に Eq/Hash/Debug/Clone 保持。Java equals() 等へのブリッジ | TASK-048 |
| S10 | unsafe 許可操作 | unchecked cast + exception 無視 + null 使用 | TASK-055 |
| S11 | Intersection constraints | 既存 `+` 構文のまま。`&` は追加しない | TASK-054 |

---

## VEP ステータス管理

以下の VEP は Discussion で Accepted ラベルだが、`docs/vep/` では `draft/` のまま。計画策定に伴い `accepted/` へ移動が必要:

| VEP | 現在の位置 | 移動先 |
|-----|-----------|--------|
| VEP-001 | draft/ | accepted/ |
| VEP-005 | draft/ | accepted/ |
| VEP-011 | draft/ | accepted/ |
| VEP-012 | draft/ | accepted/ |
| VEP-013 | draft/ | accepted/ |
| VEP-028 | draft/ | accepted/ |
| VEP-029 | draft/ | accepted/ |
| VEP-030 | draft/ | accepted/ |

---

## スケジュール概算

| マイルストーン | 期間 | 累計 | 並行可 |
|--------------|------|------|--------|
| M9: Critical Fixes | 1 週 | 1 週 | — |
| M9.5: Bug Polish | 0.5-1 週 | 1.5-2 週 | — |
| M10: Practical Usability | 1-1.5 週 | 2.5-3.5 週 | M11 と並行 |
| M11: Phase 1/1.5 Closure | 2-3 週 | 2.5-3.5 週 | M10 と並行 |
| M12: Phase 2 Core | 3-4 週 | 5-7.5 週 | M11 と並行可 |
| M13: Phase 2 Extended | 3-4 週 | 8-11.5 週 | — |
| M14: Phase 2 Safety | 1-2 週 | 9-13.5 週 | — |

**総期間目安:** 10-15 週（2.5-4 ヶ月）
**クリティカルパス:** M9 → M9.5 → M10 → M12 → M13 → M14

---

## 検証戦略

各マイルストーンで ECS テストプロジェクトを段階的に実装し、言語機能の実用性を検証する:

| マイルストーン | ECS 検証項目 |
|--------------|-------------|
| M9 完了 | 基本 enum/struct 定義・match がコンパイル可能。? 演算子が Option で動作 |
| M9.5 完了 | マルチファイルコンパイルで正しいエラー報告 |
| M10 完了 | println でデバッグ出力、Some/None 修飾なし利用、f-string |
| M11 完了 | Gradle ビルド、Iterator + 高階メソッドで for ループ |
| M12 完了 | if let でフラットなコード、derive で Eq/Hash 自動、.Some(x) 省略構文 |
| M13 完了 | `[1, 2, 3]` / `#{"a": 1}` リテラル、パイプライン、newtype で型安全 ID |
| M14 完了 | unsafe キャストで Object → 具体型変換 |

---

## 変更履歴

| 日付 | 変更 |
|------|------|
| 2026-05-17 | 初版策定。Issue 17件 + VEP 8件 + Phase 1/1.5 残5件を統合。grill-me レビューで構文設計 11 項目確定 |
| 2026-05-17 | M9/M9.5/M10 全タスク完了（11 PR マージ）。TASK-028, 031 既存完了を確認。VEP-031 (ref mut T) 起票 |
