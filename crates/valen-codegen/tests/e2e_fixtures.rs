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
    compile_fixture_impl(name, true)
        .into_iter()
        .filter(|o| o.internal_name != "valen/core/ListIterator")
        .collect()
}

fn compile_fixture_outputs_skip_typecheck(name: &str) -> Vec<valen_codegen::ClassFileOutput> {
    compile_fixture_impl(name, false)
        .into_iter()
        .filter(|o| o.internal_name != "valen/core/ListIterator")
        .collect()
}

fn compile_fixture_impl(name: &str, assert_typecheck: bool) -> Vec<valen_codegen::ClassFileOutput> {
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
    if assert_typecheck {
        let errors: Vec<_> = tc
            .diagnostics
            .iter()
            .filter(|d| d.severity == valen_diagnostics::Severity::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "type errors in {name}: {:?}",
            errors.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

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
        .filter(|o| o.internal_name != "valen/core/ListIterator")
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
    // Skip type_check assertion: foreign types require classpath scanning which
    // is not available in the test harness (resolve::resolve vs resolve_with_classpath).
    let outputs = compile_fixture_outputs_skip_typecheck("java_import.vln");
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].internal_name, "com/example/Importer");

    let c = ClassFile::from_bytes(&outputs[0].bytes).expect("parse classfile");
    assert_eq!(c.class_name().unwrap(), "com/example/Importer");
    // <init> + createList + createFile
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
    // <init> + safeCall + safeString
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
    // <init> + sumInclusive
    assert_eq!(c.methods.len(), 2);
}

