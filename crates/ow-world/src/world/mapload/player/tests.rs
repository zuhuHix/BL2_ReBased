//! Owner: lane IMPL-MAPLOAD-PLAYER. Tests of the local-player spawn on the world fixture (invented packages, AGENTS.md
//! rule 9). The fixture's `ToyGame` declares `Login` and `PostLogin` as natives; the tests bind them to stand in for the
//! game's script: `Login` spawns a `PlayerController` at a toy start (or answers None), `PostLogin` records what it
//! sees.

use std::cell::RefCell;
use std::rc::Rc;

use super::*;

use ow_core::Missing;
use ow_core::notyet::is_partial_writes;
use ow_vm::api::NativeCall;

use crate::actors::SpawnRequest;
use crate::world::tests::{Fx, fx_in, run};

const LOGIN: &str = "ToyGame.Login(string,string,int,string)";
const POST_LOGIN: &str = "ToyGame.PostLogin(object)";

/// What the toy `Login` does.
#[derive(Clone)]
enum Answer {
    /// Spawns a `PlayerController` at the start.
    Spawn,
    /// Returns None with this error text, after spawning a stray `Probe` when `write` is set.
    None { error: &'static str, write: bool },
}

/// What the toy natives saw: `Login`'s portal, options and id; at `PostLogin`, the controller's `Player`,
/// `NetPlayerIndex`, `Role` and `RemoteRole`, and the local player's `Actor`.
#[derive(Default)]
struct Seen {
    login: Vec<(Value, Value, Value)>,
    post_login: Vec<(ObjRef, Value, Value, Value, Value, Value)>,
}

struct Toy {
    f: Fx,
    game: ObjRef,
    start: ObjRef,
    seen: Rc<RefCell<Seen>>,
}

fn field(f: &Fx, o: ObjRef, name: &str) -> Value {
    actors::field(&f.w.vm.space, o, name).unwrap()
}

/// A begun world whose `WorldInfo.Game` is a `ToyGame`, with a `Probe` as the toy start at (5, 6, 7), and the two
/// natives bound to `answer`.
fn toy(mode: ParityMode, answer: Answer) -> Toy {
    let mut f = fx_in(false, mode);
    let c = f.class("ToyGame");
    let game = ow_vm::interp::instantiate_from_defaults(&mut f.w.vm, c).unwrap();
    f.w.actors().borrow_mut().place(game).unwrap();
    // The routine runs after the bring-up: play has begun.
    f.w.actors().borrow_mut().begin_play();
    let wi = f.wi;
    actors::set_field(&mut f.w.vm.space, wi, "Game", Value::Object(Some(game))).unwrap();
    let c = f.class("Probe");
    let start = ow_vm::interp::instantiate_from_defaults(&mut f.w.vm, c).unwrap();
    let Value::Struct(v) = actors::field(&f.w.vm.space, start, "Location").unwrap() else { panic!("a vector") };
    let at = Value::Struct(Rc::new(ow_object::value::StructValue {
        ty: v.ty,
        fields: vec![Value::Float(5.0), Value::Float(6.0), Value::Float(7.0)],
    }));
    actors::set_field(&mut f.w.vm.space, start, "Location", at).unwrap();
    let seen = Rc::new(RefCell::new(Seen::default()));
    let sink = seen.clone();
    f.w.vm
        .register_native(
            LOGIN,
            Rc::new(move |c: &mut NativeCall<'_>| {
                sink.borrow_mut().login.push((c.arg(0)?.clone(), c.arg(1)?.clone(), c.arg(2)?.clone()));
                let this = c.this.expect("Login on the game info");
                match &answer {
                    Answer::Spawn => {
                        let pc = c.vm.find_class("Engine.PlayerController")?;
                        let location = actors::field(&c.vm.space, start, "Location")?;
                        let req = SpawnRequest { location: Some(location), ..SpawnRequest::new(Some(pc)) };
                        let made = actors::spawn(c.vm, this, &req)?;
                        Ok(Value::Object(made))
                    }
                    Answer::None { error, write } => {
                        if *write {
                            let probe = c.vm.find_class("Engine.Probe")?;
                            actors::spawn(c.vm, this, &SpawnRequest::new(Some(probe)))?;
                        }
                        c.set_out(3, Value::Str((*error).into()))?;
                        Ok(Value::Object(None))
                    }
                }
            }),
        )
        .unwrap();
    let sink = seen.clone();
    f.w.vm
        .register_native(
            POST_LOGIN,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let Value::Object(Some(pc)) = c.arg(0)?.clone() else { panic!("PostLogin with a controller") };
                let space = &c.vm.space;
                let player = match actors::field(space, pc, "Player")? {
                    Value::Object(Some(p)) => actors::field(space, p, "Actor")?,
                    _ => Value::None,
                };
                sink.borrow_mut().post_login.push((
                    pc,
                    actors::field(space, pc, "Player")?,
                    actors::field(space, pc, "NetPlayerIndex")?,
                    actors::field(space, pc, "Role")?,
                    actors::field(space, pc, "RemoteRole")?,
                    player,
                ));
                Ok(Value::None)
            }),
        )
        .unwrap();
    Toy { f, game, start, seen }
}

