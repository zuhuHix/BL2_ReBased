//! Owner: lane H8-MATERIALS. Tests of the light reads on invented values (AGENTS.md rule 9).

use super::*;
use ow_package::testkit::PackageBuilder;

/// A package with one export: `junk` bytes, then the tags `LightmassSettings` (an int, standing in for the struct),
/// `Brightness` 7 and `LightColor` (R 255, G 128, B 0), then `None`.
fn light_package(junk: &[u8]) -> Package {
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let class = b.add_import("Core", "Class", 0, "DominantDirectionalLightComponent");
    let mut payload = junk.to_vec();
    payload.extend(b.int_tag(DIRECTIONAL_ANCHOR, 1));
    payload.extend(b.float_tag("Brightness", 7.0));
    payload.extend(b.struct_tag("LightColor", "Color", &[0, 128, 255, 255]));
    payload.extend(b.none());
    b.add_export(class, "Probe", payload);
    Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
}

#[test]
fn the_anchored_stream_is_the_one_that_parses_to_the_end() {
    // The light data in front holds the anchor's name once more: that start must not parse to the end.
    let mut probe = PackageBuilder::new_empty();
    probe.names(["None"]);
    let anchor_name = {
        let pkg = light_package(&[0u8; 4]);
        let index = pkg
            .names
            .iter()
            .position(|n| n == DIRECTIONAL_ANCHOR)
            .unwrap();
        i32::try_from(index).unwrap()
    };
    let mut junk = vec![0x11u8; 6];
    junk.extend(anchor_name.to_le_bytes());
    junk.extend(0i32.to_le_bytes());
    junk.extend([0x22u8; 9]);
    let pkg = light_package(&junk);
    let (at, entries) =
        anchored_tags(&pkg, 1, DIRECTIONAL_ANCHOR, &["Brightness", "LightColor"]).unwrap();
    assert_eq!(at, junk.len());
    assert_eq!(entries.len(), 3);
    assert_eq!(
        entries
            .iter()
            .find_map(|e| match (e.name.as_str(), &e.value) {
                ("LightColor", PropValue::Struct { fields, .. }) => color_fields(fields),
                _ => None,
            }),
        Some([255, 128, 0])
    );
    // A required tag the stream lacks: no start qualifies.
    assert!(anchored_tags(&pkg, 1, DIRECTIONAL_ANCHOR, &["Radius"]).is_err());
    let _ = probe;
}

#[test]
fn colours_are_linear_and_the_sun_shines_along_its_forward_axis() {
    assert_eq!(linear_color([255, 0, 255]), [1.0, 0.0, 1.0]);
    let mid = linear_color([128, 128, 128])[0];
    assert!((mid - 0.2195).abs() < 1e-3, "{mid}");
    let sun = Sun {
        path: "TheWorld.PersistentLevel.DominantDirectionalLight_0".into(),
        level: "Test_P".into(),
        rotation: Rotator::new(-16384, 0, 0),
        color: [1.0; 3],
        brightness: 2.0,
        tags_at: 0,
    };
    // Pitched straight down: the light shines down, so the sun is straight up.
    let t = sun.toward();
    assert!(t[2] > 0.999 && t[0].abs() < 1e-6, "{t:?}");
}
