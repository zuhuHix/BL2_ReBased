//! Tests of `WillowPawn.InitializeBehaviorProviders` (NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 2.5) on
//! invented data (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! The pawn, body class, hit regions, stances and the two definition holders are toy classes declared here (invented
//! fields on the game's class paths where the rule tests a class by name); the provider definitions come from the
//! kernel fixtures' invented packages.

use super::*;

/// The pawn's classes and struct types.
struct Pawns {
    t: Toy,
    pawn: ClassId,
    ai: ClassId,
    body: ClassId,
    region: ClassId,
    other_region: ClassId,
    holder: ClassId,
    instance: StructId,
    entry: StructId,
    component: StructId,
    replicated: StructId,
}

/// The write counters, the class-code loads and the name table's size.
fn counters(vm: &Vm) -> (u64, u64, u64, u64, usize) {
    (
        vm.space.heap.committed_writes(),
        vm.rng.draws(),
        vm.defs.load_transitions(),
        vm.class_code_loads(),
        vm.space.names.len(),
    )
}

fn pawns(mode: ParityMode, declare: bool, pack: bool) -> Pawns {
    let mut t = toy(mode);
    let bare = t.bare;
    let handle = t.handle;
    let o = PropKind::Object { class: bare };
    let n = |t: &mut Toy, s: &str| t.vm.space.names.intern(s);
    let f = |t: &mut Toy, s: &str, k: PropKind| PropertyDecl::new(n(t, s), k);
    let hit = f(&mut t, "HitRegionDefinition", o.clone());
    let component = t.vm.space.classes.define_struct("WillowGame.ToyComponentData", None, vec![hit]).unwrap();
    let cd = f(&mut t, "ComponentData", PropKind::Struct(component));
    let entry = t.vm.space.classes.define_struct("WillowGame.InstanceDataUnion", None, vec![cd]).unwrap();
    let data = f(&mut t, "Data", PropKind::Array(Box::new(PropKind::Struct(entry))));
    let instance = t.vm.space.classes.define_struct("WillowGame.ToyInstanceDataState", None, vec![data]).unwrap();
    let flag = f(&mut t, "bNeedToApplyThisState", PropKind::Bool);
    let replicated = t.vm.space.classes.define_struct("WillowGame.ToyReplicatedState", None, vec![flag]).unwrap();
    let fields = vec![
        f(&mut t, "ConsumerHandle", PropKind::Struct(handle)),
        f(&mut t, "BodyClass", o.clone()),
        f(&mut t, "InstanceDataState", PropKind::Struct(instance)),
        f(&mut t, "ReplicatedBehaviorConsumerState", PropKind::Struct(replicated)),
    ];
    let pawn = t.vm.space.classes.define("WillowGame.WillowPawn", None, fields).unwrap();
    let ai = t.vm.space.classes.define("WillowGame.WillowAIPawn", Some(pawn), vec![]).unwrap();
    let bpd = f(&mut t, "BehaviorProviderDefinition", o.clone());
    let holder = t.vm.space.classes.define("WillowGame.ToyDefinitionHolder", None, vec![bpd.clone()]).unwrap();
    let base = t.vm.space.classes.define("WillowGame.HitRegionDefinition", None, vec![bpd]).unwrap();
    let death = f(&mut t, "OverrideDeathDef", o.clone());
    let region = t.vm.space.classes.define("WillowGame.BodyHitRegionDefinition", Some(base), vec![death]).unwrap();
    let other_region = t.vm.space.classes.define("WillowGame.ToyOtherHitRegion", Some(base), vec![]).unwrap();
    let list = || PropKind::Array(Box::new(PropKind::Object { class: bare }));
    let body_fields = vec![
        f(&mut t, "ScrapedStances", list()),
        f(&mut t, "DefaultHitRegion", o.clone()),
        f(&mut t, "HitRegionList", list()),
        f(&mut t, "DeathDef", o.clone()),
        f(&mut t, "PlayerEventDef", o),
    ];
    let body = t.vm.space.classes.define("WillowGame.ToyBodyClass", None, body_fields).unwrap();
    if declare {
        let decl = FunctionDecl::native(
            ObjectPath::parse(INITIALIZE_BEHAVIOR_PROVIDERS).unwrap(),
            INITIALIZE_BEHAVIOR_PROVIDERS,
        )
        .with_owner(pawn);
        t.vm.declare_function(decl).expect("declared");
    }
    if pack {
        Pack.register(&mut t.vm).unwrap();
    }
    Pawns { t, pawn, ai, body, region, other_region, holder, instance, entry, component, replicated }
}