fn input<'a>(players: &'a [ObjRef]) -> PlayActorInput<'a> {
    PlayActorInput { url: "Toy_P?Listen?Quick#Gate", game_players: players, net_id: NetIdAnswer::Empty, remote_role: Some(1) }
}

fn not_yet(e: VmError) -> NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

#[test]
fn the_url_gives_its_joined_options_and_its_portal() {
    assert_eq!(url_options("Sanctuary_P"), (String::new(), String::new()));
    assert_eq!(url_options("Toy_P?Listen?Quick#Gate"), ("?Listen?Quick".into(), "Gate".into()));
    assert_eq!(url_options("Toy_P??Name=A#"), ("?Name=A".into(), String::new()));
}

/// The routine in the hub's order: `Login` with the joined options, the portal and the empty id; the controller (at the
/// toy start) gets the net index, the attachment and the roles before `PostLogin`; the local player's `Actor` is set
/// after `PostLogin`; the controller is the world's local player last.
#[test]
fn login_then_the_attachment_then_post_login_then_the_play_actor() {
    run(|| {
        let mut x = toy(ParityMode::Off, Answer::Spawn);
        let other = make_local_player(&mut x.f.w, "Engine.LocalPlayer").unwrap();
        let player = make_local_player(&mut x.f.w, "Engine.LocalPlayer").unwrap();
        let players = [other, player];
        let PlayActorOutcome::Spawned(made) = spawn_play_actor(&mut x.f.w, player, &input(&players)).unwrap() else {
            panic!("a controller")
        };
        let pc = made.controller;
        assert_eq!(made.index, 1, "the player's index in GamePlayers");
        let seen = x.seen.borrow();
        assert_eq!(seen.login, vec![(Value::Str("Gate".into()), Value::Str("?Listen?Quick".into()), Value::Int(0))]);
        let expected = (pc, Value::Object(Some(player)), Value::Byte(1), Value::Byte(ROLE_AUTHORITY), Value::Byte(1));
        let (c, p, n, r, rr, actor_then) = seen.post_login[0].clone();
        assert_eq!((c, p, n, r, rr), expected, "attached, indexed and given its roles before PostLogin");
        assert_eq!(actor_then, Value::Object(None), "the local player's Actor is set after PostLogin");
        assert_eq!(field(&x.f, player, "Actor"), Value::Object(Some(pc)));
        assert_eq!(field(&x.f, pc, "Location"), field(&x.f, x.start, "Location"), "spawned at the toy start");
        assert_eq!(x.f.w.actors().borrow().local_players(), &[pc]);
        assert_eq!(field(&x.f, other, "Actor"), Value::Object(None));
        let _ = x.game;
    });
}

/// `Login` answering None: with an error text, the load's failure carrying the text unchanged; without one, "Login
/// returned None without an error" (`NotYet { NotRead }`, marked only when `Login` wrote). Nothing after `Login` runs:
/// no attachment, no `PostLogin`, no local player.
#[test]
fn a_none_login_attaches_nothing_and_keeps_the_error() {
    for (error, write) in [("toy server full", false), ("toy server full", true), ("", false), ("", true)] {
        run(move || {
            let mut x = toy(ParityMode::Off, Answer::None { error, write });
            let player = make_local_player(&mut x.f.w, "Engine.LocalPlayer").unwrap();
            let got = spawn_play_actor(&mut x.f.w, player, &input(&[player]));
            if error.is_empty() {
                let n = not_yet(got.unwrap_err());
                assert_eq!(n.missing, Missing::NotRead);
                assert!(n.what.ends_with("Login returned None without an error"), "{}", n.what);
                assert_eq!(is_partial_writes(&n.what), write, "marked only after Login wrote: {}", n.what);
            } else {
                assert_eq!(got.unwrap(), PlayActorOutcome::LoginFailed { error: error.to_string() }, "the text unchanged");
            }
            assert!(x.seen.borrow().post_login.is_empty(), "no PostLogin");
            assert_eq!(field(&x.f, player, "Actor"), Value::Object(None), "no attachment");
            assert!(x.f.w.actors().borrow().local_players().is_empty());
        });
    }
}

