//! JVM type descriptor and internal name generation.

use smol_str::SmolStr;
use valen_hir::{PrimTy, TyRef};

use crate::jvm_ir::JvmType;

/// Converts a HIR type reference to its JVM type representation.
pub fn tyref_to_jvm(tyref: &TyRef, package: Option<&[SmolStr]>) -> JvmType {
    match tyref {
        TyRef::Prim(p) => prim_to_jvm(p),
        TyRef::Named(name) => JvmType::Object(class_internal_name(name, package)),
        TyRef::Generic(name, _) => JvmType::Object(class_internal_name(name, package)),
        TyRef::Nullable(inner) => {
            let inner_jvm = tyref_to_jvm(inner, package);
            match JvmType::boxed_name(&inner_jvm) {
                Some(boxed) => JvmType::Object(boxed.to_string()),
                None => inner_jvm,
            }
        }
        TyRef::Fn(_, _) => JvmType::Object("java/lang/Object".to_string()),
        TyRef::SelfTy | TyRef::Unresolved(_) | TyRef::Error => {
            JvmType::Object("java/lang/Object".to_string())
        }
    }
}

fn prim_to_jvm(prim: &PrimTy) -> JvmType {
    match prim {
        PrimTy::Int => JvmType::Int,
        PrimTy::Long => JvmType::Long,
        PrimTy::Float => JvmType::Float,
        PrimTy::Double => JvmType::Double,
        PrimTy::Bool => JvmType::Boolean,
        PrimTy::Char => JvmType::Char,
        PrimTy::Byte => JvmType::Byte,
        PrimTy::Short => JvmType::Short,
        PrimTy::String => JvmType::Object("java/lang/String".to_string()),
        PrimTy::Unit => JvmType::Void,
        PrimTy::Nothing => JvmType::Void,
    }
}

/// Returns the JVM type descriptor string (e.g. `I`, `Ljava/lang/String;`).
pub fn jvm_type_descriptor(ty: &JvmType) -> String {
    match ty {
        JvmType::Byte => "B".to_string(),
        JvmType::Short => "S".to_string(),
        JvmType::Int => "I".to_string(),
        JvmType::Long => "J".to_string(),
        JvmType::Float => "F".to_string(),
        JvmType::Double => "D".to_string(),
        JvmType::Char => "C".to_string(),
        JvmType::Boolean => "Z".to_string(),
        JvmType::Void => "V".to_string(),
        JvmType::Object(name) => format!("L{name};"),
        JvmType::Array(elem) => format!("[{}", jvm_type_descriptor(elem)),
    }
}

/// Returns the JVM method descriptor string (e.g. `(IJ)Z`).
pub fn jvm_method_descriptor(params: &[JvmType], ret: &JvmType) -> String {
    let mut desc = String::from("(");
    for p in params {
        desc.push_str(&jvm_type_descriptor(p));
    }
    desc.push(')');
    desc.push_str(&jvm_type_descriptor(ret));
    desc
}

/// Builds a JVM internal class name by prepending the package path (e.g. `com/example/Foo`).
pub fn class_internal_name(name: &str, package: Option<&[SmolStr]>) -> String {
    match package {
        Some(parts) if !parts.is_empty() => {
            let prefix: String = parts
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("/");
            format!("{prefix}/{name}")
        }
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prim_descriptors() {
        assert_eq!(jvm_type_descriptor(&JvmType::Int), "I");
        assert_eq!(jvm_type_descriptor(&JvmType::Long), "J");
        assert_eq!(jvm_type_descriptor(&JvmType::Float), "F");
        assert_eq!(jvm_type_descriptor(&JvmType::Double), "D");
        assert_eq!(jvm_type_descriptor(&JvmType::Boolean), "Z");
        assert_eq!(jvm_type_descriptor(&JvmType::Char), "C");
        assert_eq!(jvm_type_descriptor(&JvmType::Byte), "B");
        assert_eq!(jvm_type_descriptor(&JvmType::Short), "S");
        assert_eq!(jvm_type_descriptor(&JvmType::Void), "V");
    }

    #[test]
    fn object_descriptor() {
        assert_eq!(
            jvm_type_descriptor(&JvmType::Object("java/lang/String".to_string())),
            "Ljava/lang/String;"
        );
    }

    #[test]
    fn array_descriptor() {
        assert_eq!(
            jvm_type_descriptor(&JvmType::Array(Box::new(JvmType::Int))),
            "[I"
        );
        assert_eq!(
            jvm_type_descriptor(&JvmType::Array(Box::new(JvmType::Object(
                "java/lang/Object".to_string()
            )))),
            "[Ljava/lang/Object;"
        );
    }

    #[test]
    fn method_descriptor() {
        assert_eq!(jvm_method_descriptor(&[], &JvmType::Void), "()V");
        assert_eq!(
            jvm_method_descriptor(&[JvmType::Int, JvmType::Long], &JvmType::Boolean),
            "(IJ)Z"
        );
        assert_eq!(
            jvm_method_descriptor(
                &[JvmType::Object("java/lang/String".to_string())],
                &JvmType::Object("java/lang/String".to_string())
            ),
            "(Ljava/lang/String;)Ljava/lang/String;"
        );
    }

    #[test]
    fn class_name_with_package() {
        let pkg: Vec<SmolStr> = vec!["com".into(), "example".into()];
        assert_eq!(class_internal_name("Foo", Some(&pkg)), "com/example/Foo");
    }

    #[test]
    fn class_name_no_package() {
        assert_eq!(class_internal_name("Foo", None), "Foo");
        let empty: Vec<SmolStr> = vec![];
        assert_eq!(class_internal_name("Foo", Some(&empty)), "Foo");
    }

    #[test]
    fn tyref_prim_conversion() {
        assert_eq!(tyref_to_jvm(&TyRef::Prim(PrimTy::Int), None), JvmType::Int);
        assert_eq!(
            tyref_to_jvm(&TyRef::Prim(PrimTy::Bool), None),
            JvmType::Boolean
        );
        assert_eq!(
            tyref_to_jvm(&TyRef::Prim(PrimTy::String), None),
            JvmType::Object("java/lang/String".to_string())
        );
        assert_eq!(
            tyref_to_jvm(&TyRef::Prim(PrimTy::Unit), None),
            JvmType::Void
        );
    }

    #[test]
    fn tyref_named_with_package() {
        let pkg: Vec<SmolStr> = vec!["com".into(), "app".into()];
        assert_eq!(
            tyref_to_jvm(&TyRef::Named("User".into()), Some(&pkg)),
            JvmType::Object("com/app/User".to_string())
        );
    }

    #[test]
    fn tyref_nullable_boxes_primitive() {
        let nullable_int = TyRef::Nullable(Box::new(TyRef::Prim(PrimTy::Int)));
        assert_eq!(
            tyref_to_jvm(&nullable_int, None),
            JvmType::Object("java/lang/Integer".to_string())
        );
    }

    #[test]
    fn tyref_nullable_keeps_reference() {
        let nullable_str = TyRef::Nullable(Box::new(TyRef::Prim(PrimTy::String)));
        assert_eq!(
            tyref_to_jvm(&nullable_str, None),
            JvmType::Object("java/lang/String".to_string())
        );
    }

    #[test]
    fn tyref_generic_erased() {
        let list_int = TyRef::Generic("List".into(), vec![TyRef::Prim(PrimTy::Int)]);
        assert_eq!(
            tyref_to_jvm(&list_int, None),
            JvmType::Object("List".to_string())
        );
    }
}
