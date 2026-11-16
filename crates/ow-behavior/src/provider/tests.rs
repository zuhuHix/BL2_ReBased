//! The provider decode on invented packages (`crate::kernel::fixtures`): the cases of `tests/behavior_test.py`
//! sections 1 and 3, and the structural oracles.

use super::*;
use crate::kernel::fixtures::{Env, Seq, Spec, Target, Word, beh, ev, toy_provider};

fn decoded(spec: &Spec) -> (Env, ProviderData) {
    let mut env = Env::of(spec);
    let definition = env.definition("TestBpd", spec.name);
    let data = decode(&mut env.vm, definition).expect("decode");
    (env, data)
}

fn refused(spec: &Spec) -> String {
    let mut env = Env::of(spec);
    let definition = env.definition("TestBpd", spec.name);
    decode(&mut env.vm, definition).unwrap_err().to_string()
}

#[test]
fn the_value_block_decodes_exactly_and_enum_bytes_read_by_name() {
    let (_env, p) = decoded(&toy_provider(
        "exact",
        "BVAR_InstanceData",
        "BVARLINK_Input",
    ));
    assert!(p.values_decoded, "{:?}", p.diagnostics);
    assert!(p.diagnostics.is_empty());
    assert_eq!(p.path, "ToyProvider");
    let main = &p.sequences[0];
    let got: Vec<(&str, &str, &str)> = main
        .variables
        .iter()
        .map(|v| (v.var_type.as_str(), v.name.as_str(), v.object.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("BVAR_Object", "None", ""),
            ("BVAR_Object", "None", "Things.DamageType.ToyFire"),
            ("BVAR_Int", "Count", ""),
            ("BVAR_NamedVariable", "Ref", ""),
            ("BVAR_Float", "None", ""),
        ]
    );
    assert_eq!(main.variables[2].word, 7);
    assert_eq!(main.variables[4].word, 1.5f32.to_bits() as i32);
    // The object word is the package-relative import reference.
    assert!(main.variables[1].word < 0);
    let extra = &p.sequences[1];
    assert!(!extra.enabled_on_spawn);
    assert_eq!(
        extra
            .variables
            .iter()
            .map(|v| v.var_type.as_str())
            .collect::<Vec<_>>(),
        ["BVAR_InstanceData", "BVAR_Object"]
    );
    // InstanceData keeps its first word (0) only.
    assert_eq!(extra.variables[0].word, 0);
    let cmp = &main.behaviors[0];
    assert_eq!(cmp.class, "WillowGame.Behavior_CompareObject");
    assert_eq!(cmp.name, "Cmp");
    assert_eq!(
        cmp.variables,
        vec![
            VariableLink {
                property: "ObjectA".into(),
                link_type: "BVARLINK_Input".into(),
                connection: 0,
                variables: vec![0],
            },
            VariableLink {
                property: "ObjectB".into(),
                link_type: "BVARLINK_Input".into(),
                connection: 0,
                variables: vec![1],
            },
        ]
    );
    assert_eq!(main.events[0].variables[0].link_type, "BVARLINK_Output");
}

#[test]
fn links_unpack_with_a_signed_id_byte_and_ranges() {
    let (_env, p) = decoded(&toy_provider(
        "exact",
        "BVAR_InstanceData",
        "BVARLINK_Input",
    ));
    let main = &p.sequences[0];
    assert_eq!(
        main.links
            .iter()
            .map(|l| (l.behavior, l.id))
            .collect::<Vec<_>>(),
        [(0, -1), (1, 0), (2, 1)]
    );
    assert_eq!((main.events[0].start, main.events[0].length), (0, 1));
    assert_eq!((main.behaviors[0].start, main.behaviors[0].length), (1, 2));
    // Behavior_CompareObject tags no Context here: the struct's zero (false); a class with no Context: true.
    assert!(!main.behaviors[0].default_output);
    assert!(main.behaviors[1].default_output);
}