/// Every refusal before any write: unmarked, `Login` not called, the counters unchanged.
#[test]
fn the_preflight_refuses_before_login() {
    type Case = (&'static str, Missing, fn(&mut Toy, ObjRef) -> (Vec<ObjRef>, PlayActorInput<'static>));
    let cases: [Case; 7] = [
        ("unknown id", Missing::NotRead, |_, p| (vec![p], PlayActorInput { net_id: NetIdAnswer::Unknown, ..input(&[]) })),
        ("no remote role", Missing::NotRead, |_, p| (vec![p], PlayActorInput { remote_role: None, ..input(&[]) })),
        ("authority as the remote role", Missing::NotRead, |_, p| {
            (vec![p], PlayActorInput { remote_role: Some(ROLE_AUTHORITY), ..input(&[]) })
        }),
        ("a client", Missing::NotRead, |x, p| {
            let wi = x.f.wi;
            actors::set_field(&mut x.f.w.vm.space, wi, "NetMode", Value::Byte(NET_MODE_CLIENT)).unwrap();
            (vec![p], input(&[]))
        }),
        ("already an actor", Missing::NotRead, |x, p| {
            let s = x.start;
            actors::set_field(&mut x.f.w.vm.space, p, "Actor", Value::Object(Some(s))).unwrap();
            (vec![p], input(&[]))
        }),
        ("no game info", Missing::NotRead, |x, p| {
            let wi = x.f.wi;
            actors::set_field(&mut x.f.w.vm.space, wi, "Game", Value::Object(None)).unwrap();
            (vec![p], input(&[]))
        }),
        ("not in GamePlayers", Missing::NotRead, |_, _| (Vec::new(), input(&[]))),
    ];
    for (name, missing, setup) in cases {
        run(move || {
            let mut x = toy(ParityMode::Off, Answer::Spawn);
            let player = make_local_player(&mut x.f.w, "Engine.LocalPlayer").unwrap();
            let (players, given) = setup(&mut x, player);
            let given = PlayActorInput { game_players: &players, ..given };
            let mark = WriteMark::now(&x.f.w.vm);
            let e = spawn_play_actor(&mut x.f.w, player, &given).unwrap_err();
            if name == "not in GamePlayers" {
                assert!(matches!(&e, VmError::Script { message, .. } if message.contains("GamePlayers")), "{e:?}");
            } else {
                let n = not_yet(e);
                assert_eq!(n.missing, missing, "{name}: {}", n.what);
                assert!(!is_partial_writes(&n.what), "{name}: unmarked");
            }
            assert!(mark.still(&x.f.w.vm), "{name}: nothing written");
            assert!(x.seen.borrow().login.is_empty(), "{name}: Login not called");
            assert!(x.f.w.actors().borrow().local_players().is_empty(), "{name}");
        });
    }
}

/// A game info whose class defines no `Login`: a plain data error (not a `NotYet`), unmarked, nothing written.
#[test]
fn a_game_info_without_login_is_a_data_error() {
    run(|| {
        let mut x = toy(ParityMode::Off, Answer::Spawn);
        let player = make_local_player(&mut x.f.w, "Engine.LocalPlayer").unwrap();
        let (wi, other) = (x.f.wi, x.start);
        actors::set_field(&mut x.f.w.vm.space, wi, "Game", Value::Object(Some(other))).unwrap();
        let mark = WriteMark::now(&x.f.w.vm);
        let e = spawn_play_actor(&mut x.f.w, player, &input(&[player])).unwrap_err();
        assert!(
            matches!(&e, VmError::Script { message, .. } if message.contains("defines no Login") && !is_partial_writes(message)),
            "{e:?}"
        );
        assert!(mark.still(&x.f.w.vm), "nothing written");
        assert!(x.seen.borrow().login.is_empty());
        assert!(x.f.w.actors().borrow().local_players().is_empty());
    });
}

/// Reference is unchanged: the C++ has no map load, so the routine is not ported there.
#[test]
fn reference_has_no_local_player_spawn() {
    run(|| {
        let mut x = toy(ParityMode::Reference, Answer::Spawn);
        let player = make_local_player(&mut x.f.w, "Engine.LocalPlayer").unwrap();
        let n = not_yet(spawn_play_actor(&mut x.f.w, player, &input(&[player])).unwrap_err());
        assert_eq!(n.missing, Missing::NotPorted);
        assert!(x.seen.borrow().login.is_empty());
    });
}

// ------------------------------------------------------------------------------------------ installed

