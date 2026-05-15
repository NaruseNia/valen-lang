# Refactor Audit Summary — 2026-05-15

**Date:** 2026-05-15 (Thu)
**Target:** valen-ast, valen-parser, valen-hir, valen-codegen, valen-diagnostics, valenc, valen-lsp, valenfmt
**Files scanned:** 40
**Lines scanned:** 21,231

## Overview

| Severity | Count |
|----------|-------|
| critical | 7 |
| major | 59 |
| minor | 84 |
| enhancement | 17 |
| **Total** | **167** |

## By Dimension

| Dimension | Count |
|-----------|-------|
| correctness | 38 |
| spec_coverage | 27 |
| test_coverage | 34 |
| error_handling | 13 |
| design | 14 |
| idiomatic_rust | 16 |
| performance | 12 |
| documentation | 10 |
| naming | 0 |
| fixture_coverage | 3 |

## By Crate

| Crate | Critical | Major | Minor | Enhancement | Total |
|-------|----------|-------|-------|-------------|-------|
| valen-ast | 0 | 3 | 8 | 6 | 17 |
| valen-parser | 1 | 5 | 19 | 3 | 28 |
| valen-hir | 0 | 7 | 17 | 3 | 27 |
| valen-codegen | 3 | 8 | 12 | 0 | 23 |
| valen-diagnostics | 0 | 0 | 8 | 3 | 11 |
| valenc | 1 | 5 | 7 | 1 | 14 |
| valen-lsp | 1 | 11 | 12 | 0 | 24 |
| valenfmt | 1 | 3 | 14 | 5 | 23 |

## Critical Issues

| # | Scope | Title | Dimension |
|---|-------|-------|-----------|
| 001 | valen-parser | [Nested generics `>>` ambiguity causes parse failure](critical/001_valen-parser_nested_generics_shr_ambiguity.md) | correctness |
| 002 | valen-codegen | [load/store instruction panic on slot > 255](critical/002_valen-codegen_load_store_panic_slot_overflow.md) | correctness |
| 003 | valen-codegen | [emit_arith/emit_neg unreachable! panics in production](critical/003_valen-codegen_unreachable_panics_in_production.md) | error_handling |
| 004 | valen-codegen | [IInc i32-to-i16 silent truncation](critical/004_valen-codegen_iinc_i32_to_i16_truncation.md) | correctness |
| 005 | valenc | [Exhaustiveness check missing from compiler pipeline](critical/005_valenc_exhaustiveness_check_missing.md) | correctness |
| 006 | valenfmt | [Annotations silently dropped on most declarations](critical/006_valenfmt_annotations_not_printed.md) | correctness |
| 007 | valen-lsp | [offset_to_position panics on out-of-bounds](critical/007_valen-lsp_offset_to_position_panic.md) | correctness |

## Major Issues

