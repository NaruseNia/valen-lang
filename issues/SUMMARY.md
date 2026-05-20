# Refactor Audit Summary

**Date:** 2026-05-20
**Target:** valen-ast, valen-parser, valen-hir, valen-codegen, valen-lsp, valen-diagnostics, valenc, valenfmt
**Files scanned:** 41
**Lines scanned:** ~29,742

## Overview

| Severity | Count |
|----------|-------|
| critical | 6 |
| major | 53 |
| minor | 6 |
| enhancement | 6 |
| **Total** | **71** |

## By Dimension

| Dimension | Count |
|-----------|-------|
| correctness | 16 |
| spec_coverage | 28 |
| test_coverage | 8 |
| design | 6 |
| performance | 7 |
| error_handling | 4 |
| idiomatic_rust | 2 |

## By Crate

| Crate | Critical | Major | Minor | Enhancement | Total |
|-------|----------|-------|-------|-------------|-------|
| valen-ast | 0 | 4 | 2 | 2 | 8 |
| valen-codegen | 1 | 10 | 0 | 4 | 15 |
| valen-diagnostics | 0 | 2 | 0 | 0 | 2 |
| valen-hir | 3 | 10 | 1 | 0 | 14 |
| valen-lsp | 0 | 13 | 1 | 0 | 14 |
| valen-parser | 0 | 8 | 2 | 0 | 10 |
| valenc | 1 | 4 | 0 | 0 | 5 |
| valenfmt | 1 | 2 | 0 | 0 | 3 |

## Issue List