impl Pawns {
    fn obj(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.t.object(class, name)
    }

    /// Sets `o.name` to `v` with no write counted (set-up only).
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.t.vm.space.heap.get(o).unwrap().class;
        let p = self.t.vm.space.prop(class, name).unwrap();
        *self.t.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.t.vm.space.heap.get(o).unwrap().class;
        let p = self.t.vm.space.prop(class, name).unwrap();
        self.t.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// A pawn of `class` with `PID` -1, its body class `body`, no instance data and the flag clear.
    fn pawn_of(&mut self, class: ClassId, body: Option<ObjRef>) -> ObjRef {
        let p = self.obj(class, "Maya");
        let h = self.t.handle(INVALID_PID);
        self.set(p, "ConsumerHandle", h);
        self.set(p, "BodyClass", Value::Object(body));
        self.set_instances(p, &[]);
        self.set_flag(p, false);
        p
    }

    fn pawn(&mut self, body: Option<ObjRef>) -> ObjRef {
        self.pawn_of(self.pawn, body)
    }

    fn set_instances(&mut self, pawn: ObjRef, regions: &[Option<ObjRef>]) {
        let entries = regions
            .iter()
            .map(|r| {
                Value::structure(self.entry, vec![Value::structure(self.component, vec![Value::Object(*r)])])
            })
            .collect();
        let v = Value::structure(self.instance, vec![Value::array(entries)]);
        self.set(pawn, "InstanceDataState", v);
    }

    fn set_flag(&mut self, pawn: ObjRef, on: bool) {
        let v = Value::structure(self.replicated, vec![Value::Bool(on)]);
        self.set(pawn, "ReplicatedBehaviorConsumerState", v);
    }

    /// An object of `class` whose `BehaviorProviderDefinition` is `definition`.
    fn holding(&mut self, class: ClassId, name: &str, definition: Option<ObjRef>) -> ObjRef {
        let o = self.obj(class, name);
        self.set(o, "BehaviorProviderDefinition", Value::Object(definition));
        o
    }

    fn body_class(&mut self) -> ObjRef {
        let b = self.obj(self.body, "BodyClass_Toy");
        for list in ["ScrapedStances", "HitRegionList"] {
            self.set(b, list, Value::array(vec![]));
        }
        b
    }

    fn list(&mut self, o: ObjRef, name: &str, items: &[Option<ObjRef>]) {
        self.set(o, name, Value::array(items.iter().map(|i| Value::Object(*i)).collect()));
    }

    fn def(&mut self, file: &str, path: &str) -> ObjRef {
        self.t.definition(file, path)
    }

    fn run(&mut self, pawn: ObjRef) -> Result<Value, VmError> {
        self.t.call(INITIALIZE_BEHAVIOR_PROVIDERS, Some(pawn), vec![])
    }

    fn pid(&self, pawn: ObjRef) -> i32 {
        let h = self.get(pawn, "ConsumerHandle");
        self.t.pid_of(&h)
    }

    /// The stock paths of the providers on the pawn's kernel process.
    fn providers(&self, pawn: ObjRef) -> Vec<String> {
        let pid = process_of(&self.t.vm, pawn).unwrap().expect("a kernel process");
        provider_paths(&self.t.vm, pid).unwrap().unwrap()
    }

    fn path(&self, d: ObjRef) -> String {
        stock_path(&self.t.vm, d).unwrap()
    }

