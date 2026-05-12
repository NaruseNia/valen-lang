pub mod class_emit;
pub mod data_class_methods;
pub mod descriptor;
pub mod emit;
pub mod enum_emit;
pub mod jvm_ir;
pub mod lower;
#[cfg(test)]
mod test_fixtures;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JvmVersion {
    Java21,
    Java25,
}

pub struct ClassFileOutput {
    pub internal_name: String,
    pub bytes: Vec<u8>,
}

pub fn compile_hir(hir: &valen_hir::Hir) -> Result<Vec<ClassFileOutput>, emit::CodegenError> {
    let jvm_classes = lower::lower_hir(hir);
    jvm_classes.iter().map(emit::emit_class).collect()
}

#[cfg(test)]
mod integration_tests {
    use ristretto_classfile::{ClassAccessFlags, FieldAccessFlags};
    use valen_hir::*;

    use crate::test_fixtures::*;

    #[test]
    fn e2e_empty_final_class() {
        let hir = empty_class("Foo", ClassDefKind::Final);
        let classes = compile_and_verify(&hir);
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].class_name().unwrap(), "Foo");
        assert!(classes[0].access_flags.contains(ClassAccessFlags::FINAL));
        assert_eq!(classes[0].methods.len(), 1);
    }

    #[test]
    fn e2e_class_with_fields_and_ctor() {
        let hir = class_with_params(
            "User",
            ClassDefKind::Final,
            vec![
                ctor_param("name", TyRef::Prim(PrimTy::String), Vis::Pub, false),
                ctor_param("age", TyRef::Prim(PrimTy::Int), Vis::Private, true),
            ],
            Vis::Pub,
        );
        let classes = compile_and_verify(&hir);
        let c = &classes[0];
        assert_eq!(c.fields.len(), 2);
        assert!(c.fields[0].access_flags.contains(FieldAccessFlags::PUBLIC));
        assert!(c.fields[0].access_flags.contains(FieldAccessFlags::FINAL));
        assert!(c.fields[1].access_flags.contains(FieldAccessFlags::PRIVATE));
        assert!(!c.fields[1].access_flags.contains(FieldAccessFlags::FINAL));
    }

    #[test]
    fn e2e_data_class_generates_five_methods() {
        let hir = data_class(
            "Point",
            vec![
                ctor_param("x", TyRef::Prim(PrimTy::Float), Vis::Pub, false),
                ctor_param("y", TyRef::Prim(PrimTy::Float), Vis::Pub, false),
            ],
        );
        let classes = compile_and_verify(&hir);
        let c = &classes[0];
        assert_eq!(c.class_name().unwrap(), "Point");
        assert!(c.access_flags.contains(ClassAccessFlags::FINAL));
        assert_eq!(c.methods.len(), 5);
    }

    #[test]
    fn e2e_data_class_int_fields() {
        let hir = data_class(
            "Vec2",
            vec![
                ctor_param("x", TyRef::Prim(PrimTy::Int), Vis::Pub, false),
                ctor_param("y", TyRef::Prim(PrimTy::Int), Vis::Pub, false),
            ],
        );
        let classes = compile_and_verify(&hir);
        assert_eq!(classes[0].methods.len(), 5);
    }

    #[test]
    fn e2e_data_class_mixed_types() {
        let hir = data_class(
            "Record",
            vec![
                ctor_param("id", TyRef::Prim(PrimTy::Long), Vis::Pub, false),
                ctor_param("name", TyRef::Prim(PrimTy::String), Vis::Pub, false),
                ctor_param("active", TyRef::Prim(PrimTy::Bool), Vis::Pub, false),
                ctor_param("score", TyRef::Prim(PrimTy::Double), Vis::Pub, false),
            ],
        );
        let classes = compile_and_verify(&hir);
        assert_eq!(classes[0].fields.len(), 4);
        assert_eq!(classes[0].methods.len(), 5);
    }

    #[test]
    fn e2e_class_with_stub_method() {
        let hir = class_with_method(
            "Greeter",
            "greet",
            vec![self_param()],
            Some(TyRef::Prim(PrimTy::String)),
            true,
        );
        let classes = compile_and_verify(&hir);
        assert_eq!(classes[0].methods.len(), 2); // <init> + greet
    }

    #[test]
    fn e2e_abstract_class() {
        let hir = empty_class("Shape", ClassDefKind::Abstract);
        let classes = compile_and_verify(&hir);
        assert!(classes[0].access_flags.contains(ClassAccessFlags::ABSTRACT));
        assert!(!classes[0].access_flags.contains(ClassAccessFlags::FINAL));
    }

    #[test]
    fn e2e_class_with_package() {
        let mut hir = empty_class("App", ClassDefKind::Final);
        hir.package = Some(vec!["com".into(), "example".into()]);
        let classes = compile_and_verify(&hir);
        assert_eq!(classes[0].class_name().unwrap(), "com/example/App");
    }
}
