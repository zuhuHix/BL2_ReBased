//! The `Profiles` reader on invented bytes (AGENTS.md rule 9: every name and value is made up).

use ow_package::testkit::{PackageBuilder, ints};

use super::*;

/// The invented name table the bytes index.
fn names() -> Vec<String> {
    ["None", "ProfA", "ProfB", "BoneOne", "BoneTwo", "AnimX"].iter().map(|s| s.to_string()).collect()
}

fn name(b: &mut Vec<u8>, index: i32, number: i32) {
    b.extend(ints(&[index, number]));
}

fn floats(b: &mut Vec<u8>, v: &[f32]) {
    for x in v {
        b.extend_from_slice(&x.to_le_bytes());
    }
}

/// A component on bone `bone` whose nine transforms are the identity rotation, translated by `t`.
fn component(b: &mut Vec<u8>, bone: i32, t: f32) {
    name(b, bone, 0);
    for _ in 0..9 {
        floats(b, &[0.0, 0.0, 0.0, 1.0, t, 0.0, 0.0]);
    }
}

/// A profile named `profile` with `bones` components.
fn profile(b: &mut Vec<u8>, profile: i32, bones: &[i32]) {
    name(b, profile, 0);
    floats(b, &[-1.0, 1.0, -0.5, 0.5]);
    b.extend(ints(&[bones.len() as i32]));
    for (i, &bone) in bones.iter().enumerate() {
        component(b, bone, i as f32);
    }
    for k in 0..9 {
        name(b, 5, k);
    }
}

/// Two profiles: `ProfA` with two components, `ProfB` with one.
fn value() -> Vec<u8> {
    let mut b = ints(&[2]);
    profile(&mut b, 1, &[3, 4]);
    profile(&mut b, 2, &[3]);
    b
}

#[test]
fn two_profiles_decode_to_their_values() {
    let v = value();
    assert_eq!(v.len(), 4 + 2 * PROFILE_FIXED_BYTES + 3 * COMPONENT_BYTES, "the fitted sizes");
    let p = read(&v, &names()).unwrap();
    assert_eq!(p.len(), 2);
    assert_eq!((p[0].profile_name.as_str(), p[1].profile_name.as_str()), ("ProfA", "ProfB"));
    assert_eq!((p[0].horizontal_range, p[0].vertical_range), ([-1.0, 1.0], [-0.5, 0.5]));
    let bones: Vec<&str> = p[0].components.iter().map(|c| c.bone_name.as_str()).collect();
    assert_eq!(bones, ["BoneOne", "BoneTwo"]);
    assert_eq!(p[0].components[1].transforms[8], AimTransform { quaternion: [0.0, 0.0, 0.0, 1.0], translation: [1.0, 0.0, 0.0] });
    assert_eq!(p[1].components.len(), 1);
    assert_eq!(p[0].anim_names[0], "AnimX", "number 0 is the plain name");
    assert_eq!(p[0].anim_names[2], "AnimX_1", "number N + 1 is Name_N");
}

#[test]
fn every_oracle_miss_is_refused() {
    let n = names();
    let v = value();
    // A short value: the last name cut off.
    assert!(matches!(read(&v[..v.len() - 4], &n), Err(ProfilesError::Short { .. })));
    // A long value: four bytes left over.
    let mut long = v.clone();
    long.extend(ints(&[0]));
    assert!(matches!(read(&long, &n), Err(ProfilesError::Trailing { .. })));
    // A count past the bytes, and a negative one.
    let mut big = v.clone();
    big[..4].copy_from_slice(&1000i32.to_le_bytes());
    assert!(matches!(read(&big, &n), Err(ProfilesError::Count { at: 0, count: 1000 })));
    big[..4].copy_from_slice(&(-1i32).to_le_bytes());
    assert!(matches!(read(&big, &n), Err(ProfilesError::Count { .. })));
    // A name index outside the table (the first profile's name).
    let mut bad = v.clone();
    bad[4..8].copy_from_slice(&99i32.to_le_bytes());
    assert!(matches!(read(&bad, &n), Err(ProfilesError::Name { at: 4, index: 99, .. })));
    // A non-unit quaternion: the first component's first transform's w (after its bone name).
    let first_quat = 4 + 8 + 16 + 4 + 8;
    let mut skew = v.clone();
    skew[first_quat + 12..first_quat + 16].copy_from_slice(&0.5f32.to_le_bytes());
    assert!(matches!(read(&skew, &n), Err(ProfilesError::NotUnit { .. })));
    // A NaN in a range.
    let mut nan = v;
    nan[12..16].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(matches!(read(&nan, &n), Err(ProfilesError::NotFinite { at: 12 })));
}

