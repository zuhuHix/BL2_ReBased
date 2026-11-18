//! Invented packages and temporary UTF-16 text only. AI-assisted (Codex/GPT-6.1 Sol).
use super::*;
use ow_attr::AttributeSystem;
use ow_defs::{DefStore, identity::IdentityPolicy};
use ow_object::heap::ObjectSpace;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use std::sync::atomic::{AtomicU32, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ow-localize-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn roots(&self) -> Vec<PathBuf> {
        ["Engine", "Gearbox", "WillowGame"]
            .map(|s| self.0.join(s).join("Localization"))
            .to_vec()
    }
    fn file(&self, root: usize, name: &str, text: &str) -> PathBuf {
        let dir = self.roots()[root].join("INT");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let bytes: Vec<u8> = [0xfeff]
            .into_iter()
            .chain(text.encode_utf16())
            .flat_map(u16::to_le_bytes)
            .collect();
        std::fs::write(&path, bytes).unwrap();
        path
    }
    fn backend(&self) -> Localization {
        Localization::ready(self.roots(), "INT", false).unwrap()
    }
    fn vm(&self, mode: ParityMode) -> Vm {
        let mut t = ScriptToy::with_imports(&["Class", "Function", "StrProperty"]);
        let object = t.class("Object", 0);
        t.function(
            object,
            "Localize",
            &[
                Param::new("Str", "Section"),
                Param::new("Str", "Key"),
                Param::new("Str", "Package"),
            ],
            Some(("Str", 0)),
            &[],
            &FunctionSpec::new(0x400 | 0x2000 | 0x20000, "Localize"),
            |_| None,
        );
        let bytes = t.p.build();
        let defs = DefStore::from_synthetic(
            &self.0.join("cooked"),
            &[("Core.upk", &bytes)],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        );
        // The actual world pack must install the binding; no direct call of the Rust closure.
        use ow_vm::api::NativePack;
        crate::world::Pack.register(&mut vm).unwrap();
        vm
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the directory this fixture created, under the system temp directory.
        assert!(self.0.starts_with(std::env::temp_dir()));
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn call(vm: &mut Vm, key: &str) -> Result<Value, VmError> {
    let f = vm.find_function("Core.Object.Localize")?;
    Ok(vm
        .call(
            f,
            None,
            vec![
                Value::string("ToySection"),
                Value::string(key),
                Value::string("ToyText"),
            ],
        )?
        .ret)
}

#[test]
fn ordered_roots_case_quotes_spaces_unicode_and_cache() {
    let f = Fixture::new();
    f.file(
        0,
        "ToyText.int",
        "[ToySection]\nShared=engine\nEngineOnly=base\n",
    );
    f.file(
        1,
        "TOYTEXT.INT",
        "[toysection]\nShared=gearbox\nGearboxOnly=middle\n",
    );
    let path = f.file(2, "tOyTeXt.InT", "  [TOYSECTION]  \r\n sHaReD = \"  café 雪 😀  \" \r\nEmpty=\"\"\nSpace=\": \"\nPlain = word = value  \n");
    let mut b = f.backend();
    assert_eq!(
        b.get("toysection", "shared", "toytext").unwrap(),
        "  café 雪 😀  "
    );
    assert_eq!(
        b.get("ToySection", "EngineOnly", "ToyText").unwrap(),
        "base"
    );
    assert_eq!(
        b.get("ToySection", "GearboxOnly", "ToyText").unwrap(),
        "middle"
    );
    assert_eq!(b.get("ToySection", "Space", "ToyText").unwrap(), ": ");
    assert_eq!(b.get("ToySection", "Empty", "ToyText").unwrap(), "");
    assert_eq!(
        b.get("ToySection", "Plain", "ToyText").unwrap(),
        "word = value"
    );
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        b.get("ToySection", "Shared", "TOYTEXT").unwrap(),
        "  café 雪 😀  "
    );
}

#[test]
fn extra_configured_root_keeps_its_priority() {
    let f = Fixture::new();
    f.file(2, "ToyText.int", "[ToySection]\nKey=third\n");
    let mut roots = f.roots();
    let extra = f.0.join("Extra");
    std::fs::create_dir_all(extra.join("INT")).unwrap();
    // An explicit big-endian BOM; this file is independent of the LE fixtures.
    let mut bytes = vec![0xfe, 0xff];
    bytes.extend(
        "[ToySection]\nKey=extra"
            .encode_utf16()
            .flat_map(u16::to_be_bytes),
    );
    std::fs::write(extra.join("INT/ToyText.int"), bytes).unwrap();
    roots.push(extra);
    assert_eq!(
        Localization::ready(roots, "int", false)
            .unwrap()
            .get("ToySection", "Key", "ToyText")
            .unwrap(),
        "extra"
    );
}

#[test]
fn missing_files_sections_keys_and_final_marker() {
    let f = Fixture::new();
    f.file(
        0,
        "ToyText.int",
        "[ToySection]\nKey=base###discard###tail\n",
    );
    f.file(2, "ToyText.int", "[Elsewhere]\nKey=other\n");
    let mut b = f.backend();
    assert_eq!(b.get("ToySection", "Key", "ToyText").unwrap(), "base");
    assert_eq!(
        b.get("Missing", "Key", "ToyText").unwrap(),
        "?INT?ToyText.Missing.Key?"
    );
    assert_eq!(
        b.get("ToySection", "Absent", "NoFile").unwrap(),
        "?INT?NoFile.ToySection.Absent?"
    );
    assert_eq!(
        b.get("ToySection", "Absent###tail", "NoFile").unwrap(),
        "?INT?NoFile.ToySection.Absent"
    );
    assert_eq!(
        Localization::not_ready()
            .get("unused", "Key###tail", "../unused")
            .unwrap(),
        "Key"
    );
}

#[test]
fn unread_selected_syntax_is_typed_and_unused_entries_are_allowed() {
    let f = Fixture::new();
    f.file(2, "ToyText.int", "[ToySection]\nGood=usable\nEscape=one\\ntwo\nArray[0]=x\nQuoted=\"one\\\"two\"\n[Unused]\nUnknown[0]=\\q\n");
    let mut b = f.backend();
    assert_eq!(b.get("ToySection", "Good", "ToyText").unwrap(), "usable");
    for key in ["Escape", "Array", "Array[0]", "Quoted"] {
        assert!(
            matches!(b.get("ToySection", key, "ToyText"), Err(VmError::NotYet(_))),
            "{key}"
        );
    }
    assert!(matches!(
        Localization::ready(f.roots(), "FRA", false),
        Err(VmError::NotYet(_))
    ));
    assert!(matches!(
        Localization::ready(f.roots(), "", false),
        Err(VmError::NotYet(_))
    ));
    assert!(matches!(
        Localization::ready(f.roots(), "INT", true),
        Err(VmError::NotYet(_))
    ));
}

#[test]
fn corrupt_or_ambiguous_selected_data_does_not_fall_back() {
    let f = Fixture::new();
    f.file(0, "ToyText.int", "[ToySection]\nKey=fallback\n");
    let path = f.file(2, "ToyText.int", "");
    for text in [
        "[ToySection]\nKey=one\nKEY=two",
        "[ToySection]\nKey=\"unclosed",
        "[ToySection\nKey=bad",
        "[ToySection]\nKey",
    ] {
        f.file(2, "ToyText.int", text);
        assert!(matches!(
            f.backend().get("ToySection", "Key", "ToyText"),
            Err(VmError::Script { .. })
        ));
    }
    for bytes in [
        vec![0xff, 0xfe, 1],
        vec![0xff, 0xfe, 0, 0xd8],
        vec![0xff, 0xfe, 0, 0],
    ] {
        std::fs::write(&path, bytes).unwrap();
        let error = f.backend().get("ToySection", "Key", "ToyText").unwrap_err();
        assert!(matches!(error, VmError::Script { .. }));
        assert!(error.to_string().contains(&path.display().to_string()));
    }
}

#[test]
fn io_and_size_failures_keep_the_cause_and_never_use_lower_root() {
    let f = Fixture::new();
    f.file(0, "ToyText.int", "[ToySection]\nKey=fallback");
    let path = f.roots()[2].join("INT").join("ToyText.int");
    std::fs::create_dir_all(&path).unwrap();
    let error = f.backend().get("ToySection", "Key", "ToyText").unwrap_err();
    assert!(matches!(error, VmError::Script { .. }));
    assert!(error.to_string().contains(&path.display().to_string()));
    assert!(
        error
            .to_string()
            .contains(&std::fs::read(&path).unwrap_err().to_string())
    );
    std::fs::remove_dir(&path).unwrap();
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_FILE_BYTES + 1).unwrap();
    drop(file);
    assert!(matches!(
        f.backend().get("ToySection", "Key", "ToyText"),
        Err(VmError::NotYet(_))
    ));
}