    /// Runs the native on `pawn`, expecting a `NotYet` with nothing counted and no process made; answers its text.
    fn refused(&mut self, pawn: ObjRef, missing: Missing) -> String {
        let before = counters(&self.t.vm);
        let text = match self.run(pawn) {
            Err(VmError::NotYet(n)) => {
                assert_eq!(n.missing, missing, "{n}");
                n.to_string()
            }
            other => panic!("expected NotYet, got {other:?}"),
        };
        assert_eq!(counters(&self.t.vm), before, "{text}: nothing counted");
        assert!(!ow_core::notyet::is_partial_writes(&text), "{text}");
        assert_eq!(self.pid(pawn), INVALID_PID, "{text}: the handle is unchanged");
        assert_eq!(process_of(&self.t.vm, pawn).unwrap(), None, "{text}: no process");
        text
    }
}

/// `NotYet::layer_absent`'s kind, whatever the layer.
fn is_layer_absent(m: &Missing) -> bool {
    matches!(m, Missing::LayerAbsent(_))
}


// 1.
#[test]
fn a_handle_other_than_minus_one_does_nothing() {
    let mut x = pawns(ParityMode::Off, true, true);
    let p = x.pawn(None);
    let h = x.t.handle(5);
    x.set(p, "ConsumerHandle", h);
    let before = counters(&x.t.vm);
    assert_eq!(x.run(p).unwrap(), Value::None);
    assert_eq!(counters(&x.t.vm), before);
    assert_eq!(x.pid(p), 5);
    assert_eq!(process_of(&x.t.vm, p).unwrap(), None);
}