| # | Scope | Title | Dimension |
|---|-------|-------|-----------|
| 008 | valen-parser | [Lexer missing Long literal suffix (L)](major/008_valen-parser_missing_long_literal.md) | spec_coverage |
| 009 | valen-parser | [Lexer missing Float suffix literal (f)](major/009_valen-parser_missing_float_suffix.md) | spec_coverage |
| 010 | valen-parser | [Lexer missing Char literal](major/010_valen-parser_missing_char_literal.md) | spec_coverage |
| 011 | valen-parser | [Lexer missing === and !== operators](major/011_valen-parser_missing_ref_equality_ops.md) | spec_coverage |
| 012 | valen-parser | [Generic variance always Invariant (in/out ignored)](major/012_valen-parser_variance_always_invariant.md) | spec_coverage |
| 013 | valen-hir | [Wrong DiagCode for duplicate definition](major/013_valen-hir_wrong_diagcode_duplicate_def.md) | correctness |
| 014 | valen-hir | [TyRef::Unresolved blindly maps to TypeParam](major/014_valen-hir_unresolved_maps_to_typeparam.md) | correctness |
| 015 | valen-hir | [Array type descriptor loses element info](major/015_valen-hir_array_type_loses_element.md) | correctness |
| 016 | valen-hir | [Vis::Internal treated as Pub](major/016_valen-hir_internal_vis_treated_as_pub.md) | spec_coverage |
| 017 | valen-hir | [check_impl passes empty class_name for method lookup](major/017_valen-hir_check_impl_empty_classname.md) | correctness |
| 018 | valen-hir | [? operator passes through on non-Option/Result types](major/018_valen-hir_try_no_error_on_invalid_type.md) | spec_coverage |
| 019 | valen-hir | [for loop variable defaults to Int for non-Range types](major/019_valen-hir_for_loop_always_int.md) | correctness |
| 020 | valen-hir | [synth_path only resolves first segment of multi-segment path](major/020_valen-hir_path_first_segment_only.md) | correctness |
| 021 | valen-hir | [Orphan check uses import names not definition origin](major/021_valen-hir_orphan_check_name_based.md) | correctness |
| 022 | valen-codegen | [StackMapTable stripped on verification failure](major/022_valen-codegen_stackmap_stripping.md) | correctness |
| 023 | valen-codegen | [max_stack approximate, can underestimate](major/023_valen-codegen_max_stack_approximate.md) | correctness |
| 024 | valen-codegen | [Pattern struct slot allocation ignores wide types](major/024_valen-codegen_pattern_slot_wide_types.md) | correctness |
| 025 | valen-codegen | [class_emit.rs duplicates emit.rs functionality](major/025_valen-codegen_class_emit_duplicated.md) | design |
| 026 | valen-codegen | [No error-case fixtures](major/026_valen-codegen_no_error_fixtures.md) | fixture_coverage |
| 027 | valen-codegen | [No unit tests for expr.rs (~1800 lines)](major/027_valen-codegen_no_expr_tests.md) | test_coverage |
| 028 | valen-codegen | [Lambda 3+ params silently produces wrong arity](major/028_valen-codegen_lambda_3plus_wrong_arity.md) | correctness |
| 029 | valen-codegen | [Non-local callee fallback leaves dangling stack value](major/029_valen-codegen_callee_stack_leak.md) | correctness |
| 030 | valen-codegen | [No codegen-level error diagnostics](major/030_valen-codegen_no_error_diagnostics.md) | error_handling |
| 031 | valen-codegen | [No bytecode-level correctness tests](major/031_valen-codegen_no_bytecode_tests.md) | test_coverage |
| 032 | valenc | [--target flag parsed but never forwarded to codegen](major/032_valenc_target_flag_ignored.md) | spec_coverage |
| 033 | valenc | [Exit codes not distinguished (1 vs 2)](major/033_valenc_exit_codes_not_distinguished.md) | spec_coverage |
| 034 | valenc | [Diagnostics attributed to first input file only](major/034_valenc_diagnostics_first_file_only.md) | correctness |
| 035 | valenc | [Coherence check passes empty imports](major/035_valenc_coherence_empty_imports.md) | correctness |
| 036 | valenc | [--target value not validated](major/036_valenc_target_not_validated.md) | error_handling |
| 037 | valenc | [No multi-file compilation tests](major/037_valenc_no_multi_file_tests.md) | test_coverage |
| 038 | valen-lsp | [FileId hardcoded to 0 for all documents](major/038_valen-lsp_fileid_hardcoded_zero.md) | correctness |
| 039 | valen-lsp | [Goto def resolves by name only, no scope awareness](major/039_valen-lsp_goto_def_name_only.md) | correctness |
| 040 | valen-lsp | [Full re-parse on every keystroke, no debounce](major/040_valen-lsp_no_incremental_parse.md) | performance |
| 041 | valen-lsp | [Workspace indexing blocks initialize response](major/041_valen-lsp_workspace_blocks_init.md) | performance |
| 042 | valen-lsp | [find_let_type_annotation returns early on first non-let line](major/042_valen-lsp_find_let_returns_early.md) | correctness |
| 043 | valen-lsp | [No tests for completion/hover/semantic tokens](major/043_valen-lsp_no_completion_tests.md) | test_coverage |
| 044 | valen-lsp | [Goto def tests only verify HIR name, not position](major/044_valen-lsp_goto_def_tests_incomplete.md) | test_coverage |
| 045 | valen-lsp | [semantic_tokens_full uses byte length not UTF-16](major/045_valen-lsp_semantic_tokens_byte_length.md) | correctness |
| 046 | valen-lsp | [Workspace files not re-analyzed on change](major/046_valen-lsp_no_cross_file_refresh.md) | correctness |
| 047 | valen-lsp | [Each document analyzed in isolation](major/047_valen-lsp_no_cross_file_resolution.md) | correctness |
| 048 | valen-lsp | [REQ-TOOL-003 acceptance criteria gaps](major/048_valen-lsp_req_tool_003_gaps.md) | spec_coverage |
| 049 | valen-ast | [Missing span() accessor methods causing code duplication](major/049_valen-ast_missing_span_accessors.md) | design |
| 050 | valen-ast | [Type::Tuple missing Span field](major/050_valen-ast_tuple_missing_span.md) | design |
| 051 | valen-ast | [No unit tests for AST types](major/051_valen-ast_no_ast_type_tests.md) | test_coverage |
| 052 | valen-ast | [DataClassDecl missing supertypes field](major/052_valen-ast_dataclass_missing_supertypes.md) | spec_coverage |
| 053 | valenfmt | [--check mode ignored for stdin input](major/053_valenfmt_check_ignored_stdin.md) | spec_coverage |
| 054 | valenfmt | [Nested block comments cause incorrect extraction](major/054_valenfmt_nested_block_comments.md) | correctness |
| 055 | valenfmt | [has_blank_line panics when from > to](major/055_valenfmt_has_blank_line_panic.md) | error_handling |
| 056 | valen-parser | [parse_primary missing LongLit/FloatLit/CharLit handling](major/056_valen-parser_primary_missing_literal_kinds.md) | correctness |