/// Sanctuary on the installed game (`OW_BL2_ROOT`) in Off: the map load, the map route (lane 2), then one local player
/// of the engine's configured class (`Engine.LocalPlayer`: the installed base engine config file's
/// `LocalPlayerClassName`, which the default engine config file does not override) through the local-player spawn with the map URL's text. The remote
/// role is a harness input (1, `ROLE_SimulatedProxy`: the single-player value is not read, UNVERIFIED). Asserts what is
/// reachable: lane 2's game info, and an outcome that agrees with the world (a play actor and a local player exactly
/// when it spawned). Reports the rest to `local/p2/mapload-player/play_actor_Off.md` (class paths and counts only).
#[test]
#[ignore = "reads the installed game (OW_BL2_ROOT); writes local/p2/mapload-player/"]
fn sanctuary_s_local_player_on_the_installed_game() {
    use std::fmt::Write as _;

    use crate::world::mapload::bringup::{BringUpOptions, RouteMode, bring_up_map_for_play};
    use crate::world::mapload::game_info::{GameInfoInput, installed_default_game};

    let root = std::path::PathBuf::from(std::env::var_os("OW_BL2_ROOT").expect("set OW_BL2_ROOT"));
    let cooked = root.join("WillowGame/CookedPCConsole");
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/p2/mapload-player");
    std::fs::create_dir_all(&out).unwrap();
    let mode = ParityMode::Off;
    let text = ow_vm::interp::on_vm_stack(move || {
        let default_game = installed_default_game(&root).map_err(|e| e.to_string())?;
        let defs = ow_defs::DefStore::open(cooked, ow_defs::identity::IdentityPolicy::FirstInScanOrder, mode)
            .expect("the store");
        let vm = ow_vm::api::Vm::new(
            ow_object::heap::ObjectSpace::new(),
            defs,
            Rc::new(ow_attr::AttributeSystem::standard().map_err(|e| e.to_string())?),
            mode,
        )
        .with_stub_policy(ow_vm::api::StubPolicy::refuse_all());
        let snapshot = ow_balance::types::BalanceSnapshot::new(
            ow_balance::types::Playthrough::NORMAL,
            ow_balance::types::OpLevel::NONE,
            ow_balance::types::PlayerCount::SINGLE,
            ow_balance::types::ExpLevel::new(8),
            ow_balance::types::GameStage(8),
            ow_balance::types::AwesomeLevel::new(0),
            ow_balance::types::LevelCapUpgrades::NONE,
        )
        .map_err(|e| e.to_string())?;
        let mut w = World::new(vm, crate::world::BalanceState::new(snapshot), &[&ow_vm::natives_core::Pack])
            .map_err(|e| e.to_string())?;
        // Before the map, as the engine's start-up does: one local player, no actor.
        let player = make_local_player(&mut w, "Engine.LocalPlayer").map_err(|e| e.to_string())?;
        super::super::load_map(&mut w, &ow_core::PackageName::parse("Sanctuary_P").unwrap()).map_err(|e| e.to_string())?;
        let input = GameInfoInput { options: "", default_game: default_game.as_deref() };
        let every = BringUpOptions { reset_time: true, mode: RouteMode::EveryActor };
        let up = bring_up_map_for_play(&mut w, &input, every).map_err(|e| e.to_string())?;
        assert_eq!(up.game_info.class, "WillowGame.WillowCoopGameInfo", "lane 2's game info");
        let mut r = String::new();
        let _ = writeln!(r, "# Sanctuary's local player ({mode:?}; local, class paths and counts only)");
        let _ = writeln!(r, "game info: {} (bring-up failures {})", up.game_info.class, up.bring_up.failures.len());
        let players = [player];
        let given = PlayActorInput {
            url: "Sanctuary_P",
            game_players: &players,
            net_id: NetIdAnswer::Empty,
            remote_role: Some(1),
        };
        let got = spawn_play_actor(&mut w, player, &given);
        let actor = actors::field(&w.vm.space, player, "Actor").map_err(|e| e.to_string())?;
        let locals = w.actors().borrow().local_players().to_vec();
        match &got {
            Ok(PlayActorOutcome::Spawned(made)) => {
                assert_eq!(actor, Value::Object(Some(made.controller)));
                assert_eq!(locals, vec![made.controller]);
                let _ = writeln!(r, "spawned: {}", ow_attr::eval::class_path(&w.vm.space, made.controller));
            }
            Ok(other) => {
                assert_eq!((actor.clone(), locals.len()), (Value::Object(None), 0));
                let _ = writeln!(r, "outcome: {other:?}");
            }
            Err(e) => {
                assert_eq!((actor.clone(), locals.len()), (Value::Object(None), 0), "no play actor on a failure");
                let marked = matches!(e, VmError::NotYet(n) if is_partial_writes(&n.what));
                let text: String = e.to_string().chars().take(300).collect();
                let _ = writeln!(r, "stopped (marked {marked}): {text}");
            }
        }
        Ok::<_, String>(r)
    })
    .expect("the VM thread")
    .expect("the local player");
    std::fs::write(out.join("play_actor_Off.md"), &text).unwrap();
    print!("{text}");
}
