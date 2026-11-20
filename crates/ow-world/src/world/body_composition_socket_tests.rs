//! The skeletal component's socket attach in the body composition (lane IMPL-BODYCOMP-SOCKET-BONE-TABLES), coded
//! from NATIVE_BODYCOMP_SOCKET_BONE_TABLES.md section 4 alone, on the parent module's invented fixture (AGENTS.md rule
//! 9: invented values; the class, struct and field names are the packages'). The tests follow the note's section 4.1
//! list. Every refusal is checked write-free against #518's three counters and the name table's length.
//! AI-assisted (Claude Code). **UNVERIFIED** in the running game.

use super::*;
use crate::world::skeletal::{ReferenceSkeleton, SkeletalMeshes, SkeletonBone, SkeletonSource};

/// The skeleton source: the given bone texts for the fixture's mesh, by its leaf name.
struct Bones(Vec<&'static str>);

impl SkeletonSource for Bones {
    fn skeleton(&self, _package: &str, _path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        if leaf != "ToyBodyMesh" {
            return Ok(None);
        }
        let bones = self
            .0
            .iter()
            .enumerate()
            .map(|(i, n)| SkeletonBone { name: (*n).to_string(), parent: if i == 0 { None } else { Some(0) }, bind: None })
            .collect();
        Ok(Some(ReferenceSkeleton { bones }))
    }
}

/// The fixture's mesh export, loaded, and its one socket handle, which the store leaves unloaded.
fn mesh(e: &mut Env) -> (ObjRef, ObjRef) {
    let path = ObjectPath::parse("GD_ToyBody.ToyBodyMesh").unwrap();
    let m = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    e.vm.defs.ensure_loaded(&mut e.vm.space, m).unwrap();
    let Value::Object(Some(cold)) = prop(&e.vm, m, "Sockets").elements().unwrap()[0].clone() else { panic!("socket") };
    assert!(e.vm.defs.is_unloaded(&e.vm.space, cold), "the socket handle is cold");
    (m, cold)
}

fn struct_of(vm: &Vm, class: ClassId, name: &str) -> StructId {
    let p = vm.space.prop(class, name).unwrap();
    match &vm.space.classes.decl(p).unwrap().kind {
        PropKind::Struct(s) => *s,
        other => panic!("{name}: {other:?}"),
    }
}

fn floats3(vm: &Vm, s: StructId, v: [f32; 3]) -> Value {
    value(vm, s, vec![("X", Value::Float(v[0])), ("Y", Value::Float(v[1])), ("Z", Value::Float(v[2]))])
}

fn ints3(vm: &Vm, s: StructId, v: [i32; 3]) -> Value {
    value(vm, s, vec![("Pitch", Value::Int(v[0])), ("Yaw", Value::Int(v[1])), ("Roll", Value::Int(v[2]))])
}

/// A loaded socket object: `socket` on `bone`, with its offsets and a scale the attach must ignore.
fn socket(e: &mut Env, socket: &str, bone: &str, location: [f32; 3], rotation: [i32; 3]) -> ObjRef {
    let class = e.vm.find_class("Engine.SkeletalMeshSocket").unwrap();
    let s = make(&mut e.vm, class, "ToySocket");
    let (sn, bn) = (name(&mut e.vm, socket), name(&mut e.vm, bone));
    put(&mut e.vm, s, "SocketName", sn);
    put(&mut e.vm, s, "BoneName", bn);
    let vec_s = struct_of(&e.vm, class, "RelativeLocation");
    let rot_s = struct_of(&e.vm, class, "RelativeRotation");
    let loc = floats3(&e.vm, vec_s, location);
    let rot = ints3(&e.vm, rot_s, rotation);
    let scale = floats3(&e.vm, vec_s, [7.0, 8.0, 9.0]);
    put(&mut e.vm, s, "RelativeLocation", loc);
    put(&mut e.vm, s, "RelativeRotation", rot);
    put(&mut e.vm, s, "RelativeScale", scale);
    s
}

/// The fixture's mesh with `sockets` (objects, in order) and the skeleton source `bones`; the parent template uses it.
fn parent_with(e: &mut Env, sockets: Vec<ObjRef>, bones: Option<Vec<&'static str>>) -> (ObjRef, ObjRef) {
    let (m, _) = mesh(e);
    put(&mut e.vm, m, "Sockets", Value::array(sockets.into_iter().map(|s| Value::Object(Some(s))).collect()));
    if let Some(b) = bones {
        SkeletalMeshes::install(&mut e.vm, Box::new(Bones(b))).unwrap();
    }
    let skel = e.skeletal;
    let pt = make(&mut e.vm, skel, "Body_Template");
    put(&mut e.vm, pt, "SkeletalMesh", Value::Object(Some(m)));
    (pt, m)
}

/// Attachment 0 (the parent, `pt`) and one child per `(template, socket)` under it, then the pawn.
fn body(e: &mut Env, pt: ObjRef, children: &[(ObjRef, &str)]) -> ObjRef {
    let mut list = vec![attachment(e, A { template: Some(pt), ..A::default() })];
    for (t, s) in children {
        list.push(attachment(e, A { base: 0, template: Some(*t), socket: s, ..A::default() }));
    }
    pawn_with(e, list)
}

fn child(e: &mut Env, name: &str) -> ObjRef {
    let skel = e.skeletal;
    make(&mut e.vm, skel, name)
}

/// The made parent (entry 0's component) and its `Attachments` list.
fn parent_list(e: &Env, p: ObjRef) -> (ObjRef, Vec<Value>) {
    let parent = made_of(e, &entries(e, p)[0]);
    (parent, prop(&e.vm, parent, "Attachments").elements().unwrap().to_vec())
}

fn text(e: &Env, v: &Value) -> String {
    let Value::Name(n) = v else { panic!("{v:?}") };
    e.vm.space.names.text(*n).into_owned()
}

/// `(component, bone, location, rotation, scale)` of one `Attachments` entry.
fn entry(e: &Env, v: &Value) -> (Option<ObjRef>, String, [f32; 3], [i32; 3], [f32; 3]) {
    let fl = |x: &Value| [0, 1, 2].map(|i| match f(e, x, ["X", "Y", "Z"][i]) {
        Value::Float(v) => v,
        o => panic!("{o:?}"),
    });
    let it = |x: &Value| [0, 1, 2].map(|i| match f(e, x, ["Pitch", "Yaw", "Roll"][i]) {
        Value::Int(v) => v,
        o => panic!("{o:?}"),
    });
    let Value::Object(c) = f(e, v, "Component") else { panic!() };
    (c, text(e, &f(e, v, "BoneName")), fl(&f(e, v, "RelativeLocation")), it(&f(e, v, "RelativeRotation")), fl(&f(e, v, "RelativeScale")))
}

/// A refusal of the build: its kind and text, with #518's counters and the name table's length unchanged, no entry
/// written and no partial-writes prefix.
fn refusal(e: &mut Env, p: ObjRef) -> (Missing, String) {
    let names = e.vm.space.names.len();
    let (missing, what) = refused(e, p);
    assert_eq!(e.vm.space.names.len(), names, "nothing interned: {what}");
    (missing, what)
}

/// Test 1: a socket hit. The entry takes the socket's location and rotation (added to zero), its bone, and unit scale;
/// the socket's own scale is ignored. The child's back link is the parent, and the owner retry is not run.
#[test]
fn a_socket_hit_writes_the_entry_and_the_back_link() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Shield", "Hips", [1.5, -2.0, 3.25], [100, -200, 300]);
    let (pt, _) = parent_with(&mut e, vec![s], None);
    let head = child(&mut e, "Shield_Template");
    let p = body(&mut e, pt, &[(head, "Shield")]);
    run(&mut e, p).unwrap();
    let (parent, list) = parent_list(&e, p);
    let made = made_of(&e, &entries(&e, p)[1]);
    assert_eq!(list.len(), 1);
    assert_eq!(entry(&e, &list[0]), (Some(made), "Hips".into(), [1.5, -2.0, 3.25], [100, -200, 300], [1.0; 3]));
    assert_eq!(prop(&e.vm, made, "AttachedToSkelComponent"), Value::Object(Some(parent)));
    let components = prop(&e.vm, p, "Components");
    assert!(!components.elements().unwrap().contains(&Value::Object(Some(made))), "no owner retry for a true attach");
}

