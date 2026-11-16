//! The C++ value-side stand-in for a designer attribute (lane FIX-DESIGNER-RESOLVER, decision 2026-11-16-62): in
//! `ParityMode::Reference` a `WillowGame.DesignerAttributeDefinition` is its own `BaseValue` evaluated on the context
//! (`src/progression.cpp:117`); in Off its value chain is read, as any attribute's. Invented values (AGENTS.md rule 9).

use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::layout::{PropKind, PropertyDecl};
use ow_package::store::PackageStore;

use super::testworld::{Fx, system};
use crate::api::{EvalCx, InitData};

/// A designer attribute whose `BaseValue` is the constant 5 and whose value chain is a constant 9 (no-context chain),
/// read from the source `source` in `mode` (`None`: a world without a store, which is Off).
fn read(mode: Option<ParityMode>) -> (f32, f32) {
    let mut fx = Fx::new();
    let attribute = fx.c.attribute;
    let aid = fx.c.aid;
    let base = fx.w.space.names.intern("BaseValue");
    let designer =
        fx.w.space
            .classes
            .define(
                "WillowGame.DesignerAttributeDefinition",
                Some(attribute),
                vec![PropertyDecl::new(base, PropKind::Struct(aid))],
            )
            .unwrap();
    if let Some(m) = mode {
        fx.w.defs = Some(DefStore::new(
            PackageStore::new_empty(),
            IdentityPolicy::FirstInScanOrder,
            m,
        ));
    }
    let ctx = fx.no_context();
    let nine = fx.constant(9.0);
    let attr = fx.attr_in(designer, "Att_Designer", vec![ctx], vec![nine]);
    let five = fx.data(InitData::constant(5.0));
    let p = fx.w.space.prop(designer, "BaseValue").unwrap();
    fx.w.space
        .write(attr, ow_object::ids::PropAddr::scalar(p), five)
        .unwrap();
    let source = fx.no_context();
    let sys = system();
    let value = sys
        .value_of(attr, Some(source), None, &mut EvalCx::new(&mut fx.w))
        .unwrap()
        .unwrap()
        .value;
    let base = sys
        .base_value_of(attr, Some(source), None, &mut EvalCx::new(&mut fx.w))
        .unwrap()
        .unwrap()
        .value;
    (value, base)
}

#[test]
fn a_designer_attribute_is_its_base_value_only_in_reference_mode() {
    assert_eq!(read(Some(ParityMode::Reference)), (5.0, 5.0));
    assert_eq!(read(Some(ParityMode::Off)), (9.0, 9.0));
    assert_eq!(read(None), (9.0, 9.0));
}