## Minor Issues (57–140)

| # | Scope | Title | Dimension |
|---|-------|-------|-----------|
| 057 | valen-parser | Lexer missing doc comment token | spec_coverage |
| 058 | valen-parser | Block comment nesting not supported in lexer | correctness |
| 059 | valen-parser | Stale deferred comment in lexer header | documentation |
| 060 | valen-parser | Operator precedence redundant guards (bitor/bitand) | correctness |
| 061 | valen-parser | recover_to_item_boundary missing @ and typealias | error_handling |
| 062 | valen-parser | No test for assignment operators | test_coverage |
| 063 | valen-parser | No test for bitwise operators | test_coverage |
| 064 | valen-parser | No test for safe block | test_coverage |
| 065 | valen-parser | No test for nested generics | test_coverage |
| 066 | valen-parser | No test for error recovery multi-item | test_coverage |
| 067 | valen-parser | No lexer test for block comments | test_coverage |
| 068 | valen-parser | No lexer test for integer overflow | test_coverage |
| 069 | valen-parser | No lexer test for string escape sequences | test_coverage |
| 070 | valen-parser | No test for generic function declarations | test_coverage |
| 071 | valen-parser | match_enum rest pattern untested | test_coverage |
| 072 | valen-parser | mut binding pattern untested | test_coverage |
| 073 | valen-parser | impl methods missing visibility/annotations | correctness |
| 074 | valen-parser | data class requires semicolon only (no body) | spec_coverage |
| 075 | valen-parser | Stmt::Expr variant never emitted by parser | correctness |
| 076 | valen-parser | No class field declarations in body | spec_coverage |
| 077 | valen-parser | parse_int silent overflow | error_handling |
| 078 | valen-parser | self_param_precedence needs parentheses | correctness |
| 079 | valen-parser | float regex no leading dot | correctness |
| 080 | valen-hir | Coherence impls cloned unnecessarily | idiomatic_rust |
| 081 | valen-hir | find_impl_method clones FnDef | idiomatic_rust |
| 082 | valen-hir | Classpath only scans java/javax/org packages | spec_coverage |
| 083 | valen-hir | Exhaustive check skips trait default methods | spec_coverage |
| 084 | valen-hir | No test for immutable assign | test_coverage |
| 085 | valen-hir | No test for private field access | test_coverage |
| 086 | valen-hir | No test for loop constructs in type checker | test_coverage |
| 087 | valen-hir | No test for try/safe expressions | test_coverage |
| 088 | valen-hir | No test for string interpolation type check | test_coverage |
| 089 | valen-hir | No test for lambda type check | test_coverage |
| 090 | valen-hir | get_body_by_name ignores name parameter | correctness |
| 091 | valen-hir | No doc comments on exhaustive checker internals | documentation |
| 092 | valen-hir | resolve build_method_index clones all defs | performance |
| 093 | valen-hir | exhaustive find_enum clones EnumDef | idiomatic_rust |
| 094 | valen-hir | No test for nested match exhaustiveness | test_coverage |
| 095 | valen-hir | No test for typealias resolution | test_coverage |
| 096 | valen-hir | Resolver pub methods on non-pub struct | idiomatic_rust |
| 097 | valen-hir | No test for annotation class resolution | test_coverage |
| 098 | valen-hir | classpath unwrap_or("") on constant pool | error_handling |
| 099 | valen-hir | No doc comment on CoherenceChecker | documentation |
| 100 | valen-hir | Resolver struct marked pub unnecessarily | idiomatic_rust |
| 101 | valen-codegen | data class equals returns Int instead of Boolean | correctness |
| 102 | valen-codegen | toString stack accounting fragile | correctness |
| 103 | valen-codegen | Missing pub doc comments on access structs | documentation |
| 104 | valen-codegen | Excessive .to_string() allocations | performance |
| 105 | valen-codegen | JvmVersion defined but never branched on | spec_coverage |
| 106 | valen-codegen | lower_hir silently skips DefKind variants | spec_coverage |
| 107 | valen-codegen | is_sealed_trait_def linear scan | performance |
| 108 | valen-codegen | collect_permitted_subclasses DataClass gap | spec_coverage |
| 109 | valen-codegen | generate_getter lacks doc comment | idiomatic_rust |
| 110 | valen-codegen | Data class vs regular class interface resolution inconsistent | design |
| 111 | valen-codegen | descriptor_to_field_type fallback | correctness |
| 112 | valen-codegen | lower_for_iterator redundant labels | correctness |
| 113 | valen-codegen | StubBody stack_delta returns 0 | correctness |
| 114 | valen-codegen | Vis::Internal maps to package-private undocumented | spec_coverage |
| 115 | valen-codegen | emit_convert silently returns empty vec | correctness |
| 116 | valen-diagnostics | thiserror dependency unused | idiomatic_rust |
| 117 | valen-diagnostics | Labels and notes fields always empty | design |
| 118 | valen-diagnostics | LSP and CLI bypass DiagCode::Display | idiomatic_rust |
| 119 | valen-diagnostics | Missing test for hint() method | test_coverage |
| 120 | valen-diagnostics | Missing test for push() method | test_coverage |
| 121 | valen-diagnostics | Missing test for owned IntoIterator | test_coverage |
| 122 | valen-diagnostics | Missing doc comments on len/is_empty/push | documentation |
| 123 | valen-diagnostics | DiagCode V0700 range undocumented | documentation |
| 124 | valen-diagnostics | CLI emit_diagnostics omits severity | spec_coverage |
| 125 | valenc | No test for file-not-found error | test_coverage |
| 126 | valenc | No test verifying specific exit codes | test_coverage |
| 127 | valenc | Unused deps smol_str and tracing | idiomatic_rust |
| 128 | valenc | valid_fn.vln fixture unused | fixture_coverage |
| 129 | valenc | LineIndex duplicated between valenc and LSP | documentation |
| 130 | valenc | has_byte_offset_pattern overly complex | idiomatic_rust |
| 131 | valenc | No --check test for type errors | test_coverage |
| 132 | valen-lsp | extract_word_at ASCII-only assumption | correctness |
| 133 | valen-lsp | extract_receiver_before_dot doesn't strip partial ident | correctness |
| 134 | valen-lsp | No .vln fixture files for tests | fixture_coverage |
| 135 | valen-lsp | No UTF-16 edge case tests | test_coverage |
| 136 | valen-lsp | No doc comments on helpers | documentation |
| 137 | valen-lsp | Dead ST_FUNCTION/ST_PARAMETER constants | idiomatic_rust |
| 138 | valen-lsp | Semantic token classification by casing heuristic | correctness |
| 139 | valen-lsp | HIR always Some — conditional type_check misleading | idiomatic_rust |
| 140 | valen-lsp | detect_context 'for' pattern matching fragile | correctness |
| 141 | valen-lsp | No LSP error codes in ResponseError | error_handling |
| 142 | valen-lsp | didClose removes workspace re-index entry | correctness |
| 143 | valen-lsp | Missing keywords in completion list | spec_coverage |
| 144 | valen-ast | Span::merge uses assert not debug_assert | error_handling |
| 145 | valen-ast | Spanned<T> exported but unused | idiomatic_rust |
| 146 | valen-ast | TokenKind PartialEq without Eq (float) | correctness |
| 147 | valen-ast | EnumDecl missing body doc comment | spec_coverage |
| 148 | valen-ast | No module doc overview on lib.rs | documentation |
| 149 | valen-ast | Literal Float f32 precision risk | correctness |
| 150 | valen-ast | Span::len returns 0 for inverted spans | correctness |
| 151 | valenfmt | No block comment preservation test | test_coverage |
| 152 | valenfmt | sort_imports clones entire item list | performance |
| 153 | valenfmt | Comment text cloned in flush | idiomatic_rust |
| 154 | valenfmt | import_sort_key allocates for non-import | idiomatic_rust |
| 155 | valenfmt | No exit code for parse errors in --check | error_handling |
| 156 | valenfmt | No doc comments on FnCtx/ItemKind | documentation |
| 157 | valenfmt | write_indent uses loop instead of repeat | idiomatic_rust |
| 158 | valenfmt | No trailing semicolon removal test per expr type | test_coverage |
| 159 | valenfmt | Comment extractor no raw/multi-line string support | correctness |
| 160 | valenfmt | No import sorting stability test | test_coverage |