/// Test 2: a bone hit (no socket of that name) has zero offsets and the name as its bone; a socket and a bone of the
/// same name: the socket wins.
#[test]
fn a_bone_hit_has_zero_offsets_and_a_socket_wins_over_a_bone() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Spine3", "Spine1", [4.0, 5.0, 6.0], [1, 2, 3]);
    let (pt, _) = parent_with(&mut e, vec![s], Some(vec!["Root", "Spine3", "R_Thigh"]));
    let (a, b) = (child(&mut e, "A_Template"), child(&mut e, "B_Template"));
    let p = body(&mut e, pt, &[(a, "R_Thigh"), (b, "Spine3")]);
    run(&mut e, p).unwrap();
    let (_, list) = parent_list(&e, p);
    assert_eq!(list.len(), 2);
    let (_, bone, loc, rot, scale) = entry(&e, &list[0]);
    assert_eq!((bone.as_str(), loc, rot, scale), ("R_Thigh", [0.0; 3], [0; 3], [1.0; 3]), "a bone hit");
    let (_, bone, loc, ..) = entry(&e, &list[1]);
    assert_eq!((bone.as_str(), loc), ("Spine1", [4.0, 5.0, 6.0]), "the socket wins over the bone Spine3");
}

/// Test 3: no socket and no bone is false: the owner retry runs as today. A name the remap makes None, and a parent
/// without a mesh, are false with no socket and no skeleton read (the cold socket and the missing source untouched).
#[test]
fn a_miss_none_name_or_no_mesh_is_the_owner_retry() {
    for case in ["miss", "none", "no mesh"] {
        let mut e = env();
        answer_render_hooks_for_tests(true);
        let (m, cold) = mesh(&mut e);
        let bones = if case == "miss" { Some(vec!["Root"]) } else { None };
        let (pt, _) = parent_with(&mut e, vec![], bones);
        if case != "miss" {
            put(&mut e.vm, m, "Sockets", Value::array(vec![Value::Object(Some(cold))]));
        }
        if case == "none" {
            let (gestalt, remap) = remap_struct(&e);
            let (from, to) = (name(&mut e.vm, "Hips"), name(&mut e.vm, "None"));
            let entry = value(&e.vm, remap, vec![("OriginalSocketName", from), ("MangledSocketName", to)]);
            let g = value(&e.vm, gestalt, vec![("SocketMappings", Value::array(vec![entry]))]);
            put(&mut e.vm, pt, "GestaltData", g);
        }
        if case == "no mesh" {
            put(&mut e.vm, pt, "SkeletalMesh", Value::Object(None));
        }
        let head = child(&mut e, "Head_Template");
        let p = body(&mut e, pt, &[(head, "Hips")]);
        run(&mut e, p).unwrap();
        let (_, list) = parent_list(&e, p);
        assert!(list.is_empty(), "{case}: no entry");
        let made = made_of(&e, &entries(&e, p)[1]);
        assert_eq!(prop(&e.vm, made, "Owner"), Value::Object(Some(p)), "{case}: the owner retry");
        assert!(e.vm.defs.is_unloaded(&e.vm.space, cold), "{case}: nothing loaded");
    }
}

