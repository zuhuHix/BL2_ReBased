//! Lane W7-WORLDLOGIC: the installed Sanctuary's vending machines, interactive objects and travel stations, read
//! headless. AI-assisted (Claude Code). Ignored: it needs the installed game, which the repository does not ship.
//!
//! Run: `cargo test -p ow-mission --test w7_worldlogic_installed -- --ignored --nocapture` (`OW_GAME` names the
//! install; the default is the Steam folder). It reads the packages read-only and writes game data only under the
//! workspace's ignored `local/p2/w7-worldlogic/` (AGENTS.md rule 1).
//!
//! It asserts the counts the lane recorded (decision 2026-11-16-60) and, when `local/slice/vendors.json`
//! (`tools/prepare_vendors.py`'s output at stage 8) is present, compares the Reference listing with it field by field:
//! each machine's id and kind, each row's pool and probability (value and int/float kind), each leaf's balance,
//! weight and class; every one must agree and the compared count must be the recorded one.

use std::path::{Path, PathBuf};

use ow_attr::AttributeSystem;
use ow_attr::api::EvalCx;
use ow_balance::offline::{
    GAME_REPLICATION_INFO_CLASS, GLOBALS_CLASS, OfflineWorld, PLAYER_CONTROLLER_CLASS, REPLICATION_INFO_CLASS,
};
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::rand::CrtRand;
use ow_core::{DetMap, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_items::merge::RollData;
use ow_items::pools::{PyNum, off_drop_json};
use ow_items::registry::{CensusInput, Pv, build_from_census, python_path_text};
use ow_items::roll::{RollCx, RollWorld};
use ow_mission::interactive::{Origin, census};
use ow_mission::stations::stations;
use ow_mission::vendors::{
    Machine, MachineRead, MapView, StockRow, VendorError, listing, reset_interval_seconds, roll_attachment,
};
use ow_object::heap::ObjectSpace;

const MAP: &str = "Sanctuary_P";
const STAGE: i32 = 8;
const SEED: u32 = 20_261_116;

/// The counts recorded for the installed Sanctuary (decision 2026-11-16-60).
const MACHINES: usize = 8;
const ATTACHMENTS: usize = 58;
const ATTACHMENTS_ROLLED: usize = 52;
const OBJECTS: usize = 198;
const STATIONS: usize = 4;
const COMPARED_FIELDS: usize = 1_846;

// ---- a minimal JSON reader for the tool's output (test code: objects, arrays, strings, numbers, literals) ----

struct Json<'a> {
    b: &'a [u8],
    i: usize,
}