#[test]
fn package_paths_are_refused_before_missing_file_fallback() {
    let f = Fixture::new();
    let mut b = f.backend();
    for package in [
        "",
        "../ToyText",
        "..\\ToyText",
        "/ToyText",
        "C:\\ToyText",
        "ToyText.int",
        "Toy*",
        "Toy?",
        "Toy ",
        "Toy\0",
        "CON",
        "com1",
        "LPT9",
    ] {
        assert!(
            matches!(
                b.get("ToySection", "Key", package),
                Err(VmError::Script { .. })
            ),
            "invalid package path accepted"
        );
    }
}

#[test]
fn actual_world_native_returns_text_missing_backend_is_not_not_ready() {
    let f = Fixture::new();
    f.file(2, "ToyText.int", "[ToySection]\nKey=bound###ignored");
    let mut vm = f.vm(ParityMode::Off);
    assert!(matches!(call(&mut vm, "Key"), Err(VmError::NotYet(_))));
    vm.add_service(f.backend()).unwrap();
    assert_eq!(call(&mut vm, "Key").unwrap(), Value::string("bound"));
    assert_eq!(
        call(&mut vm, "Absent").unwrap(),
        Value::string("?INT?ToyText.ToySection.Absent?")
    );
    let mut vm = f.vm(ParityMode::Off);
    vm.add_service(Localization::not_ready()).unwrap();
    assert_eq!(call(&mut vm, "Key").unwrap(), Value::string("Key"));
}