// 2.
#[test]
fn no_kernel_service_is_the_kernel_layer_absent() {
    let mut x = pawns(ParityMode::Off, true, false);
    let f = x.t.vm.find_function(INITIALIZE_BEHAVIOR_PROVIDERS).unwrap();
    x.t.vm.register_native_for(f, Rc::new(super::super::pawn_providers::native)).unwrap();
    let p = x.pawn(None);
    let before = counters(&x.t.vm);
    match x.run(p) {
        Err(VmError::NotYet(n)) => {
            assert!(is_layer_absent(&n.missing) && n.what.contains("the behavior kernel natives"), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(counters(&x.t.vm), before);
    assert_eq!(x.pid(p), INVALID_PID);
}

// 3.
#[test]
fn no_body_class_registers_a_process_with_no_provider() {
    let mut x = pawns(ParityMode::Off, true, true);
    let p = x.pawn(None);
    assert_eq!(x.run(p).unwrap(), Value::None);
    assert_eq!(x.pid(p), 0, "the lowest free PID");
    assert_eq!(process_of(&x.t.vm, p).unwrap(), Some(0));
    assert_eq!(owner_of(&x.t.vm, 0).unwrap(), Some(Owner::Kernel));
    assert!(x.providers(p).is_empty());
}

// 4.
#[test]
fn a_definition_with_no_sequences_is_not_registered() {
    let mut x = pawns(ParityMode::Off, true, true);
    let empty = x.def("TestCreateEmpty", CREATE_EMPTY);
    let death = x.holding(x.holder, "Death_Toy", Some(empty));
    let b = x.body_class();
    x.set(b, "DeathDef", Value::Object(Some(death)));
    let p = x.pawn(Some(b));
    x.run(p).unwrap();
    assert_eq!(x.pid(p), 0);
    assert!(x.providers(p).is_empty());
}

// 5.
#[test]
fn a_stance_with_one_enabled_sequence_registers_and_runs_pass_one() {
    let mut x = pawns(ParityMode::Off, true, true);
    let ok = x.def("TestPassOk", PASS_OK);
    let stance = x.holding(x.holder, "Stance_Crouch", Some(ok));
    let none = x.holding(x.holder, "Stance_None", None);
    let b = x.body_class();
    x.list(b, "ScrapedStances", &[None, Some(none), Some(stance)]);
    let p = x.pawn(Some(b));
    x.run(p).unwrap();
    assert_eq!(x.providers(p), vec![x.path(ok)]);
    assert_eq!(x.t.seen(), vec!["OnBehaviorSequenceEnabled PO consumer=Maya".to_string()]);
}

// 6, and Maya's shape (section 2.3): the conditioned events definition refuses with nothing written.
#[test]
fn a_conditioned_player_event_definition_with_an_unloaded_condition_is_refused() {
    let mut x = pawns(ParityMode::Off, true, true);
    let cond = x.def("TestCond", CONDITIONAL);
    let ok = x.def("TestPassOk", PASS_OK);
    let empty = x.def("TestCreateEmpty", CREATE_EMPTY);
    let b = x.body_class();
    let mut stances = Vec::new();
    for i in 0..4 {
        stances.push(Some(x.holding(x.holder, &format!("Stance_{i}"), Some(empty))));
    }
    stances.push(Some(x.holding(x.holder, "Stance_Crouch", Some(ok))));
    x.list(b, "ScrapedStances", &stances);
    let death = x.holding(x.holder, "Death_Toy", Some(empty));
    let events = x.holding(x.holder, "PlayerEvents_Toy", Some(cond));
    x.set(b, "DeathDef", Value::Object(Some(death)));
    x.set(b, "PlayerEventDef", Value::Object(Some(events)));
    let default = x.holding(x.region, "HitRegion_Body", Some(empty));
    x.set(b, "DefaultHitRegion", Value::Object(Some(default)));
    let p = x.pawn(Some(b));
    // Pass 2's plan (lane IMPL-BEHAVIOR-KERNEL-PASS2) replaces the pass-2 LayerAbsent: its step 1, the condition a
    // store handle not loaded yet (the preflight loads nothing), is the not-loaded NotRead, before any write.
    let text = x.refused(p, Missing::NotRead);
    assert!(text.contains("a CustomEnableCondition the store has not loaded"), "{text}");
    assert!(x.t.seen().is_empty(), "pass 1 never ran");
}

// 7.
#[test]
fn a_default_hit_region_in_the_list_is_the_repeat() {
    let mut x = pawns(ParityMode::Off, true, true);
    let ok = x.def("TestPassOk", PASS_OK);
    let region = x.holding(x.region, "HitRegion_Body", Some(ok));
    let b = x.body_class();
    x.set(b, "DefaultHitRegion", Value::Object(Some(region)));
    x.list(b, "HitRegionList", &[Some(region)]);
    let p = x.pawn(Some(b));
    let text = x.refused(p, Missing::NotRead);
    assert!(text.contains("a provider registered twice on one consumer"), "{text}");
}

// 8.
#[test]
fn a_default_hit_region_with_an_empty_list_is_not_registered() {
    let mut x = pawns(ParityMode::Off, true, true);
    let ok = x.def("TestPassOk", PASS_OK);
    let region = x.holding(x.region, "HitRegion_Body", Some(ok));
    let b = x.body_class();
    x.set(b, "DefaultHitRegion", Value::Object(Some(region)));
    let p = x.pawn(Some(b));
    x.run(p).unwrap();
    assert!(x.providers(p).is_empty());
    // A default not in a non-empty list is not handed over either.
    let mut y = pawns(ParityMode::Off, true, true);
    let ok = y.def("TestPassOk", PASS_OK);
    let delay = y.def("TestDelay", DELAYED);
    let default = y.holding(y.region, "HitRegion_Body", Some(ok));
    let other = y.holding(y.region, "HitRegion_Head", Some(delay));
    let b = y.body_class();
    y.set(b, "DefaultHitRegion", Value::Object(Some(default)));
    y.list(b, "HitRegionList", &[Some(other)]);
    let p = y.pawn(Some(b));
    y.run(p).unwrap();
    assert_eq!(y.providers(p), vec![y.path(delay)]);
}

// 9.
#[test]
fn a_hit_region_entry_then_its_override_death_definition() {
    let mut x = pawns(ParityMode::Off, true, true);
    let ok = x.def("TestPassOk", PASS_OK);
    let delay = x.def("TestDelay", DELAYED);
    let death = x.holding(x.holder, "Death_Override", Some(delay));
    let region = x.holding(x.region, "HitRegion_Head", Some(ok));
    x.set(region, "OverrideDeathDef", Value::Object(Some(death)));
    let b = x.body_class();
    x.list(b, "HitRegionList", &[None, Some(region)]);
    let p = x.pawn(Some(b));
    x.run(p).unwrap();
    assert_eq!(x.providers(p), vec![x.path(ok), x.path(delay)]);
}

// 10.
#[test]
fn instance_data_hit_regions_register_only_body_hit_regions() {
    let mut x = pawns(ParityMode::Off, true, true);
    let ok = x.def("TestPassOk", PASS_OK);
    let delay = x.def("TestDelay", DELAYED);
    let body_region = x.holding(x.region, "HitRegion_Attachment", Some(ok));
    let other = x.holding(x.other_region, "HitRegion_Other", Some(delay));
    let b = x.body_class();
    let p = x.pawn(Some(b));
    x.set_instances(p, &[None, Some(other), Some(body_region)]);
    x.run(p).unwrap();
    assert_eq!(x.providers(p), vec![x.path(ok)]);
}

// 11.
#[test]
fn the_replicated_state_flag_is_not_read() {
    let mut x = pawns(ParityMode::Off, true, true);
    let p = x.pawn(None);
    x.set_flag(p, true);
    let text = x.refused(p, Missing::NotRead);
    assert!(text.contains(APPLY_REPLICATED), "{text}");
}

// 12.
#[test]
fn a_stance_the_store_has_not_loaded_is_refused_with_no_load() {
    let mut x = pawns(ParityMode::Off, true, true);
    let cond = x.def("TestCond", CONDITIONAL);
    // The conditional provider names its package's `Condition` export, which stays an unloaded store handle.
    let seqs = x.get(cond, "BehaviorSequences");
    let first = &seqs.elements().expect("sequences")[0];
    let lazy = match crate::provider::field(&x.t.vm.space, first, "CustomEnableCondition") {
        Some(Value::Object(Some(o))) => *o,
        other => panic!("{other:?}"),
    };
    assert!(x.t.vm.defs.is_unloaded(&x.t.vm.space, lazy));
    let b = x.body_class();
    x.list(b, "ScrapedStances", &[Some(lazy)]);
    let p = x.pawn(Some(b));
    let text = x.refused(p, Missing::NotRead);
    assert!(text.contains(PROVIDERS_NOT_LOADED), "{text}");
    assert!(x.t.vm.defs.is_unloaded(&x.t.vm.space, lazy), "still unloaded");
}

// 13.
#[test]
fn the_providers_come_in_the_list_order() {
    let mut x = pawns(ParityMode::Off, true, true);
    let [a, b_, c, d] = [
        x.def("TestPassOk", PASS_OK),
        x.def("TestDelay", DELAYED),
        x.def("TestBpd", PROVIDER),
        x.def("TestCreateB", CREATE_B),
    ];
    let region = x.holding(x.region, "HitRegion_Head", Some(a));
    let stance = x.holding(x.holder, "Stance_Run", Some(b_));
    let death = x.holding(x.holder, "Death_Toy", Some(c));
    let events = x.holding(x.holder, "PlayerEvents_Toy", Some(d));
    let body = x.body_class();
    x.list(body, "HitRegionList", &[Some(region)]);
    x.list(body, "ScrapedStances", &[Some(stance)]);
    x.set(body, "DeathDef", Value::Object(Some(death)));
    x.set(body, "PlayerEventDef", Value::Object(Some(events)));
    let p = x.pawn(Some(body));
    x.run(p).unwrap();
    assert_eq!(x.providers(p), vec![x.path(a), x.path(b_), x.path(c), x.path(d)]);
}

// 14.
#[test]
fn a_later_pass_one_refusal_carries_the_partial_writes_prefix() {
    let mut x = pawns(ParityMode::Off, true, true);
    let ok = x.def("TestPassOk", PASS_OK);
    let refuse = x.def("TestPassRefuse", PASS_REFUSE);
    let stance = x.holding(x.holder, "Stance_Crouch", Some(ok));
    let death = x.holding(x.holder, "Death_Toy", Some(refuse));
    let b = x.body_class();
    x.list(b, "ScrapedStances", &[Some(stance)]);
    x.set(b, "DeathDef", Value::Object(Some(death)));
    let p = x.pawn(Some(b));
    match x.run(p) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert!(ow_core::notyet::is_partial_writes(&n.what), "{n}");
            assert!(n.what.contains("an invented OnCreate behavior"), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.pid(p), 0, "the registration stays");
    assert_eq!(x.providers(p), vec![x.path(ok)], "the refused provider is not kept");
}

// 15.
#[test]
fn an_ai_pawn_is_the_ai_body_not_read() {
    let mut x = pawns(ParityMode::Off, true, true);
    let p = x.pawn_of(x.ai, None);
    let text = x.refused(p, Missing::NotRead);
    assert!(text.contains(AI_PAWN), "{text}");
}

// 16.
#[test]
fn preflight_seven_takes_the_first_definition_that_refuses() {
    // A conditioned definition first, then a repeat of another: the condition.
    let mut x = pawns(ParityMode::Off, true, true);
    let cond = x.def("TestCond", CONDITIONAL);
    let ok = x.def("TestPassOk", PASS_OK);
    let s1 = x.holding(x.holder, "Stance_A", Some(cond));
    let s2 = x.holding(x.holder, "Stance_B", Some(ok));
    let s3 = x.holding(x.holder, "Stance_C", Some(ok));
    let b = x.body_class();
    x.list(b, "ScrapedStances", &[Some(s1), Some(s2), Some(s3)]);
    let p = x.pawn(Some(b));
    let text = x.refused(p, Missing::NotRead);
    assert!(text.contains("a CustomEnableCondition the store has not loaded"), "{text}");
    // A repeat first, then a conditioned definition: the repeat.
    let mut y = pawns(ParityMode::Off, true, true);
    let cond = y.def("TestCond", CONDITIONAL);
    let ok = y.def("TestPassOk", PASS_OK);
    let s1 = y.holding(y.holder, "Stance_A", Some(ok));
    let s2 = y.holding(y.holder, "Stance_B", Some(ok));
    let s3 = y.holding(y.holder, "Stance_C", Some(cond));
    let b = y.body_class();
    y.list(b, "ScrapedStances", &[Some(s1), Some(s2), Some(s3)]);
    let p = y.pawn(Some(b));
    let text = y.refused(p, Missing::NotRead);
    assert!(text.contains("a provider registered twice on one consumer"), "{text}");
}

// Preflight 5: a pawn the kernel already holds is RegisterBehaviorConsumer's refusal, before any write.
#[test]
fn a_pawn_the_kernel_already_registered_is_refused() {
    let mut x = pawns(ParityMode::Off, true, true);
    let p = x.pawn(None);
    x.t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(p))]).unwrap();
    let before = counters(&x.t.vm);
    match x.run(p) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains("RegisterBehaviorConsumer of a consumer the kernel already registered"), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(counters(&x.t.vm), before);
}