/// The remap's entry struct.
fn remap_struct(e: &Env) -> (StructId, StructId) {
    let gestalt = struct_of(&e.vm, e.skeletal, "GestaltData");
    let layout = e.vm.space.classes.struct_layout(gestalt).unwrap();
    let (_, d) = layout.field(e.vm.space.names.find("SocketMappings").unwrap()).unwrap();
    let PropKind::Array(inner) = &d.kind else { panic!() };
    let PropKind::Struct(r) = inner.as_ref() else { panic!() };
    (gestalt, *r)
}

/// Test 4: the remap rewrites the name before the search, and the first matching entry wins.
#[test]
fn the_remap_rewrites_the_name_and_the_first_entry_wins() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Launcher", "Spine3", [9.0, 0.0, 0.0], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![s], Some(vec!["Root"]));
    let (gestalt, remap) = remap_struct(&e);
    let mut mappings = Vec::new();
    for (from, to) in [("Gun", "Launcher"), ("Gun", "Root"), ("Other", "Root")] {
        let (a, b) = (name(&mut e.vm, from), name(&mut e.vm, to));
        mappings.push(value(&e.vm, remap, vec![("OriginalSocketName", a), ("MangledSocketName", b)]));
    }
    let g = value(&e.vm, gestalt, vec![("SocketMappings", Value::array(mappings))]);
    put(&mut e.vm, pt, "GestaltData", g);
    let head = child(&mut e, "Gun_Template");
    let p = body(&mut e, pt, &[(head, "Gun")]);
    run(&mut e, p).unwrap();
    let (_, list) = parent_list(&e, p);
    let (_, bone, loc, ..) = entry(&e, &list[0]);
    assert_eq!((bone.as_str(), loc), ("Spine3", [9.0, 0.0, 0.0]), "Gun -> Launcher (the first entry), a socket hit");
}

