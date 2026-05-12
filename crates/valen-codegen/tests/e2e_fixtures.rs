use ristretto_classfile::{ClassAccessFlags, ClassFile, FieldAccessFlags};
use valen_ast::FileId;

fn fixture_path(name: &str) -> std::path::PathBuf {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/codegen")
        .join(name)
}

fn compile_fixture(name: &str) -> Vec<ClassFile<'static>> {
    let path = fixture_path(name);
    let source = std::fs::read_to_string(&path).expect("read fixture");
    let file_id = FileId(0);
    let parse_result = valen_parser::parse(&source, file_id);
    assert!(
        !parse_result.diagnostics.has_errors(),
        "parse errors in {name}: {:?}",
        parse_result
            .diagnostics
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );

    let resolve_result = valen_hir::resolve::resolve(&parse_result.items);
    assert!(
        !resolve_result.diagnostics.has_errors(),
        "resolve errors in {name}"
    );

    let outputs = valen_codegen::compile_hir(&resolve_result.hir).expect("codegen failed");
    assert!(!outputs.is_empty(), "no class files generated for {name}");

    outputs
        .iter()
        .map(|o| {
            assert_eq!(&o.bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
            ClassFile::from_bytes(&o.bytes).expect("ClassFile::from_bytes failed")
        })
        .collect()
}

#[test]
fn fixture_empty_class() {
    let classes = compile_fixture("empty_class.vln");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].class_name().unwrap(), "com/example/Foo");
    assert!(classes[0].access_flags.contains(ClassAccessFlags::PUBLIC));
    assert!(classes[0].access_flags.contains(ClassAccessFlags::FINAL));
    assert_eq!(classes[0].methods.len(), 1); // <init>
}

#[test]
fn fixture_class_with_fields() {
    let classes = compile_fixture("class_with_fields.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/User");
    assert_eq!(c.fields.len(), 2);

    // pub name: String → public final
    assert!(c.fields[0].access_flags.contains(FieldAccessFlags::PUBLIC));
    assert!(c.fields[0].access_flags.contains(FieldAccessFlags::FINAL));

    // pub mut age: Int → public, not final
    assert!(c.fields[1].access_flags.contains(FieldAccessFlags::PUBLIC));
    assert!(!c.fields[1].access_flags.contains(FieldAccessFlags::FINAL));

    // <init> + greet (stub)
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_data_class() {
    let classes = compile_fixture("data_class.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Point");
    assert!(c.access_flags.contains(ClassAccessFlags::FINAL));
    assert_eq!(c.fields.len(), 2);
    // <init> + equals + hashCode + toString + copy
    assert_eq!(c.methods.len(), 5);
}

#[test]
fn fixture_data_class_mixed_types() {
    let classes = compile_fixture("data_class_mixed.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Record");
    assert_eq!(c.fields.len(), 4);
    assert_eq!(c.methods.len(), 5);
}

#[test]
fn fixture_abstract_class() {
    let classes = compile_fixture("abstract_class.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Shape");
    assert!(c.access_flags.contains(ClassAccessFlags::ABSTRACT));
    assert!(!c.access_flags.contains(ClassAccessFlags::FINAL));
    assert!(!c.access_flags.contains(ClassAccessFlags::PUBLIC)); // no pub keyword
    assert_eq!(c.methods.len(), 1); // <init> only
}

#[test]
fn fixture_open_class() {
    let classes = compile_fixture("open_class.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Animal");
    assert!(!c.access_flags.contains(ClassAccessFlags::ABSTRACT));
    assert!(!c.access_flags.contains(ClassAccessFlags::FINAL));
    assert_eq!(c.fields.len(), 1);
    // <init> + speak (stub)
    assert_eq!(c.methods.len(), 2);
}