// Preflight 2: a pawn class missing a declaration is a data error naming the field, before any write.
#[test]
fn a_missing_declaration_is_a_script_error() {
    let mut x = pawns(ParityMode::Off, true, true);
    let handle = x.t.handle;
    let field = PropertyDecl::new(x.t.vm.space.names.intern("ConsumerHandle"), PropKind::Struct(handle));
    let thin = x.t.vm.space.classes.define("WillowGame.ToyThinPawn", Some(x.t.bare), vec![field]).unwrap();
    let p = x.obj(thin, "Thin");
    let h = x.t.handle(INVALID_PID);
    x.set(p, "ConsumerHandle", h);
    let f = x.t.vm.find_function(INITIALIZE_BEHAVIOR_PROVIDERS).unwrap();
    let before = counters(&x.t.vm);
    match x.t.vm.call(f, Some(p), vec![]) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("declares no BodyClass"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(counters(&x.t.vm), before);
}

// Reference binds nothing: the stub policy answers.
#[test]
fn reference_leaves_the_native_unbound() {
    let mut x = pawns(ParityMode::Reference, true, true);
    let p = x.pawn(None);
    assert_eq!(x.run(p).unwrap(), Value::None);
    assert!(x.t.vm.log.iter().any(|l| l.starts_with("UNIMPLEMENTED")), "{:?}", x.t.vm.log);
    assert_eq!(x.pid(p), INVALID_PID);
    assert_eq!(process_of(&x.t.vm, p).unwrap(), None);
}