/// Test 5: two attaches of one component overwrite its entry (the list does not grow); two components give two
/// entries in order. Driven through the write itself: within one build each attachment makes a new component.
#[test]
fn a_second_attach_of_one_component_overwrites_its_entry() {
    let mut e = env();
    let skel = e.skeletal;
    let parent = make(&mut e.vm, skel, "Parent");
    let (one, two) = (make(&mut e.vm, skel, "One"), make(&mut e.vm, skel, "Two"));
    let entry_struct = {
        let p = e.vm.space.prop(skel, "Attachments").unwrap();
        let PropKind::Array(inner) = &e.vm.space.classes.decl(p).unwrap().kind else { panic!() };
        let PropKind::Struct(s) = inner.as_ref() else { panic!() };
        *s
    };
    let bone = |e: &mut Env, b: &str| e.vm.space.names.intern(b);
    let sa = |bone: NameId, x: f32| SocketAttach {
        parent: 0,
        bone,
        location: [x, 0.0, 0.0],
        rotation: [0; 3],
        entry: entry_struct,
        back_link: true,
    };
    let (b1, b2) = (bone(&mut e, "Hips"), bone(&mut e, "Spine3"));
    write_socket_attach(&mut e.vm, parent, one, &sa(b1, 1.0)).unwrap();
    write_socket_attach(&mut e.vm, parent, one, &sa(b2, 2.0)).unwrap();
    let list = prop(&e.vm, parent, "Attachments").elements().unwrap().to_vec();
    assert_eq!(list.len(), 1, "overwritten, not appended");
    assert_eq!(entry(&e, &list[0]).1, "Spine3");
    assert_eq!(entry(&e, &list[0]).2, [2.0, 0.0, 0.0]);
    write_socket_attach(&mut e.vm, parent, two, &sa(b1, 3.0)).unwrap();
    let list = prop(&e.vm, parent, "Attachments").elements().unwrap().to_vec();
    assert_eq!(list.iter().map(|v| entry(&e, v).0).collect::<Vec<_>>(), vec![Some(one), Some(two)]);
}

/// Test 6: a non-skeletal new component gets its entry and no back link.
#[test]
fn a_non_skeletal_component_gets_the_entry_and_no_back_link() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![s], None);
    let cyl = e.cylinder;
    let c = make(&mut e.vm, cyl, "Cyl_Template");
    let p = body(&mut e, pt, &[(c, "Shield")]);
    run(&mut e, p).unwrap();
    let (_, list) = parent_list(&e, p);
    let made = made_of(&e, &entries(&e, p)[1]);
    assert_eq!(entry(&e, &list[0]).0, Some(made));
    assert!(e.vm.space.prop(cyl, "AttachedToSkelComponent").is_none(), "the cylinder has no back link field");
}

/// Test 7: an unloaded socket handle before the hit is the layer's `LayerAbsent`, write-free; one after the hit is
/// never read (the ordered lookup).
#[test]
fn an_unloaded_socket_before_the_hit_refuses_and_after_it_is_never_read() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let (_, cold) = mesh(&mut e);
    let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![cold, s], None);
    let head = child(&mut e, "Head_Template");
    let p = body(&mut e, pt, &[(head, "Shield")]);
    let (missing, what) = refusal(&mut e, p);
    assert!(matches!(missing, Missing::LayerAbsent(_)), "{missing:?}");
    assert!(what.contains("the unloaded socket Socket_Cold"), "{what}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, cold), "nothing loaded");

    let mut e = env();
    answer_render_hooks_for_tests(true);
    let (_, cold) = mesh(&mut e);
    let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![s, cold], None);
    let head = child(&mut e, "Head_Template");
    let p = body(&mut e, pt, &[(head, "Shield")]);
    run(&mut e, p).unwrap();
    assert_eq!(parent_list(&e, p).1.len(), 1, "the hit stands");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, cold), "the handle after the hit is not read");
}