#[test]
fn the_export_read_finds_the_tag_and_an_export_without_one_has_none() {
    let mut p = PackageBuilder::new_empty();
    // The test's names are the package's own: intern them first, so the indices the bytes hold are theirs.
    for (i, s) in names().iter().enumerate() {
        assert_eq!(p.name(s), i as i32);
    }
    let core = p.add_import("Core", "Package", 0, "Core");
    let class = p.add_import("Core", "Class", core, "Object");
    let none = p.none();
    let v = value();
    let elements = &v[4..];
    let mut payload = ints(&[0]);
    payload.extend(p.array_tag("Profiles", 2, elements));
    payload.extend_from_slice(&none);
    let with = p.add_export(class, "Node", payload);
    let mut plain = ints(&[0]);
    plain.extend_from_slice(&none);
    let without = p.add_export(class, "Plain", plain);
    let pkg = Package::from_bytes("Toy.upk".into(), "Toy".into(), &p.build()).unwrap();
    let read = read_export(&pkg, with, 4).unwrap();
    let profiles = read.profiles.unwrap();
    assert_eq!(profiles.iter().map(|x| x.profile_name.as_str()).collect::<Vec<_>>(), ["ProfA", "ProfB"]);
    assert_eq!(read.tags_end, pkg.object(with).unwrap().size as usize, "the walk ended at the payload's end");
    assert_eq!(read_export(&pkg, without, 4).unwrap().profiles, None);
}

/// The installed census (needs the game: `OPENWILLOW_BL2` the install, `OW_AIM_PACKAGES` a `;`-separated list of file
/// packages): every export of a class named `*AnimNodeAimOffset` is read with the fitted layout and its oracle. Counts
/// only are printed, and nothing may be refused.
#[test]
#[ignore = "needs the installed game"]
fn installed_aim_profiles_census() {
    let (Some(game), Ok(list)) = (std::env::var_os("OPENWILLOW_BL2"), std::env::var("OW_AIM_PACKAGES")) else {
        panic!("set OPENWILLOW_BL2 and OW_AIM_PACKAGES");
    };
    let cooked = std::path::PathBuf::from(game).join("WillowGame").join("CookedPCConsole");
    let mut refused = 0;
    for name in list.split(';').filter(|n| !n.is_empty()) {
        let pkg = Package::load(&cooked.join(name)).unwrap();
        for i in 1..=pkg.exports.len() as i32 {
            let class = pkg.object(i).ok().filter(|o| o.class != 0).and_then(|o| pkg.object(o.class).ok()).map(|c| c.name.clone());
            if !class.is_some_and(|c| c.ends_with("AnimNodeAimOffset")) {
                continue;
            }
            match read_export(&pkg, i, 4) {
                Ok(ExportProfiles { profiles: Some(p), .. }) => {
                    let components: Vec<usize> = p.iter().map(|x| x.components.len()).collect();
                    println!("CENSUS {name} export {i}: {} profiles, components {components:?}", p.len());
                }
                Ok(ExportProfiles { profiles: None, .. }) => println!("CENSUS {name} export {i}: no Profiles tag"),
                Err(e) => {
                    refused += 1;
                    println!("CENSUS {name} export {i} refused: {e}");
                }
            }
        }
    }
    assert_eq!(refused, 0, "every installed node reads");
}