#[test]
fn reference_pack_leaves_existing_stub_behavior_and_binding_alone() {
    let f = Fixture::new();
    let mut vm = f.vm(ParityMode::Reference);
    assert!(matches!(
        call(&mut vm, "Key"),
        Err(VmError::UnboundNative(_))
    ));
    vm.stub_policy = ow_vm::api::StubPolicy::LogAndZero;
    assert_eq!(call(&mut vm, "Key").unwrap(), Value::string(""));
    vm.register_native(KEY, Rc::new(|_| Ok(Value::string("old binding"))))
        .unwrap();
    register(&mut vm).unwrap();
    vm.add_service(f.backend()).unwrap();
    assert_eq!(call(&mut vm, "Key").unwrap(), Value::string("old binding"));
}

#[test]
#[ignore = "reads only the user's installed UTF-16 INT files; prints counts, never localized payload"]
fn installed_int_lookups() {
    let game =
        PathBuf::from(std::env::var_os("OPENWILLOW_BL2").expect("installed game path required"));
    let f = Fixture::new();
    let mut vm = f.vm(ParityMode::Off);
    vm.add_service(
        Localization::ready(
            ["Engine", "Gearbox", "WillowGame"]
                .map(|s| game.join(s).join("Localization"))
                .to_vec(),
            "INT",
            false,
        )
        .unwrap(),
    )
    .unwrap();
    let function = vm.find_function("Core.Object.Localize").unwrap();
    let mut hits = 0;
    for (section, key) in [
        ("Missions", "ColonAndSpace"),
        ("Missions", "SingleCountObjectiveComplete"),
        ("HUDWidget_Missions", "MissionTurnInString"),
    ] {
        let value = vm
            .call(
                function,
                None,
                vec![
                    Value::string(section),
                    Value::string(key),
                    Value::string("WillowGame"),
                ],
            )
            .unwrap()
            .ret;
        let Value::Str(text) = value else {
            panic!("native did not return string")
        };
        assert!(
            !text.is_empty() && !text.starts_with("?INT?"),
            "installed lookup missed"
        );
        hits += 1;
    }
    eprintln!(
        "installed INT native lookups: {hits}/3 hits; payload omitted; runtime parity UNVERIFIED"
    );
}