#[test]
fn fixture_fn_for_break_continue() {
    let classes = compile_fixture("fn_for_break_continue.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/BreakCont");
    // <init> + findFirstEven
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
    // <init> + makeRange + makeInclusive
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

#[test]
fn fixture_try_operator() {
    let classes = compile_fixture("try_operator.vln");
    assert!(!classes.is_empty());

    let main_class = classes
        .iter()
        .find(|c| c.class_name().unwrap() == "test/TryOperator")
        .expect("TryOperator class");
    let methods: Vec<String> = main_class
        .methods
        .iter()
        .filter_map(|m| {
            main_class
                .constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .collect();
    assert!(
        methods.contains(&"findValue".to_string()),
        "missing findValue"
    );
    assert!(
        methods.contains(&"useOption".to_string()),
        "missing useOption"
    );
    assert!(methods.contains(&"divide".to_string()), "missing divide");
    assert!(methods.contains(&"compute".to_string()), "missing compute");
}

#[test]
fn fixture_operator_overload() {
    let classes = compile_fixture("operator_overload.vln");
    assert!(
        classes.len() >= 2,
        "expected at least Vec2 + Calculator classes"
    );

    let calc = classes
        .iter()
        .find(|c| {
            c.class_name()
                .ok()
                .and_then(|n| n.as_str().map(|s| s.contains("Calculator")))
                .unwrap_or(false)
        })
        .expect("Calculator class not found");

    let methods: Vec<String> = calc
        .methods
        .iter()
        .filter_map(|m| {
            calc.constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .collect();
    assert!(
        methods.contains(&"addVectors".to_string()),
        "missing addVectors"
    );
    assert!(
        methods.contains(&"subVectors".to_string()),
        "missing subVectors"
    );
}

#[test]
fn fixture_enum_destructure_bind() {
    let classes = compile_fixture("enum_destructure_bind.vln");
    assert!(
        classes.len() >= 2,
        "expected Color enum + Matcher class, got {}",
        classes.len()
    );

    let matcher = classes
        .iter()
        .find(|c| {
            c.class_name()
                .ok()
                .and_then(|n| n.as_str().map(|s| s.contains("Matcher")))
                .unwrap_or(false)
        })
        .expect("Matcher class not found");

    let methods: Vec<String> = matcher
        .methods
        .iter()
        .filter_map(|m| {
            matcher
                .constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .collect();
    assert!(
        methods.contains(&"describeColor".to_string()),
        "missing describeColor method"
    );
}

#[test]
fn fixture_println_print() {
    let classes = compile_fixture("println_print.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/PrintDemo");

    let cp_strings: Vec<String> = (1..c.constant_pool.len())
        .filter_map(|i| c.constant_pool.try_get_utf8(i as u16).ok())
        .map(|s| s.to_string())
        .collect();
    assert!(
        cp_strings.iter().any(|s| s == "java/io/PrintStream"),
        "should reference PrintStream"
    );
}

#[test]
fn fixture_if_let() {
    let classes = compile_fixture("if_let.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];

    let methods: Vec<String> = c
        .methods
        .iter()
        .filter_map(|m| {
            c.constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .collect();
    assert!(
        methods.contains(&"unwrapOr".to_string()),
        "missing unwrapOr method"
    );
}

#[test]
fn fixture_let_else() {
    let classes = compile_fixture("let_else.vln");
    // Color (abstract), Color$Red, Color$Green, Color$Blue, LetElseTest
    assert!(
        classes.len() >= 2,
        "should generate Color enum + LetElseTest classes"
    );
    let test_class = classes
        .iter()
        .find(|c| c.class_name().unwrap() == "com/example/LetElseTest")
        .expect("LetElseTest class should be generated");
    let method = test_class
        .methods
        .iter()
        .find(|m| {
            test_class
                .constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str())
                .map(|s| s == "extractBlue")
                .unwrap_or(false)
        })
        .expect("extractBlue method should exist");
    // Verify the method has code (was compiled, not a stub)
    let has_code = method
        .attributes
        .iter()
        .any(|a| matches!(a, Attribute::Code { .. }));
    assert!(has_code, "extractBlue should have a Code attribute");
}

#[test]
fn fixture_derive() {
    let classes = compile_fixture("derive.vln");
    assert!(
        classes.len() >= 4,
        "should generate Shape enum + variant classes + Entity"
    );

    let circle = classes
        .iter()
        .find(|c| {
            c.class_name()
                .map(|n| {
                    n.as_str()
                        .map(|s| s.ends_with("Shape$Circle"))
                        .unwrap_or(false)
                })
                .unwrap_or(false)
        })
        .expect("Shape$Circle variant class should be generated");

    let circle_methods: Vec<String> = circle
        .methods
        .iter()
        .filter_map(|m| {
            circle
                .constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .collect();

    assert!(
        circle_methods.contains(&"equals".to_string()),
        "Circle should have derived equals: {circle_methods:?}"
    );
    assert!(
        circle_methods.contains(&"hashCode".to_string()),
        "Circle should have derived hashCode: {circle_methods:?}"
    );
    assert!(
        circle_methods.contains(&"toString".to_string()),
        "Circle should have derived toString: {circle_methods:?}"
    );
}

#[test]
fn fixture_variant_shorthand() {
    let classes = compile_fixture("variant_shorthand.vln");
    assert!(
        classes.len() >= 2,
        "should generate Color enum + VariantShorthandTest classes"
    );
    let test_class = classes
        .iter()
        .find(|c| c.class_name().unwrap() == "com/example/VariantShorthandTest")
        .expect("VariantShorthandTest class should be generated");
    let methods: Vec<String> = test_class
        .methods
        .iter()
        .filter_map(|m| {
            test_class
                .constant_pool
                .try_get_utf8(m.name_index)
                .ok()
                .and_then(|n| n.as_str().map(|s| s.to_string()))
        })
        .collect();
    assert!(
        methods.contains(&"makeRed".to_string()),
        "missing makeRed method"
    );
    assert!(
        methods.contains(&"makeBlue".to_string()),
        "missing makeBlue method"
    );
    assert!(
        methods.contains(&"describe".to_string()),
        "missing describe method"
    );
    assert!(
        methods.contains(&"isRed".to_string()),
        "missing isRed method"
    );
}

#[test]
fn fixture_iterator_collection_ops() {
    let classes = compile_fixture("iterator_collection_ops.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/IterOps");
    // <init> + 8 iterator methods + synthetic lambda methods
    assert!(c.methods.len() >= 9);
}

#[test]
fn fixture_collection_literals() {
    let classes = compile_fixture("collection_literals.vln");
    assert_eq!(classes.len(), 1);
    let c = &classes[0];
    assert_eq!(c.class_name().unwrap(), "com/example/Collections");
    // <init> + makeList + emptyList + makeMap + emptyMap + singleElement
    assert_eq!(c.methods.len(), 6);
}