| # | Severity | Dimension | Scope | Title |
|---|----------|-----------|-------|-------|
| 001 | critical | correctness | valen-codegen | [for_loop_iterator_continue_label_no_frame](critical/001_valen-codegen_for_loop_iterator_continue_label_no_frame.md) |
| 002 | critical | correctness | valen-hir | [inherent_impl_blocks_from_stdlib_not_registered](critical/002_valen-hir_inherent_impl_blocks_from_stdlib_not_registered.md) |
| 003 | critical | spec_coverage | valen-hir | [no_open_class_inheritance_check](critical/003_valen-hir_no_open_class_inheritance_check.md) |
| 004 | critical | spec_coverage | valen-hir | [no_override_fn_validation](critical/004_valen-hir_no_override_fn_validation.md) |
| 005 | critical | correctness | valenc | [target_jvm_version_silently_ignored](critical/005_valenc_target_jvm_version_silently_ignored.md) |
| 006 | critical | correctness | valenfmt | [unsafe_expr_missing_from_expr_ends_with_block](critical/006_valenfmt_unsafe_expr_missing_from_expr_ends_with_block.md) |
| 007 | major | spec_coverage | valen-ast | [int_literal_uses_i64_instead_of_i32](major/007_valen-ast_int_literal_uses_i64_instead_of_i32.md) |
| 008 | major | spec_coverage | valen-ast | [missing_hex_and_binary_literal_token_kinds](major/008_valen-ast_missing_hex_and_binary_literal_token_kinds.md) |
| 009 | major | correctness | valen-ast | [span_merge_panics_in_production_code](major/009_valen-ast_span_merge_panics_in_production_code.md) |
| 010 | major | test_coverage | valen-ast | [test_coverage_only_span_module](major/010_valen-ast_test_coverage_only_span_module.md) |
| 011 | major | spec_coverage | valen-codegen | [break_with_value_not_implemented](major/011_valen-codegen_break_with_value_not_implemented.md) |
| 012 | major | spec_coverage | valen-codegen | [convert_methods_not_implemented_in_codegen](major/012_valen-codegen_convert_methods_not_implemented_in_codegen.md) |
| 013 | major | error_handling | valen-codegen | [integer_literal_panic_on_overflow](major/013_valen-codegen_integer_literal_panic_on_overflow.md) |
| 014 | major | design | valen-codegen | [iterator_intrinsics_eager_not_lazy](major/014_valen-codegen_iterator_intrinsics_eager_not_lazy.md) |
| 015 | major | spec_coverage | valen-codegen | [main_fn_not_emitted_as_jvm_entry_point](major/015_valen-codegen_main_fn_not_emitted_as_jvm_entry_point.md) |
| 016 | major | correctness | valen-codegen | [method_call_on_interface_uses_invokevirtual](major/016_valen-codegen_method_call_on_interface_uses_invokevirtual.md) |
| 017 | major | correctness | valen-codegen | [missing_acc_super_on_several_classes](major/017_valen-codegen_missing_acc_super_on_several_classes.md) |
| 018 | major | test_coverage | valen-codegen | [no_test_for_fn_main_entry_point](major/018_valen-codegen_no_test_for_fn_main_entry_point.md) |
| 019 | major | spec_coverage | valen-codegen | [println_print_only_accepts_string](major/019_valen-codegen_println_print_only_accepts_string.md) |
| 020 | major | spec_coverage | valen-codegen | [top_level_fn_not_emitted](major/020_valen-codegen_top_level_fn_not_emitted.md) |
| 021 | major | spec_coverage | valen-diagnostics | [diagcode_constants_unused_in_codebase](major/021_valen-diagnostics_diagcode_constants_unused_in_codebase.md) |
| 022 | major | design | valen-diagnostics | [labels_and_notes_never_populated](major/022_valen-diagnostics_labels_and_notes_never_populated.md) |
| 023 | major | spec_coverage | valen-hir | [any_type_not_in_prelude](major/023_valen-hir_any_type_not_in_prelude.md) |
| 024 | major | spec_coverage | valen-hir | [data_class_no_superclass_support_in_hir](major/024_valen-hir_data_class_no_superclass_support_in_hir.md) |
| 025 | major | design | valen-hir | [exhaustive_check_operates_on_ast_not_hir](major/025_valen-hir_exhaustive_check_operates_on_ast_not_hir.md) |
| 026 | major | correctness | valen-hir | [field_access_ignores_generic_type_substitution](major/026_valen-hir_field_access_ignores_generic_type_substitution.md) |
| 027 | major | performance | valen-hir | [method_resolution_linear_scan_all_defs](major/027_valen-hir_method_resolution_linear_scan_all_defs.md) |
| 028 | major | spec_coverage | valen-hir | [no_entry_point_validation](major/028_valen-hir_no_entry_point_validation.md) |
| 029 | major | test_coverage | valen-hir | [no_test_for_numeric_conversion](major/029_valen-hir_no_test_for_numeric_conversion.md) |
| 030 | major | test_coverage | valen-hir | [no_test_for_option_result_methods](major/030_valen-hir_no_test_for_option_result_methods.md) |
| 031 | major | spec_coverage | valen-hir | [numeric_conversion_methods_missing_byte_short_char](major/031_valen-hir_numeric_conversion_methods_missing_byte_short_char.md) |
| 032 | major | spec_coverage | valen-hir | [visibility_internal_not_enforced](major/032_valen-hir_visibility_internal_not_enforced.md) |
| 033 | major | spec_coverage | valen-lsp | [builtin_functions_filtered_from_completion](major/033_valen-lsp_builtin_functions_filtered_from_completion.md) |
| 034 | major | design | valen-lsp | [completion_hover_documentation_inconsistent](major/034_valen-lsp_completion_hover_documentation_inconsistent.md) |
| 035 | major | performance | valen-lsp | [full_reparse_on_every_keystroke](major/035_valen-lsp_full_reparse_on_every_keystroke.md) |
| 036 | major | spec_coverage | valen-lsp | [hover_lacks_rich_variable_info](major/036_valen-lsp_hover_lacks_rich_variable_info.md) |
| 037 | major | spec_coverage | valen-lsp | [inlay_hints_limited](major/037_valen-lsp_inlay_hints_limited.md) |
| 038 | major | spec_coverage | valen-lsp | [no_impl_trait_only_context](major/038_valen-lsp_no_impl_trait_only_context.md) |
| 039 | major | spec_coverage | valen-lsp | [no_import_path_completion](major/039_valen-lsp_no_import_path_completion.md) |
| 040 | major | spec_coverage | valen-lsp | [no_java_stdlib_completion](major/040_valen-lsp_no_java_stdlib_completion.md) |
| 041 | major | spec_coverage | valen-lsp | [no_package_info_in_completions](major/041_valen-lsp_no_package_info_in_completions.md) |
| 042 | major | test_coverage | valen-lsp | [no_tests_for_completion_hover_inlay](major/042_valen-lsp_no_tests_for_completion_hover_inlay.md) |
| 043 | major | spec_coverage | valen-lsp | [no_trait_method_stubs_in_impl](major/043_valen-lsp_no_trait_method_stubs_in_impl.md) |
| 044 | major | spec_coverage | valen-lsp | [override_keyword_missing_from_completion](major/044_valen-lsp_override_keyword_missing_from_completion.md) |
| 045 | major | correctness | valen-lsp | [scope_filtering_insufficient](major/045_valen-lsp_scope_filtering_insufficient.md) |
| 046 | major | correctness | valen-parser | [fstring_interpolation_error_spans_wrong_location](major/046_valen-parser_fstring_interpolation_error_spans_wrong_location.md) |
| 047 | major | correctness | valen-parser | [fstring_silently_drops_interpolation](major/047_valen-parser_fstring_silently_drops_interpolation.md) |
| 048 | major | spec_coverage | valen-parser | [generic_type_params_in_path_expr_not_supported](major/048_valen-parser_generic_type_params_in_path_expr_not_supported.md) |
| 049 | major | correctness | valen-parser | [let_else_backtracking_leaks_diagnostics](major/049_valen-parser_let_else_backtracking_leaks_diagnostics.md) |
| 050 | major | spec_coverage | valen-parser | [missing_hex_binary_octal_integer_literals](major/050_valen-parser_missing_hex_binary_octal_integer_literals.md) |
| 051 | major | test_coverage | valen-parser | [no_lexer_tests_for_char_long_float_fstring](major/051_valen-parser_no_lexer_tests_for_char_long_float_fstring.md) |
| 052 | major | test_coverage | valen-parser | [no_parser_tests_for_unsafe_safe_cast_deref_refmut](major/052_valen-parser_no_parser_tests_for_unsafe_safe_cast_deref_refmut.md) |
| 053 | major | correctness | valen-parser | [self_parameter_operator_precedence_unclear](major/053_valen-parser_self_parameter_operator_precedence_unclear.md) |
| 054 | major | error_handling | valenc | [diagnostic_severity_not_displayed](major/054_valenc_diagnostic_severity_not_displayed.md) |
| 055 | major | correctness | valenc | [emit_diagnostics_fallback_wrong_file](major/055_valenc_emit_diagnostics_fallback_wrong_file.md) |
| 056 | major | error_handling | valenc | [exit_code_heuristic_fragile](major/056_valenc_exit_code_heuristic_fragile.md) |
| 057 | major | spec_coverage | valenc | [output_flag_name_differs_from_spec](major/057_valenc_output_flag_name_differs_from_spec.md) |
| 058 | major | correctness | valenfmt | [comment_extractor_unaware_of_fstrings](major/058_valenfmt_comment_extractor_unaware_of_fstrings.md) |
| 059 | major | test_coverage | valenfmt | [no_tests_for_m14_expressions](major/059_valenfmt_no_tests_for_m14_expressions.md) |
| 060 | minor | idiomatic_rust | valen-ast | [no_eq_partialeq_on_ast_nodes](minor/060_valen-ast_no_eq_partialeq_on_ast_nodes.md) |
| 061 | minor | design | valen-ast | [type_span_returns_option_unnecessarily](minor/061_valen-ast_type_span_returns_option_unnecessarily.md) |
| 062 | minor | spec_coverage | valen-hir | [classpath_scan_only_java_javax_org](minor/062_valen-hir_classpath_scan_only_java_javax_org.md) |
| 063 | minor | design | valen-lsp | [server_rs_3159_line_monolith](minor/063_valen-lsp_server_rs_3159_line_monolith.md) |
| 064 | minor | error_handling | valen-parser | [describe_token_function_incomplete](minor/064_valen-parser_describe_token_function_incomplete.md) |
| 065 | minor | correctness | valen-parser | [impl_block_functions_always_pub_visibility](minor/065_valen-parser_impl_block_functions_always_pub_visibility.md) |
| 066 | enhancement | performance | valen-ast | [large_expr_enum_size](enhancement/066_valen-ast_large_expr_enum_size.md) |
| 067 | enhancement | idiomatic_rust | valen-ast | [missing_display_impl_for_key_types](enhancement/067_valen-ast_missing_display_impl_for_key_types.md) |
| 068 | enhancement | performance | valen-codegen | [scope_slots_never_reclaimed](enhancement/068_valen-codegen_scope_slots_never_reclaimed.md) |
| 069 | enhancement | performance | valen-codegen | [string_concat_not_using_invokedynamic](enhancement/069_valen-codegen_string_concat_not_using_invokedynamic.md) |
| 070 | enhancement | performance | valen-codegen | [synthetic_classes_always_emitted](enhancement/070_valen-codegen_synthetic_classes_always_emitted.md) |
| 071 | enhancement | performance | valen-codegen | [unit_only_enum_not_optimized](enhancement/071_valen-codegen_unit_only_enum_not_optimized.md) |

## Filed To

- ローカル md: `./issues/` ディレクトリ（71 ファイル）