/// Test 8: a bone search with no skeleton source is the layer's `LayerAbsent`, write-free.
#[test]
fn a_bone_search_without_a_source_refuses() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let (pt, _) = parent_with(&mut e, vec![], None);
    let head = child(&mut e, "Head_Template");
    let p = body(&mut e, pt, &[(head, "Hips")]);
    let (missing, what) = refusal(&mut e, p);
    assert_eq!(missing, Missing::LayerAbsent(crate::world::skeletal::LAYER));
    assert!(what.contains("no skeleton source"), "{what}");
}

/// Test 9: each gate's refusal, write-free, in Off's order; a decal child class and a decal with the flag set pass;
/// the overlaps report the detach and the parent-animation stop.
#[test]
fn each_gate_refuses_write_free_in_order() {
    let decal_class = |e: &Env| e.vm.space.classes.by_path(DECAL).unwrap();
    for case in ["detach", "parent anim", "decal", "placement", "decal child", "decal flag set", "decal detach", "decal parent anim"] {
        let mut e = env();
        answer_render_hooks_for_tests(true);
        let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [0; 3]);
        let (pt, _) = parent_with(&mut e, vec![s], None);
        let kid = match case {
            "detach" | "parent anim" | "placement" => child(&mut e, "Head_Template"),
            "decal child" => {
                let base = decal_class(&e);
                let sub = class(&mut e.vm, "Test.ToyDecalChild", base, &[]);
                make(&mut e.vm, sub, "Decal_Template")
            }
            _ => {
                let d = decal_class(&e);
                make(&mut e.vm, d, "Decal_Template")
            }
        };
        match case {
            "detach" | "decal detach" => put(&mut e.vm, kid, "bAttached", Value::Bool(true)),
            "parent anim" | "decal parent anim" => {
                let other = child(&mut e, "Anim_Parent");
                put(&mut e.vm, pt, "ParentAnimComponent", Value::Object(Some(other)));
            }
            "placement" => put(&mut e.vm, pt, "bAttached", Value::Bool(true)),
            "decal flag set" => put(&mut e.vm, pt, "bAcceptsDynamicDecals", Value::Bool(true)),
            _ => {}
        }
        let p = body(&mut e, pt, &[(kid, "Shield")]);
        let want = match case {
            "detach" | "decal detach" => Some((Missing::NotRead, SOCKET_DETACH)),
            "parent anim" | "decal parent anim" => Some((Missing::NotRead, SOCKET_PARENT_ANIM)),
            "decal" => Some((Missing::NotRead, SOCKET_DECAL)),
            "placement" => Some((Missing::LayerAbsent(BONE_TRANSFORM_LAYER), SOCKET_PLACEMENT)),
            _ => None,
        };
        match want {
            Some((missing, what)) => assert_eq!(refusal(&mut e, p), (missing, what.to_string()), "{case}"),
            None => {
                run(&mut e, p).unwrap();
                assert_eq!(parent_list(&e, p).1.len(), 1, "{case}: attached");
            }
        }
    }
}

/// Test 10: a refusal on the second of two socket attaches leaves the whole build unwritten: the counters unchanged,
/// no entry of the first attach, no partial-writes prefix.
#[test]
fn a_refusal_on_the_second_attach_writes_nothing() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![s], None);
    let (one, two) = (child(&mut e, "One_Template"), child(&mut e, "Two_Template"));
    put(&mut e.vm, two, "bAttached", Value::Bool(true));
    let p = body(&mut e, pt, &[(one, "Shield"), (two, "Shield")]);
    assert_eq!(refusal(&mut e, p), (Missing::NotRead, SOCKET_DETACH.to_string()));
    assert!(prop(&e.vm, pt, "Attachments").elements().unwrap().is_empty(), "the template's list is untouched");
}

/// Test 11: under `ParityMode::Reference` the world registers neither the pawn wrapper nor the body composition (they
/// are Off only), so the socket attach is never reached and the stub policy answers as before.
#[test]
fn reference_binds_nothing() {
    let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Reference);
    ow_vm::api::NativePack::register(&crate::world::Pack, &mut vm).unwrap();
    for key in crate::world::pawn_meshes::KEYS {
        assert!(!vm.has_native(key), "{key}");
    }
}