impl Json<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn eat(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.b.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            Err(format!("expected {} at byte {}", c as char, self.i))
        }
    }

    fn value(&mut self, depth: u32) -> Result<Pv, String> {
        if depth > 64 {
            return Err("nested too deep".into());
        }
        self.ws();
        match self.b.get(self.i) {
            Some(b'{') => {
                self.i += 1;
                let mut m = DetMap::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(Pv::Map(m));
                }
                loop {
                    self.ws();
                    let Pv::Str(k) = self.string()? else { unreachable!() };
                    self.eat(b':')?;
                    m.insert(k, self.value(depth + 1)?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Pv::Map(m));
                        }
                        _ => return Err(format!("bad object at byte {}", self.i)),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(Pv::List(v));
                }
                loop {
                    v.push(self.value(depth + 1)?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Pv::List(v));
                        }
                        _ => return Err(format!("bad array at byte {}", self.i)),
                    }
                }
            }
            Some(b'"') => self.string(),
            Some(b't') => self.word("true", Pv::Bool(true)),
            Some(b'f') => self.word("false", Pv::Bool(false)),
            Some(b'n') => self.word("null", Pv::Null),
            Some(_) => self.number(),
            None => Err("unexpected end".into()),
        }
    }

    fn word(&mut self, w: &str, v: Pv) -> Result<Pv, String> {
        if self.b[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(v)
        } else {
            Err(format!("bad literal at byte {}", self.i))
        }
    }

    fn number(&mut self) -> Result<Pv, String> {
        let start = self.i;
        while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
            self.i += 1;
        }
        let t = std::str::from_utf8(&self.b[start..self.i]).map_err(|e| e.to_string())?;
        if t.contains(['.', 'e', 'E']) {
            t.parse().map(Pv::Float).map_err(|e| format!("{t}: {e}"))
        } else {
            t.parse().map(Pv::Int).map_err(|e| format!("{t}: {e}"))
        }
    }

    fn string(&mut self) -> Result<Pv, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            let Some(&c) = self.b.get(self.i) else { return Err("unterminated string".into()) };
            self.i += 1;
            match c {
                b'"' => return Ok(Pv::Str(out)),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or("bad escape")?;
                    self.i += 1;
                    match e {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = std::str::from_utf8(self.b.get(self.i..self.i + 4).ok_or("bad escape")?)
                                .map_err(|e| e.to_string())?;
                            let code = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                            self.i += 4;
                        }
                        other => out.push(other as char),
                    }
                }
                _ => {
                    // Copy one UTF-8 sequence whole.
                    let start = self.i - 1;
                    let mut end = self.i;
                    while end < self.b.len() && (self.b[end] & 0xC0) == 0x80 {
                        end += 1;
                    }
                    out.push_str(std::str::from_utf8(&self.b[start..end]).map_err(|e| e.to_string())?);
                    self.i = end;
                }
            }
        }
    }
}

fn parse_json(text: &str) -> Result<Pv, String> {
    let mut j = Json { b: text.as_bytes(), i: 0 };
    j.value(0)
}

/// The differences between our listing (`report.json`'s rows) and the tool's machines; the count of fields compared.
fn compare(ours: &[Pv], tool: &Pv, diffs: &mut Vec<String>) -> usize {
    let theirs = tool.at("machines").items();
    let mut fields = 0usize;
    if ours.len() != theirs.len() {
        diffs.push(format!("{} machines against {}", ours.len(), theirs.len()));
    }
    for m in ours {
        let id = m.at("id").as_str().unwrap_or("");
        let Some(t) = theirs.iter().find(|t| t.at("id").as_str() == Some(id)) else {
            diffs.push(format!("{id}: not in the tool's output"));
            continue;
        };
        fields += 1;
        if m.at("kind") != t.at("kind") {
            diffs.push(format!("{id}: kind {:?} against {:?}", m.at("kind"), t.at("kind")));
        }
        for config in ["inventory", "featured"] {
            let (a, b) = (m.at(config).items(), t.at(config).items());
            if a.len() != b.len() {
                diffs.push(format!("{id} {config}: {} rows against {}", a.len(), b.len()));
                continue;
            }
            for (ra, rb) in a.iter().zip(b) {
                fields += 2;
                if ra.at("pool") != rb.at("pool") {
                    diffs.push(format!("{id} {config}: pool {:?} against {:?}", ra.at("pool"), rb.at("pool")));
                }
                // The value and its int or float kind, as the tool's JSON writes it.
                if ra.at("probability") != rb.at("probability") {
                    diffs.push(format!(
                        "{id} {config}: probability {:?} against {:?}",
                        ra.at("probability"),
                        rb.at("probability")
                    ));
                }
                let (la, lb) = (ra.at("items").items(), rb.at("items").items());
                if la.len() != lb.len() {
                    diffs.push(format!("{id} {config}: {} leaves against {}", la.len(), lb.len()));
                    continue;
                }
                for (x, y) in la.iter().zip(lb) {
                    fields += 3;
                    for k in ["balance", "class"] {
                        if x.at(k) != y.at(k) {
                            diffs.push(format!("{id} {config}: {k} {:?} against {:?}", x.at(k), y.at(k)));
                        }
                    }
                    if x.at("weight").number() != y.at("weight").number() {
                        diffs.push(format!("{id} {config}: weight {:?} against {:?}", x.at("weight"), y.at("weight")));
                    }
                }
            }
        }
    }
    fields
}

