# VEP-001: Self type in class and trait context

**Status:** Proposed  
**Author:** NaruseNia  
**Date:** 2026-05-15

## Summary

Allow `Self` as a type alias for the enclosing class/trait type, including generic parameters.

## Motivation

```valen
class Generic<T> {
    fn new() -> Generic<T> {  // verbose, repeats name + params
        Generic()
    }
}
```

With `Self`:
```valen
class Generic<T> {
    fn new() -> Self {  // Self = Generic<T>
        Generic()
    }
}
```

## Design

- `Self` resolves to the enclosing type with its generic parameters applied
- In `class Foo<T>`: `Self` = `Foo<T>`
- In `trait Bar<T>`: `Self` = the implementing type
- In `impl Bar for Foo<Int>`: `Self` = `Foo<Int>`
- `Self` is only valid inside class/trait/impl bodies

## Prior Art

- Rust: `Self` in impl blocks and trait definitions
- Swift: `Self` in protocol and class contexts
- Kotlin: No direct equivalent (uses explicit type names)

## Implementation Notes

- `TyRef::SelfTy` already exists in valen-hir
- Parser needs to recognize `Self` as a type in class/trait/impl bodies
- Type checker needs to resolve `SelfTy` to the concrete type with generics