/// Test 12: Maya's shape on invented names: three socket hits and five bone hits, in her order, give eight entries in
/// that order (the sockets' offsets for the hits, zero for the bones).
#[test]
fn maya_s_shape_gives_eight_entries_in_order() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let sockets = vec![
        socket(&mut e, "ToyFX", "ToyRoot", [1.0, 1.0, 1.0], [1, 1, 1]),
        socket(&mut e, "ToyShield", "ToyHips", [2.0, 0.0, 0.0], [0, 2, 0]),
        socket(&mut e, "ToyClassMod", "ToySpine1", [3.0, 0.0, 0.0], [0, 0, 3]),
        socket(&mut e, "ToyLauncher", "ToySpine3", [4.0, 0.0, 0.0], [4, 0, 0]),
    ];
    let (pt, _) = parent_with(&mut e, sockets, Some(vec!["ToyRoot", "ToyHips", "ToySpine1", "ToySpine3", "ToyThigh"]));
    let order = ["ToyRoot", "ToySpine3", "ToySpine3", "ToyShield", "ToyClassMod", "ToyThigh", "ToyHips", "ToyLauncher"];
    let kids: Vec<(ObjRef, &str)> = order.iter().map(|s| (child(&mut e, "Gear_Template"), *s)).collect();
    let p = body(&mut e, pt, &kids);
    run(&mut e, p).unwrap();
    let (_, list) = parent_list(&e, p);
    let got: Vec<(String, f32)> = list.iter().map(|v| {
        let (_, bone, loc, ..) = entry(&e, v);
        (bone, loc[0])
    }).collect();
    let want = [("ToyRoot", 0.0), ("ToySpine3", 0.0), ("ToySpine3", 0.0), ("ToyHips", 2.0), ("ToySpine1", 3.0), ("ToyThigh", 0.0), ("ToyHips", 0.0), ("ToySpine3", 4.0)];
    assert_eq!(got, want.map(|(b, x)| (b.to_string(), x)).to_vec());
    let made: Vec<Option<ObjRef>> = entries(&e, p)[1..].iter().map(|x| Some(made_of(&e, x))).collect();
    assert_eq!(list.iter().map(|v| entry(&e, v).0).collect::<Vec<_>>(), made, "in the attachments' order");
}

/// Test 13: numbered names compare by id. A socket named `Foo` number 2 (printed `Foo_1`) matches `Foo_1`, not `Foo`;
/// a bone text `Foo_0` matches the name `Foo_0`, not `Foo`; a bone text the table never saw matches nothing; nothing is
/// interned.
#[test]
fn numbered_names_compare_by_id() {
    let mut e = env();
    let (m, _) = mesh(&mut e);
    let s = socket(&mut e, "Foo_1", "Hips", [0.0; 3], [0; 3]);
    put(&mut e.vm, m, "Sockets", Value::array(vec![Value::Object(Some(s))]));
    SkeletalMeshes::install(&mut e.vm, Box::new(Bones(vec!["Root", "Bar_0", "NeverSeenBone"]))).unwrap();
    let foo1 = e.vm.space.names.intern("Foo_1");
    let foo = e.vm.space.names.intern("Foo");
    let bar0 = e.vm.space.names.intern("Bar_0");
    let bar = e.vm.space.names.intern("Bar");
    let names = e.vm.space.names.len();
    let find = |e: &Env, n| crate::world::skeletal::find_socket(&e.vm, m, n).unwrap().map(|x| x.socket.name);
    assert_eq!(find(&e, foo1).as_deref(), Some("Foo_1"));
    assert_eq!(find(&e, foo), None);
    let bone = |e: &Env, n| crate::world::skeletal::find_bone(&e.vm, m, n).unwrap();
    assert_eq!(bone(&e, bar0), Some(1));
    assert_eq!(bone(&e, bar), None);
    assert_eq!(e.vm.space.names.len(), names, "nothing interned (NeverSeenBone stays unseen)");
    assert!(e.vm.space.names.find("NeverSeenBone").is_none());
}

