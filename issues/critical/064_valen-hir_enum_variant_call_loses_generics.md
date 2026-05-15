# 064: Enum variant call loses generic type arguments

**Severity:** Critical  
**Crate:** valen-hir  
**File:** crates/valen-hir/src/ty.rs (synth_path, line ~723)

## Description

When calling an enum variant like `Option::Some(value)`, the return type is
`Ty::Named("Option")` instead of `Ty::Generic("Option", vec![typeof(value)])`.

This causes type checking to report:
```
expected `Option<String>`, found `Option` [V0300]
```

## Reproduction

```valen
impl Fail for Rect {
    fn fail(self) -> Option<String> {
        Option::Some(Shape::staticLike())
    }
}
```

`Shape::staticLike()` returns `String`, so `Option::Some(String)` should produce
`Option<String>`, but the type checker infers bare `Option`.

## Root Cause

`synth_path` (line 723) returns `Ty::Named(first.clone())` for enum variant paths.
It does not propagate generic type arguments from the variant's field types to the
enum's type parameters.

## Fix

When resolving `EnumName::Variant(args...)`:
1. Look up the enum's generic parameters
2. Infer type arguments from the variant's fields and provided arguments
3. Return `Ty::Generic(enum_name, inferred_args)` instead of `Ty::Named(enum_name)`
