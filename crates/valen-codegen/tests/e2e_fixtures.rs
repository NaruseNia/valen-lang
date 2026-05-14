use ristretto_classfile::attributes::{Attribute, Instruction};
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

fn compile_fixture_outputs(name: &str) -> Vec<valen_codegen::ClassFileOutput> {
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

    let tc = valen_hir::ty::type_check(&resolve_result.hir, &parse_result.items);

    let outputs =
        valen_codegen::compile_hir(&resolve_result.hir, &tc.bodies).expect("codegen failed");
    assert!(!outputs.is_empty(), "no class files generated for {name}");

    for o in &outputs {
        assert_eq!(&o.bytes[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    }
    outputs
}

fn compile_fixture(name: &str) -> Vec<ClassFile<'static>> {
    compile_fixture_outputs(name)
        .iter()
        .map(|o| ClassFile::from_bytes(&o.bytes).expect("ClassFile::from_bytes failed"))
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

#[test]
fn fixture_enum_simple() {
    let classes = compile_fixture("enum_simple.vln");
    // Shape (sealed iface) + Circle (record) + Rect (record) + Point (singleton)
    assert_eq!(classes.len(), 4);

    let iface = &classes[0];
    assert_eq!(iface.class_name().unwrap(), "com/example/Shape");
    assert!(iface
        .access_flags
        .contains(ClassAccessFlags::INTERFACE | ClassAccessFlags::ABSTRACT));

    let circle = &classes[1];
    assert_eq!(circle.class_name().unwrap(), "com/example/Shape$Circle");
    assert!(circle.access_flags.contains(ClassAccessFlags::FINAL));
    assert_eq!(circle.fields.len(), 1);

    let rect = &classes[2];
    assert_eq!(rect.class_name().unwrap(), "com/example/Shape$Rect");
    assert_eq!(rect.fields.len(), 2);

    let point = &classes[3];
    assert_eq!(point.class_name().unwrap(), "com/example/Shape$Point");
    assert!(point.fields[0]
        .access_flags
        .contains(FieldAccessFlags::STATIC));
}

#[test]
fn fixture_enum_unit_only() {
    let classes = compile_fixture("enum_unit_only.vln");
    assert_eq!(classes.len(), 4); // iface + Red + Green + Blue

    for c in &classes[1..] {
        assert!(c.access_flags.contains(ClassAccessFlags::FINAL));
        assert_eq!(c.fields.len(), 1); // INSTANCE
        assert_eq!(c.methods.len(), 2); // <init> + <clinit>
    }
}

#[test]
fn fixture_fn_simple() {
    let classes = compile_fixture("fn_simple.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Math");
    // <init> + add + negate
    assert_eq!(c.methods.len(), 3);
}

#[test]
fn fixture_fn_if_else() {
    let classes = compile_fixture("fn_if_else.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Logic");
    // <init> + max
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_fn_loop() {
    let outputs = compile_fixture_outputs("fn_while_loop.vln");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].internal_name, "com/example/Loops");
}

#[test]
fn fixture_fn_match() {
    let classes = compile_fixture("fn_match.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Matcher");
    // <init> + describe
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_fn_string_interp() {
    let classes = compile_fixture("fn_string_interp.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Greeter");
    // <init> + greet
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_java_import() {
    let outputs = compile_fixture_outputs("java_import.vln");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].internal_name, "com/example/Importer");

    let c = ClassFile::from_bytes(&outputs[0].bytes).expect("parse classfile");
    assert_eq!(c.class_name().unwrap(), "com/example/Importer");
    // <init> + create_list + create_file
    assert_eq!(c.methods.len(), 3);

    // Verify the constant pool contains the correct JVM internal names
    // for imported types (not package-prefixed local names)
    let cp_strings: Vec<String> = (1..c.constant_pool.len())
        .filter_map(|i| c.constant_pool.try_get_utf8(i as u16).ok())
        .map(|s| s.to_string())
        .collect();

    assert!(
        cp_strings.iter().any(|s| s.contains("java/util/ArrayList")),
        "constant pool should contain java/util/ArrayList, got: {:?}",
        cp_strings
    );
    assert!(
        cp_strings.iter().any(|s| s.contains("java/io/File")),
        "constant pool should contain java/io/File, got: {:?}",
        cp_strings
    );
}

#[test]
fn fixture_safe_block() {
    let outputs = compile_fixture_outputs("safe_block.vln");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].internal_name, "com/example/SafeDemo");

    let c = ClassFile::from_bytes(&outputs[0].bytes).expect("parse classfile");
    assert_eq!(c.class_name().unwrap(), "com/example/SafeDemo");
    // <init> + safe_call + safe_string
    assert_eq!(c.methods.len(), 3);
}