## Enhancement Issues (161–167)

| # | Scope | Title | Dimension |
|---|-------|-------|-----------|
| 161 | valen-ast | Item enum 208 bytes — Box large variants | performance |
| 162 | valen-ast | Stmt enum 160 bytes — Box Expr | performance |
| 163 | valen-ast | Pattern enum 96 bytes — Box large variants | performance |
| 164 | valen-ast | FileId inner field is pub | idiomatic_rust |
| 165 | valen-ast | Span fields all pub | idiomatic_rust |
| 166 | valen-diagnostics | No Extend impl for Diagnostics | design |
| 167 | valen-diagnostics | Diagnostic/Label lack PartialEq | idiomatic_rust |
| 168 | valen-diagnostics | No Severity::Info level | spec_coverage |
| 169 | valen-hir | Linear scan for type lookups | performance |
| 170 | valen-hir | Coherence duplicate detection quadratic | performance |
| 171 | valen-hir | Prelude injection repetitive boilerplate | design |
| 172 | valen-parser | Lexer diagnostics cloned individually | performance |
| 173 | valen-parser | describe_token incomplete | error_handling |
| 174 | valenc | Dual version paths (--version + subcommand) | design |
| 175 | valenfmt | has_blank_line performance (count all newlines) | performance |

## Codex CLI Supplemental Review (2026-05-15)