#[test]
fn a_short_or_long_block_or_an_unknown_type_trusts_no_value() {
    for (variant, message) in [
        (
            "short",
            "variable value block shorter than its variables in ToyProvider",
        ),
        ("long", "variable value block size mismatch in ToyProvider"),
    ] {
        let (_env, p) = decoded(&toy_provider(
            variant,
            "BVAR_InstanceData",
            "BVARLINK_Input",
        ));
        assert!(!p.values_decoded);
        assert_eq!(p.diagnostics, [message]);
        // No partial value is kept.
        assert_eq!(p.sequences[0].variables[2].word, 0);
        assert_eq!(p.sequences[0].variables[1].object, "");
    }
    let (_env, p) = decoded(&toy_provider("exact", "BVAR_Mystery", "BVARLINK_Input"));
    assert!(!p.values_decoded);
    assert_eq!(
        p.diagnostics,
        ["unknown variable type BVAR_Mystery in ToyProvider"]
    );
}

#[test]
fn an_object_word_outside_the_tables_trusts_no_value() {
    let seq = Seq {
        name: "S",
        variables: vec![("None", "BVAR_Object")],
        ..Seq::default()
    };
    let (_env, p) = decoded(&Spec::new("P", vec![seq], vec![Word::Obj(Target::Bad)]));
    assert!(!p.values_decoded);
    assert_eq!(
        p.diagnostics,
        ["object variable reference out of range in P"]
    );
}

#[test]
fn the_value_sizes_are_the_fitted_table() {
    // The C++ table, entry by entry (FITTED, BEHAVIOR_DATA_DECODE.md "Layout").
    assert_eq!(value_size("BVAR_Bool"), Some(4));
    assert_eq!(value_size("BVAR_Vector"), Some(12));
    assert_eq!(value_size("BVAR_DirectionVector"), Some(48));
    assert_eq!(value_size("BVAR_InstanceData"), Some(12));
    assert_eq!(value_size("BVAR_Attribute"), Some(20));
    assert_eq!(value_size("BVAR_UnaryMath"), Some(8));
    assert_eq!(value_size("BVAR_BinaryMath"), Some(12));
    assert_eq!(value_size("BVAR_AttachmentLocation"), Some(32));
    assert_eq!(value_size("BVAR_Flag"), Some(8));
    for zero in [
        "BVAR_NamedVariable",
        "BVAR_NamedKismetVariable",
        "BVAR_AllPlayers",
    ] {
        assert_eq!(value_size(zero), Some(0));
    }
    // Exact names only: the enum's spelling.
    assert_eq!(value_size("bvar_bool"), None);
    assert_eq!(value_size("BVAR_Mystery"), None);
}

#[test]
fn a_variable_link_outside_its_array_is_a_diagnostic_not_an_error() {
    let mut b = beh("B", "Behavior_ToySame", (0, 0));
    b.vars = (0, 1);
    let seq = Seq {
        name: "S",
        behaviors: vec![b],
        variables: vec![("None", "BVAR_Int")],
        vlinks: vec![crate::kernel::fixtures::VLink(
            "Count",
            "BVARLINK_Input",
            0,
            (0, 2),
        )],
        linked: vec![0],
        ..Seq::default()
    };
    let (_env, p) = decoded(&Spec::new("P", vec![seq.clone()], vec![Word::I(3)]));
    assert!(!p.values_decoded);
    assert_eq!(
        p.diagnostics,
        ["variable link range outside its array in P"]
    );

    // A variable index outside the sequence's variables.
    let mut bad = seq.clone();
    bad.vlinks = vec![crate::kernel::fixtures::VLink(
        "Count",
        "BVARLINK_Input",
        0,
        (0, 1),
    )];
    bad.linked = vec![5];
    let (_env, p) = decoded(&Spec::new("P", vec![bad], vec![Word::I(3)]));
    assert!(!p.values_decoded);
    assert!(
        p.sequences[0].behaviors[0].variables[0]
            .variables
            .is_empty()
    );

    // A behavior's packed range outside the variable-link array.
    let mut range = seq;
    range.behaviors[0].vars = (1, 1);
    range.vlinks = vec![crate::kernel::fixtures::VLink(
        "Count",
        "BVARLINK_Input",
        0,
        (0, 1),
    )];
    let (_env, p) = decoded(&Spec::new("P", vec![range], vec![Word::I(3)]));
    assert!(!p.values_decoded);
    assert!(p.sequences[0].behaviors[0].variables.is_empty());
}