/// Review r1 of #641, item 1: a None name the remap maps to a socket is a socket attach, not the owner retry (beside
/// test 3's socket remapped to None).
#[test]
fn a_none_name_remapped_to_a_socket_is_attached() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Shield", "Hips", [5.0, 0.0, 0.0], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![s], None);
    let (gestalt, remap) = remap_struct(&e);
    let (from, to) = (name(&mut e.vm, "None"), name(&mut e.vm, "Shield"));
    let entry_value = value(&e.vm, remap, vec![("OriginalSocketName", from), ("MangledSocketName", to)]);
    let g = value(&e.vm, gestalt, vec![("SocketMappings", Value::array(vec![entry_value]))]);
    put(&mut e.vm, pt, "GestaltData", g);
    let head = child(&mut e, "Head_Template");
    let p = body(&mut e, pt, &[(head, "None")]);
    run(&mut e, p).unwrap();
    let (_, list) = parent_list(&e, p);
    assert_eq!(list.len(), 1, "remapped to Shield, a socket hit");
    let (_, bone, loc, ..) = entry(&e, &list[0]);
    assert_eq!((bone.as_str(), loc), ("Hips", [5.0, 0.0, 0.0]));
    let made = made_of(&e, &entries(&e, p)[1]);
    assert_ne!(prop(&e.vm, made, "Owner"), Value::Object(Some(p)), "not the owner retry");
}

/// Review r1 of #641, item 2: a destination whose `RelativeRotation.Yaw` is declared a float cannot take the int
/// rotation. The preflight checks the planned entry against the heap's write rules, so the build refuses before any
/// write: #518's three counters and the name table's length unchanged, no entry and no component made.
#[test]
fn a_malformed_destination_refuses_before_any_write() {
    ODD_ATTACHMENT.with(|c| c.set(true));
    let mut e = env();
    ODD_ATTACHMENT.with(|c| c.set(false));
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [1, 2, 3]);
    let (pt, _) = parent_with(&mut e, vec![s], None);
    let head = child(&mut e, "Head_Template");
    let p = body(&mut e, pt, &[(head, "Shield")]);
    let before = Counters::read(&e.vm);
    let names = e.vm.space.names.len();
    let err = run(&mut e, p).unwrap_err();
    assert!(matches!(&err, VmError::Script { message, .. } if message.contains("cannot take the socket attach's entry")), "{err:?}");
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert_eq!(e.vm.space.names.len(), names, "nothing interned");
    assert!(entries(&e, p).is_empty(), "no entry appended");
}

/// Review r2 of #641: a child template whose `bAttached` reads but which has no slot for `AttachedToSkelComponent`
/// (the made child takes the template's slots) refuses in the preflight, before the build writes: #518's three
/// counters and the name table's length unchanged, no entry appended.
#[test]
fn a_template_without_the_back_link_slot_refuses_before_any_write() {
    let mut e = env();
    answer_render_hooks_for_tests(true);
    let s = socket(&mut e, "Shield", "Hips", [0.0; 3], [0; 3]);
    let (pt, _) = parent_with(&mut e, vec![s], None);
    let head = child(&mut e, "Head_Template");
    let skel = e.skeletal;
    let slot_of = |e: &Env, name: &str| {
        let p = e.vm.space.prop(skel, name).unwrap();
        e.vm.space.classes.decl(p).unwrap();
        ow_object::ids::PropAddr::scalar(p)
    };
    let back = slot_of(&e, "AttachedToSkelComponent");
    // Drop slots from the end until the back link's slot is gone while `bAttached` still reads.
    while e.vm.space.read(head, back).is_ok() {
        let slots = &mut e.vm.space.heap.get_mut(head).unwrap().slots;
        assert!(!slots.is_empty(), "the back link's slot could not be removed");
        slots.pop();
    }
    assert!(e.vm.space.read(head, slot_of(&e, "bAttached")).is_ok(), "bAttached must still read for this case");
    let p = body(&mut e, pt, &[(head, "Shield")]);
    let before = Counters::read(&e.vm);
    let names = e.vm.space.names.len();
    let err = run(&mut e, p).unwrap_err();
    assert!(matches!(&err, VmError::Script { message, .. } if message.contains("AttachedToSkelComponent cannot be read")), "{err:?}");
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert_eq!(e.vm.space.names.len(), names, "nothing interned");
    assert!(entries(&e, p).is_empty(), "no entry appended");
}