独立レビュー（Codex CLI / gpt-5.5）による追加指摘。既存167件と重複なし。

| # | Severity | Scope | Title | Dimension |
|---|----------|-------|-------|-----------|
| 057 | major | valen-lsp | [LSP workspace indexing follows symlinks outside workspace](major/057_valen-lsp_symlink_traversal.md) | security |
| 058 | major | valen-hir | [Classpath scanner が JAR ファイルを処理しない](major/058_valen-hir_classpath_no_jar_support.md) | spec_coverage |
| 059 | minor | valen-parser | [UTF-8 BOM がエラートークンとしてlexされる](minor/059_valen-parser_utf8_bom_error.md) | edge_case |
| 060 | major | valen-lsp | [didChange がインクリメンタル編集を全文置換として処理](major/060_valen-lsp_incremental_change_corruption.md) | concurrency |
| 061 | major | valenfmt | [フォーマッタがファイルを非原子的に書き込む](major/061_valenfmt_non_atomic_write.md) | correctness |
| 062 | minor | valen-ast | [ソースオフセットが u32::MAX 超でラップアラウンド](minor/062_valen-ast_u32_offset_overflow.md) | edge_case |
| 063 | enhancement | valen-lsp | [tokio "full" feature が不要](enhancement/063_valen-lsp_tokio_full_unnecessary.md) | dependency |

## Filed To

- Local md: `./issues/` — critical/major/minor/enhancement フォルダに分類
- `issues/critical/` (001–007), `issues/major/` (008–061), `issues/minor/` (059,062), `issues/enhancement/` (063)
