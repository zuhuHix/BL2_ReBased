//! Synthetic caller visibility regression. AI-assisted (Codex / GPT-6.1 Sol).
use super::*;
use ow_core::ObjectPath;
use ow_defs::store::DefError;
use ow_object::ids::PropAddr;
use ow_object::value::Value;
use ow_package::store::StoreError;
use ow_package::testkit::reflect::Declarations;
use ow_package::testkit::{Export, PackageBuilder, ints};

struct Fixture {
    dir: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "ow-slice-loadset-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        for name in ow_object::reflect::CODE_PACKAGES
            .into_iter()
            .chain(["Startup", "Map_Toy"])
        {
            std::fs::write(
                dir.join(format!("{name}.upk")),
                PackageBuilder::new_empty().build(),
            )
            .unwrap();
        }
        let mut core = Declarations::new(&[]);
        let object = core.cls("Object", 0);
        core.default_object(object, "Object");
        let component = core.cls("Component", object);
        core.default_object(component, "Component");
        std::fs::write(dir.join("Core.upk"), core.build()).unwrap();
        let mut d = Declarations::new(&["Int"]);
        let root = d.p.add_import("Core", "Package", 0, "Core");
        let component = d.p.add_import("Core", "Class", root, "Component");
        let object = d.p.add_import("Core", "Class", root, "Object");
        let part = d.cls("ToyPart", component);
        d.prop("Int", part, "Count", 0);
        let part_default = d.default_object(part, "ToyPart");
        let owner = d.cls("ToyOwner", object);
        let owner_default = d.default_object(owner, "ToyOwner");
        let name = d.p.name("Part");
        let mut payload = ints(&[0, name, 0, 0]);
        payload.extend(d.p.int_tag("Count", 17));
        payload.extend(d.none);
        let mut export = Export::new(part, "Part", payload);
        export.outer = owner_default;
        export.archetype = part_default;
        d.p.add_export_with(export);
        std::fs::write(dir.join("WillowGame.upk"), d.build()).unwrap();
        Self { dir }
    }
    fn set(&self) -> LoadSet {
        load_set(&[self.dir.join("Map_Toy.upk").to_string_lossy().into_owned()]).unwrap()
    }
    fn defs(&self, mode: ParityMode) -> DefStore {
        DefStore::open(self.dir.clone(), IdentityPolicy::Loaded(self.set()), mode).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).unwrap();
    }
}

#[test]
fn off_slice_configures_int_roots_and_reference_has_no_backend() {
    use ow_world::world::localize::Localization;
    let check = |cooked: &str| {
        let off = open(cooked, ParityMode::Off, IdentityPolicy::FirstInScanOrder).unwrap();
        assert_eq!(off.service::<Localization>().unwrap().borrow_mut().get("ToySection", "ToyKey", "ToyText").unwrap(), "third");
        let reference = open(cooked, ParityMode::Reference, IdentityPolicy::FirstInScanOrder).unwrap();
        assert!(reference.service::<Localization>().is_none());
    };
    const CHILD: &str = "OW_TEST_SLICE_LOCALIZE_RELATIVE_CHILD";
    if std::env::var_os(CHILD).as_deref() == Some(std::ffi::OsStr::new("1")) {
        for cooked in [".", "../CookedPCConsole", "unused/.."] {
            check(cooked);
        }
        println!("relative localisation checks completed");
        return;
    }
    let f = Fixture::new();
    let game = f.dir.join("Install");
    let cooked = game.join("WillowGame").join("CookedPCConsole");
    std::fs::create_dir_all(&cooked).unwrap();
    for (root, value) in [("Engine", "first"), ("Gearbox", "second"), ("WillowGame", "third")] {
        let dir = game.join(root).join("Localization").join("INT");
        std::fs::create_dir_all(&dir).unwrap();
        let text = format!("[ToySection]\nToyKey={value}");
        let bytes: Vec<u8> = [0xfeff].into_iter().chain(text.encode_utf16()).flat_map(u16::to_le_bytes).collect();
        std::fs::write(dir.join("ToyText.int"), bytes).unwrap();
    }
    check(cooked.to_str().unwrap());
    // Set cwd only in the child test process; other parallel tests retain their own cwd.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["off_slice_configures_int_roots_and_reference_has_no_backend", "--nocapture"])
        .env(CHILD, "1")
        .current_dir(&cooked)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("relative localisation checks completed"), "child test did not run: {stdout}");
    check(cooked.join("..").join("CookedPCConsole").to_str().unwrap());
}

#[test]
fn caller_exposes_code_template_without_exposing_unrelated_packages() {
    let f = Fixture::new();
    let mut unrelated = PackageBuilder::new_empty();
    unrelated.add_export(0, "OnlyHere", vec![]);
    std::fs::write(f.dir.join("Unrelated.upk"), unrelated.build()).unwrap();
    let set = f.set();
    assert_eq!(
        &set.packages[..2],
        &[
            PackageName::parse("Startup").unwrap(),
            PackageName::parse("Map_Toy").unwrap()
        ]
    );
    assert!(
        set.packages
            .contains(&PackageName::parse("WillowGame").unwrap())
    );
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut defs = f.defs(mode);
        let mut w = ObjectSpace::new();
        assert!(
            defs.find(&mut w, &ObjectPath::parse("Unrelated.OnlyHere").unwrap())
                .unwrap()
                .is_none()
        );
        assert!(
            defs.find(&mut w, &ObjectPath::parse("WillowGame.Absent").unwrap())
                .unwrap()
                .is_none()
        );
        let got = defs.find(
            &mut w,
            &ObjectPath::parse("WillowGame.Default__ToyOwner.Part").unwrap(),
        );
        if mode == ParityMode::Off {
            let object = got.unwrap().unwrap();
            defs.load(&mut w, object).unwrap();
            let prop = defs.key(&mut w, "WillowGame.ToyPart", "Count").unwrap();
            assert_eq!(
                w.read(object, PropAddr::scalar(prop)).unwrap(),
                &Value::Int(17)
            );
        } else {
            assert!(matches!(got, Err(DefError::NotADefinition(_))), "{got:?}");
        }
    }
}

#[test]
fn required_code_holder_errors_keep_their_types() {
    for kind in ["missing", "ambiguous", "corrupt"] {
        let f = Fixture::new();
        match kind {
            "missing" => std::fs::remove_file(f.dir.join("WillowGame.upk")).unwrap(),
            "ambiguous" => {
                std::fs::create_dir(f.dir.join("Duplicate")).unwrap();
                std::fs::copy(
                    f.dir.join("WillowGame.upk"),
                    f.dir.join("Duplicate/WillowGame.upk"),
                )
                .unwrap();
            }
            _ => std::fs::write(f.dir.join("WillowGame.upk"), b"broken").unwrap(),
        }
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let mut defs = f.defs(mode);
            let result = defs.find(
                &mut ObjectSpace::new(),
                &ObjectPath::parse("WillowGame.Absent").unwrap(),
            );
            match kind {
                "missing" => assert!(
                    matches!(result, Err(DefError::Store(StoreError::PackageNotFound(_)))),
                    "{result:?}"
                ),
                "ambiguous" => assert!(
                    matches!(result, Err(DefError::Store(StoreError::AmbiguousName(_)))),
                    "{result:?}"
                ),
                _ => assert!(
                    matches!(result, Err(DefError::Store(StoreError::Package(_)))),
                    "{result:?}"
                ),
            }
        }
    }
}