#[test]
fn fixture_trait_impl() {
    let outputs = compile_fixture_outputs("trait_impl.vln");
    // At minimum Dog class should be generated
    let dog_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Dog")
        .expect("Dog class should be generated");

    let c = ClassFile::from_bytes(&dog_output.bytes).expect("parse Dog classfile");
    assert_eq!(c.class_name().unwrap(), "com/example/Dog");
    // Currently only <init>; trait impl methods not yet emitted (codegen TODO)
    assert!(
        !c.methods.is_empty(),
        "Dog should have at least <init> method"
    );
}

#[test]
fn fixture_sealed_class() {
    let outputs = compile_fixture_outputs("sealed_class.vln");
    // Animal + Cat + Fish = 3 classes
    assert_eq!(outputs.len(), 3);

    let animal_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Animal")
        .expect("Animal class should be generated");
    let animal = ClassFile::from_bytes(&animal_output.bytes).expect("parse Animal classfile");
    assert_eq!(animal.class_name().unwrap(), "com/example/Animal");

    // Animal should have PermittedSubclasses attribute
    let has_permitted = animal
        .attributes
        .iter()
        .any(|a| matches!(a, Attribute::PermittedSubclasses { .. }));
    assert!(
        has_permitted,
        "sealed class Animal should have PermittedSubclasses attribute"
    );
}

#[test]
fn fixture_fn_for_loop() {
    let classes = compile_fixture("fn_for_loop.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Loops");
    // <init> + count
    assert_eq!(c.methods.len(), 2);

    // Verify iinc instruction is present in the count method
    let count_method = &c.methods[1];
    let has_iinc = count_method.attributes.iter().any(|attr| {
        if let Attribute::Code { code, .. } = attr {
            code.iter()
                .any(|i| matches!(i, Instruction::Iinc(_, _) | Instruction::Iinc_w(_, _)))
        } else {
            false
        }
    });
    assert!(has_iinc, "for-range loop should emit iinc instruction");
}