fn game() -> PathBuf {
    std::env::var_os("OW_GAME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2"))
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn s(text: &str) -> Pv {
    Pv::Str(text.to_string())
}

fn map(entries: Vec<(&str, Pv)>) -> Pv {
    Pv::Map(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn num(n: Option<PyNum>) -> Pv {
    match n {
        Some(PyNum::Int(i)) => Pv::Int(i),
        Some(other) => Pv::Float(other.f()),
        None => Pv::Null,
    }
}

fn rows_json(rows: &[StockRow]) -> Pv {
    Pv::List(
        rows.iter()
            .map(|r| {
                map(vec![
                    ("pool", s(&r.pool)),
                    ("probability", num(r.probability)),
                    (
                        "items",
                        Pv::List(
                            r.items
                                .iter()
                                .map(|l| {
                                    map(vec![
                                        ("balance", s(&l.balance)),
                                        ("weight", Pv::Float(l.weight)),
                                        ("class", s(&l.class)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                    ("unresolved", Pv::List(r.unresolved.iter().map(|u| s(u)).collect())),
                ])
            })
            .collect(),
    )
}

fn snapshot() -> BalanceSnapshot {
    BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(STAGE),
        GameStage(STAGE),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap()
}

#[test]
#[ignore = "needs the installed game"]
fn installed_sanctuary_world_logic() {
    let root = game();
    let out_dir = workspace().join("local").join("p2").join("w7-worldlogic");
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut summary: Vec<String> = Vec::new();
    let mut report = DetMap::new();

    // ---- Reference: the tool's listing ----
    let mut defs = DefStore::open(root.clone(), IdentityPolicy::FirstInScanOrder, ParityMode::Reference).unwrap();
    let census_ref = defs.census().unwrap();
    let root = defs.packages().root().to_path_buf();
    let view = MapView::open(&census_ref, &root, MAP).unwrap();
    let levels: Vec<String> = view.levels.iter().map(|l| l.name.clone()).collect();
    summary.push(format!("levels {}: {}", levels.len(), levels.join(" ")));
    let minutes = view.minutes_between_resets().unwrap();
    summary.push(format!(
        "MinutesBetweenShopResets {minutes} (class default); first countdown {} s",
        reset_interval_seconds(minutes as f32)
    ));

    let found = view.machines();
    let mut machines: Vec<(usize, Machine)> = Vec::new();
    let mut skipped = Vec::new();
    for (level, read) in found {
        match read {
            MachineRead::Machine(m) => machines.push((level, *m)),
            MachineRead::Skipped(reason) => skipped.push(reason),
            _ => {}
        }
    }
    let mut kinds: DetMap<&str, usize> = DetMap::new();
    for (_, m) in &machines {
        *kinds.entry(m.kind.name()).or_insert(0) += 1;
    }
    summary.push(format!("vending machines {} {kinds:?}, skipped points {}", machines.len(), skipped.len()));
    assert!(skipped.is_empty(), "skipped shop points: {skipped:?}");
    assert_eq!(machines.len(), MACHINES, "{kinds:?}");
    let mut listed = Vec::new();
    for (level, m) in &machines {
        let mut values = view.values(*level).unwrap();
        let pools = &view.levels[*level].pools;
        let inventory = listing(pools, &mut values, &m.inventory, STAGE, ParityMode::Reference).unwrap();
        let featured = listing(pools, &mut values, &m.featured, STAGE, ParityMode::Reference).unwrap();
        listed.push(map(vec![
            ("id", s(&m.id)),
            ("kind", s(m.kind.name())),
            ("balance", s(&m.balance)),
            ("inventory", rows_json(&inventory)),
            ("featured", rows_json(&featured)),
        ]));
    }
    let tool_file = workspace().join("local").join("slice").join("vendors.json");
    match std::fs::read_to_string(&tool_file) {
        Ok(text) => {
            let tool = parse_json(&text).unwrap();
            assert_eq!(tool.at("stage").number(), Some(f64::from(STAGE)), "the tool's output is at another stage");
            let mut diffs = Vec::new();
            let fields = compare(&listed, &tool, &mut diffs);
            summary.push(format!(
                "prepare_vendors.py comparison: {fields} fields compared, {} difference(s)",
                diffs.len()
            ));
            assert!(diffs.is_empty(), "differences from prepare_vendors.py: {diffs:#?}");
            assert_eq!(fields, COMPARED_FIELDS);
        }
        Err(e) => summary.push(format!("prepare_vendors.py comparison: not run ({}: {e})", tool_file.display())),
    }
    report.insert("reference_listing".to_string(), Pv::List(listed));

    // ---- interactive objects and stations (package data, mode-free) ----
    let (objects, object_skips) = census(&view);
    assert!(object_skips.is_empty(), "skipped objects: {object_skips:?}");
    assert_eq!(objects.len(), OBJECTS);
    let mut by_class: DetMap<String, (usize, usize, usize)> = DetMap::new();
    for o in &objects {
        let key = format!(
            "{} {}",
            match o.origin {
                Origin::Placed => "placed",
                Origin::Spawned => "spawned",
                _ => "other",
            },
            if o.class.is_empty() { "(no factory)" } else { o.class.as_str() }
        );
        let e = by_class.entry(key).or_insert((0, 0, 0));
        e.0 += 1;
        e.1 += usize::from(o.definition.is_some());
        e.2 += usize::from(o.has_provider);
    }
    summary.push(format!(
        "interactive objects {} (class: count, definition resolved, behavior provider found):",
        objects.len()
    ));
    for (k, (n, d, p)) in &by_class {
        summary.push(format!("  {k}: {n}, {d}, {p}"));
    }
    report.insert(
        "interactive".to_string(),
        Pv::List(
            objects
                .iter()
                .map(|o| {
                    map(vec![
                        ("level", s(&o.level)),
                        ("path", s(&o.path)),
                        ("class", s(&o.class)),
                        ("definition", o.definition.as_deref().map_or(Pv::Null, s)),
                        ("cost_behavior", Pv::Bool(o.has_cost_behavior)),
                    ])
                })
                .collect(),
        ),
    );
    let costly = objects.iter().filter(|o| o.has_cost_behavior).count();
    summary.push(format!(
        "  with a use-cost behavior: {costly}; use resolved in Off mode: 0 (the script stops at the unread \
         WillowInteractiveObject.IsComponentUsable)"
    ));
    let (found_stations, station_skips) = stations(&view);
    assert!(station_skips.is_empty(), "skipped stations: {station_skips:?}");
    assert_eq!(found_stations.len(), STATIONS);
    summary.push(format!("travel stations {}:", found_stations.len()));
    let mut station_rows = Vec::new();
    for st in &found_stations {
        summary.push(format!(
            "  {}:{} {} definition {} initially_active {:?} exit points {:?} touch {:?} / {:?}",
            st.level,
            st.path.rsplit('.').next().unwrap_or(""),
            st.kind.name(),
            st.definition.as_deref().unwrap_or("-"),
            st.initially_active,
            st.exit_points,
            st.touch_radius,
            st.touch_height
        ));
        station_rows.push(map(vec![
            ("level", s(&st.level)),
            ("path", s(&st.path)),
            ("kind", s(st.kind.name())),
            ("definition", st.definition.as_deref().map_or(Pv::Null, s)),
            ("exit_points", st.exit_points.map_or(Pv::Null, |n| Pv::Int(i64::try_from(n).unwrap()))),
        ]));
    }
    report.insert("stations".to_string(), Pv::List(station_rows));
    drop(view);
    // The package-data half is written before the Off rolls, so a failure there keeps it.
    std::fs::write(out_dir.join("summary.txt"), summary.join("\n") + "\n").unwrap();
    std::fs::write(out_dir.join("report.json"), Pv::Map(report.clone()).to_json()).unwrap();

    // ---- Off: each attachment's pool fired once through W4-POOLS' roll ----
    let mut defs = DefStore::open(root.clone(), IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let census_off = defs.census().unwrap();
    let view = MapView::open(&census_off, &root, MAP).unwrap();
    let input = CensusInput::from_store(census_off, &root);
    let data = RollData::new(&input).unwrap();
    let reg = build_from_census(input, &python_path_text(&root.to_string_lossy()));
    let sys = AttributeSystem::standard().unwrap();
    let snap = snapshot();
    // The classes the offline world needs laid out first, as `ow-cli`'s `--item-pool` does.
    let mut space = ObjectSpace::new();
    for class in [PLAYER_CONTROLLER_CLASS, REPLICATION_INFO_CLASS, GAME_REPLICATION_INFO_CLASS, GLOBALS_CLASS] {
        defs.class_id(&mut space, class).unwrap();
    }
    let mut world = OfflineWorld::new(space, Some(defs), snap).unwrap();
    let mut rolled_machines = 0;
    let mut attachment_counts = (0usize, 0usize, 0usize);
    let mut off_rows = Vec::new();
    let mut not_yet: DetMap<String, usize> = DetMap::new();
    for (level, read) in view.machines() {
        let MachineRead::Machine(m) = read else { continue };
        let pools = &view.levels[level].pools;
        let mut all_rolled = true;
        let mut drops = Vec::new();
        for a in m.inventory.iter().chain(&m.featured) {
            attachment_counts.0 += 1;
            let mut rng = CrtRand::new(SEED);
            let mut rw = RollWorld::new(&mut world, &mut rng, false);
            let mut cx = RollCx::new(&snap, EvalCx::new(&mut rw), &sys, &data, ParityMode::Off);
            match roll_attachment(pools, &reg, a, &mut cx) {
                Ok(d) => {
                    attachment_counts.1 += 1;
                    drops.push(map(vec![
                        ("pool", s(&a.pool)),
                        ("drops", Pv::List(d.iter().map(off_drop_json).collect())),
                    ]));
                }
                Err(VendorError::NotYet(n)) => {
                    all_rolled = false;
                    attachment_counts.2 += 1;
                    *not_yet.entry(n.what.to_string()).or_insert(0) += 1;
                }
                Err(e) => {
                    all_rolled = false;
                    *not_yet.entry(format!("error: {e}")).or_insert(0) += 1;
                }
            }
        }
        if all_rolled && !(m.inventory.is_empty() && m.featured.is_empty()) {
            rolled_machines += 1;
        }
        off_rows.push(map(vec![("id", s(&m.id)), ("attachments", Pv::List(drops))]));
    }
    summary.push(format!(
        "Off: machines whose every attachment pool rolls through roll_pool: {rolled_machines} of {}; attachments {} \
         rolled {} NotYet {}; the shelf itself (GenerateInventory) is NotYet for all",
        machines.len(),
        attachment_counts.0,
        attachment_counts.1,
        attachment_counts.2
    ));
    for (what, n) in &not_yet {
        summary.push(format!("  {n} x {what}"));
    }
    assert_eq!((attachment_counts.0, attachment_counts.1), (ATTACHMENTS, ATTACHMENTS_ROLLED), "{not_yet:#?}");
    assert!(not_yet.keys().all(|k| !k.starts_with("error:")), "{not_yet:#?}");
    report.insert("off_rolls".to_string(), Pv::List(off_rows));

    let text = summary.join("\n") + "\n";
    std::fs::write(out_dir.join("summary.txt"), &text).unwrap();
    std::fs::write(out_dir.join("report.json"), Pv::Map(report).to_json()).unwrap();
    print!("{text}");
}