#[test]
fn link_ranges_and_targets_outside_their_arrays_refuse_the_provider() {
    let seq = |events, behaviors, links| Seq {
        name: "S",
        events,
        behaviors,
        links,
        ..Seq::default()
    };
    let e = refused(&Spec::new(
        "P",
        vec![seq(
            vec![ev("E", (0, 2))],
            vec![beh("A", "Behavior_ToySame", (0, 0))],
            vec![(0, -1, 0.0)],
        )],
        vec![],
    ));
    assert_eq!(e, "event link range outside link array in P");
    let e = refused(&Spec::new(
        "P",
        vec![seq(
            vec![],
            vec![beh("A", "Behavior_ToySame", (1, 1))],
            vec![(0, -1, 0.0)],
        )],
        vec![],
    ));
    assert_eq!(e, "behavior link range outside link array in P");
    let e = refused(&Spec::new(
        "P",
        vec![seq(
            vec![],
            vec![beh("A", "Behavior_ToySame", (0, 1))],
            vec![(3, -1, 0.0)],
        )],
        vec![],
    ));
    assert_eq!(e, "behavior link target out of range in P");
}

#[test]
fn only_the_two_provider_classes_are_read() {
    let spec = toy_provider("exact", "BVAR_InstanceData", "BVARLINK_Input");
    let mut env = Env::of(&spec);
    let cmp = env.definition("TestBpd", "ToyProvider.Cmp");
    assert_eq!(
        decode(&mut env.vm, cmp).unwrap_err().to_string(),
        "not a behavior provider: ToyProvider.Cmp (WillowGame.Behavior_CompareObject)"
    );
    let mut ai = toy_provider("exact", "BVAR_InstanceData", "BVARLINK_Input");
    ai.class = "AIBehaviorProviderDefinition";
    let (_env, p) = decoded(&ai);
    assert!(p.values_decoded);
}

#[test]
fn the_enable_condition_and_the_flags_are_read() {
    let seq = Seq {
        name: "S",
        enabled_on_spawn: None,
        mutex: true,
        condition: true,
        ..Seq::default()
    };
    let (env, p) = decoded(&Spec::new("P", vec![seq], vec![]));
    let s = &p.sequences[0];
    // bEnabledOnSpawn untagged: the struct's zero (false), as the C++ reads a declared, untagged field.
    assert!(!s.enabled_on_spawn);
    assert!(s.mutex);
    let condition = s.condition.expect("condition");
    assert_eq!(
        class_path_of(&env.vm, condition).unwrap(),
        "WillowGame.ToyCondition"
    );
    assert_eq!(object_name(&env.vm, condition).unwrap(), "Condition");
    assert!(p.values_decoded);
}

#[test]
fn enum_names_refuses_a_missing_enum() {
    let mut env = Env::of(&toy_provider(
        "exact",
        "BVAR_InstanceData",
        "BVARLINK_Input",
    ));
    assert_eq!(
        enum_names(&mut env.vm, ENUM_PACKAGE, VARIABLE_TYPE_ENUM).unwrap()[..2],
        ["BVAR_None", "BVAR_Object"]
    );
    assert_eq!(
        enum_names(
            &mut env.vm,
            ENUM_PACKAGE,
            "BehaviorProviderDefinition.ENothing"
        )
        .unwrap_err()
        .to_string(),
        "enum not found: GearboxFramework.BehaviorProviderDefinition.ENothing"
    );
}

#[test]
fn a_name_outside_the_enum_reads_as_its_number() {
    assert_eq!(name_at(&["A".to_string()], 0), "A");
    assert_eq!(name_at(&["A".to_string()], 3), "?3");
    assert_eq!(name_at(&[], -1), "?-1");
}

#[test]
fn the_provider_path_leaf_is_the_last_real_name() {
    let mut space = ObjectSpace::new();
    let a = space.names.intern("Outer");
    let b = space.names.intern("Leaf");
    let none = space.names.intern("None");
    let list = Value::array(vec![Value::Name(a), Value::Name(b), Value::Name(none)]);
    assert_eq!(provider_path_leaf(&space, Some(&list)), "Leaf");
    assert_eq!(provider_path_leaf(&space, Some(&Value::Name(a))), "Outer");
    assert_eq!(provider_path_leaf(&space, None), "");
}