#[test]
fn fixture_fn_for_range_inclusive() {
    let classes = compile_fixture("fn_for_range_inclusive.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/RangeTest");
    // <init> + sum_inclusive
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_fn_for_break_continue() {
    let classes = compile_fixture("fn_for_break_continue.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/BreakCont");
    // <init> + find_first_even
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_fn_lambda() {
    let classes = compile_fixture("fn_lambda.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Funcs");
    // <init> + apply + lambda$0 (synthetic)
    assert!(
        c.methods.len() >= 3,
        "expected at least 3 methods (init, apply, lambda$0), got {}",
        c.methods.len()
    );
    // Verify the synthetic lambda method exists.
    let has_lambda = c.methods.iter().any(|m| {
        c.constant_pool
            .try_get_utf8(m.name_index)
            .ok()
            .and_then(|n| n.as_str())
            .map(|s| s.starts_with("lambda$"))
            .unwrap_or(false)
    });
    assert!(has_lambda, "expected a synthetic lambda$ method");
    // Verify BootstrapMethods attribute is present.
    let has_bsm = c
        .attributes
        .iter()
        .any(|a| matches!(a, Attribute::BootstrapMethods { .. }));
    assert!(has_bsm, "expected BootstrapMethods attribute for lambda");
}

#[test]
fn fixture_fn_range_standalone() {
    let classes = compile_fixture("fn_range_standalone.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Ranges");
    // <init> + make_range + make_inclusive
    assert_eq!(c.methods.len(), 3);
}

#[test]
fn fixture_typealias() {
    let classes = compile_fixture("typealias.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/AliasDemo");
    // <init> + make_list
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_fn_nested_control() {
    let classes = compile_fixture("fn_nested_control.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Nested");
    // <init> + classify
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_sealed_trait() {
    let outputs = compile_fixture_outputs("sealed_trait.vln");
    // Expr (sealed interface) + Lit + Add
    assert_eq!(outputs.len(), 3);

    let expr_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Expr")
        .expect("Expr interface should be generated");
    let expr = ClassFile::from_bytes(&expr_output.bytes).expect("parse Expr classfile");
    assert!(
        expr.access_flags.contains(ClassAccessFlags::INTERFACE),
        "sealed trait should emit as interface"
    );
    assert!(
        expr.access_flags.contains(ClassAccessFlags::ABSTRACT),
        "sealed trait should be abstract"
    );
    let has_permitted = expr.attributes.iter().any(|a| {
        matches!(a, Attribute::PermittedSubclasses { class_indexes, .. } if class_indexes.len() == 2)
    });
    assert!(
        has_permitted,
        "sealed trait should have PermittedSubclasses with 2 entries"
    );

    let lit_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Lit")
        .expect("Lit class should be generated");
    let lit = ClassFile::from_bytes(&lit_output.bytes).expect("parse Lit classfile");
    assert!(
        !lit.interfaces.is_empty(),
        "Lit should implement at least one interface (Expr)"
    );
}

#[test]
fn fixture_annotation() {
    let outputs = compile_fixture_outputs("annotation.vln");
    // Deprecated (@interface) + Serializable (@interface) + OldApi (class)
    assert_eq!(outputs.len(), 3);

    let deprecated_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Deprecated")
        .expect("Deprecated annotation class should be generated");
    let deprecated =
        ClassFile::from_bytes(&deprecated_output.bytes).expect("parse Deprecated classfile");
    assert!(
        deprecated
            .access_flags
            .contains(ClassAccessFlags::ANNOTATION),
        "annotation class should have ACC_ANNOTATION"
    );
    assert!(
        deprecated
            .access_flags
            .contains(ClassAccessFlags::INTERFACE),
        "annotation class should have ACC_INTERFACE"
    );
    assert!(
        deprecated.access_flags.contains(ClassAccessFlags::ABSTRACT),
        "annotation class should have ACC_ABSTRACT"
    );

    // Should have RuntimeVisibleAnnotations (@Retention + @Target)
    let has_rva = deprecated
        .attributes
        .iter()
        .any(|a| matches!(a, Attribute::RuntimeVisibleAnnotations { .. }));
    assert!(
        has_rva,
        "annotation class should have RuntimeVisibleAnnotations"
    );

    // Should have 1 abstract method: message()
    // Should have 1 abstract method: message()
    assert_eq!(
        deprecated.methods.len(),
        1,
        "Deprecated should have 1 method (message)"
    );

    // Serializable should also be an annotation
    let serializable_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Serializable")
        .expect("Serializable annotation class should be generated");
    let serializable =
        ClassFile::from_bytes(&serializable_output.bytes).expect("parse Serializable classfile");
    assert!(
        serializable
            .access_flags
            .contains(ClassAccessFlags::ANNOTATION),
        "marker annotation class should have ACC_ANNOTATION"
    );
}

#[test]
fn fixture_default_args() {
    let classes = compile_fixture("default_args.vln");
    // Greeter class
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Greeter");
    // <init> + greet + repeat (top-level fn goes into Greeter since it's the only class)
    assert!(
        c.methods.len() >= 2,
        "Greeter should have at least <init> and greet, got {}",
        c.methods.len()
    );
}

#[test]
fn fixture_generics() {
    let outputs = compile_fixture_outputs("generics.vln");
    // Box + Pair + Wrapper = 3 classes (top-level fns go into one of the classes)
    assert!(
        outputs.len() >= 3,
        "expected at least 3 classes (Box, Pair, Wrapper), got {}",
        outputs.len()
    );

    let box_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Box")
        .expect("Box class should be generated");
    let box_class = ClassFile::from_bytes(&box_output.bytes).expect("parse Box classfile");
    assert!(
        box_class.methods.len() >= 2,
        "Box should have at least <init> and get, got {}",
        box_class.methods.len()
    );

    let pair_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Pair")
        .expect("Pair class should be generated");
    let pair_class = ClassFile::from_bytes(&pair_output.bytes).expect("parse Pair classfile");
    // <init> + getLeft + getRight
    assert!(
        pair_class.methods.len() >= 3,
        "Pair should have at least <init>, getLeft, getRight, got {}",
        pair_class.methods.len()
    );
    // Pair has 2 fields (left, right)
    assert_eq!(pair_class.fields.len(), 2);

    let wrapper_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Wrapper")
        .expect("Wrapper data class should be generated");
    let wrapper_class =
        ClassFile::from_bytes(&wrapper_output.bytes).expect("parse Wrapper classfile");
    // data class: <init> + equals + hashCode + toString + copy
    assert_eq!(wrapper_class.methods.len(), 5);
    assert_eq!(wrapper_class.fields.len(), 1);
}

#[test]
fn fixture_generics_bounds() {
    let outputs = compile_fixture_outputs("generics_bounds.vln");
    // Dog class (with Show impl methods)
    assert!(
        !outputs.is_empty(),
        "should generate at least 1 class for generics_bounds"
    );

    let dog_output = outputs
        .iter()
        .find(|o| o.internal_name == "com/example/Dog")
        .expect("Dog class should be generated");
    let dog = ClassFile::from_bytes(&dog_output.bytes).expect("parse Dog classfile");
    // <init> + show (from impl) + display + describe (top-level fns)
    assert!(
        dog.methods.len() >= 2,
        "Dog should have at least <init> and show, got {}",
        dog.methods.len()
    );
}

#[test]
fn fixture_fn_assignment() {
    let classes = compile_fixture("fn_assignment.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Mutate");
    // <init> + accumulate
    assert_eq!(c.methods.len(), 2);
}
