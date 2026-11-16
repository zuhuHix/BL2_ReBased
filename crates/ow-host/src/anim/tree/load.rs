//! Owner: lane H7-ANIMTREE. Builds a [`TreeDef`] from the definition store: the tree object, then every node its
//! `Children` links reach, each read with its class defaults applied (so an untagged `Rate` or `Child2Weight` is the
//! class's own default, not a guess).
//!
//! **Guards.** Nodes are visited from a work list, each once (by export index); a tree with more than
//! [`MAX_NODES`] nodes is refused. Cycles and the depth budget are checked afterwards on the finished graph
//! ([`super::graph::check`]), so the loader itself never recurses.

use std::fmt;

use ow_core::{DetMap, ObjectPath, OrderedMap};
use ow_defs::DefStore;
use ow_defs::store::DefError;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use super::data::{Child, Kind, Node, NodeId, Selector, SequenceParams, SkelControl, TreeDef};
use super::graph::{self, GraphError};

/// Most nodes a tree may have (the installed trees have 27 and 70).
pub const MAX_NODES: usize = 4096;
/// Most skeletal controls followed along one `NextControl` chain.
pub const MAX_CONTROL_CHAIN: usize = 64;
/// Most super classes followed for one class.
const MAX_CLASS_DEPTH: usize = 32;

/// Why a tree could not be built.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum TreeError {
    BadPath(String),
    Def(DefError),
    /// No holder has the tree's path.
    NotFound(String),
    /// A property did not hold what its class declares.
    BadValue {
        node: String,
        prop: String,
        reason: String,
    },
    /// A node is not an object of a package export.
    NotAnExport(String),
    TooManyNodes,
    Graph(GraphError),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::BadPath(p) => write!(f, "{p} is not an object path"),
            TreeError::Def(e) => write!(f, "definition store: {e}"),
            TreeError::NotFound(p) => write!(f, "no package holds {p}"),
            TreeError::BadValue { node, prop, reason } => write!(f, "{node}.{prop}: {reason}"),
            TreeError::NotAnExport(p) => write!(f, "{p} is not a package export"),
            TreeError::TooManyNodes => write!(f, "the tree has more than {MAX_NODES} nodes"),
            TreeError::Graph(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TreeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TreeError::Def(e) => Some(e),
            TreeError::Graph(e) => Some(e),
            _ => None,
        }
    }
}

impl From<DefError> for TreeError {
    fn from(e: DefError) -> Self {
        TreeError::Def(e)
    }
}

impl From<GraphError> for TreeError {
    fn from(e: GraphError) -> Self {
        TreeError::Graph(e)
    }
}

/// The classes evaluated by their own name or engine base, first match wins along the class chain (the node's
/// class first, then its super classes). The Willow and Gearbox classes listed here only choose a child (their
/// selection is a named input); the ones in [`NOT_YET`] change how poses combine and are not read.
fn kind_of(
    chain: &[String],
    get: &mut dyn FnMut(&str) -> Result<Prop, TreeError>,
) -> Result<Kind, TreeError> {
    for class in chain {
        let short = class.rsplit('.').next().unwrap_or(class);
        if let Some((_, reason)) = NOT_YET.iter().find(|(c, _)| *c == short) {
            return Ok(Kind::NotYet { reason });
        }
        let kind = match short {
            "AnimTree" => Kind::Root,
            "WillowAnimNodeSequenceAdditiveBlend" | "AnimNodeSequence" => {
                let mut p = SequenceParams::new(
                    get("AnimSeqName")?.name(),
                    get("Rate")?.float()?,
                    get("bLooping")?.boolean()?,
                    get("bPlaying")?.boolean()?,
                );
                p.start_time = get("CurrentTime")?.float()?;
                if short == "WillowAnimNodeSequenceAdditiveBlend" {
                    p.add_clip = get("AddAnimSeqName")?.name();
                }
                if chain.iter().any(|c| c.ends_with(".WillowAnimNodeSequence")) {
                    p.auto_start = get("bAutoStart")?.boolean()?;
                }
                p.root_bone_option = get("RootBoneOption")?.bytes3()?;
                p.root_rotation_option = get("RootRotationOption")?.bytes3()?;
                Kind::Sequence(p)
            }
            "AnimNodeBlendByPosture" => list(Selector::Posture, get)?,
            "AnimNodeBlendByPhysics" => list(Selector::Physics, get)?,
            "WillowAnimNode_AimState" => list(Selector::AimState, get)?,
            "WillowAnimNode_Falling" => list(Selector::Falling, get)?,
            "WillowAnimNodeBlendInjured" => list(Selector::Injured, get)?,
            "WillowAnimNodeBlendThirdPersonMenu" => list(Selector::Menu, get)?,
            "WillowAnimNode_Condition" => {
                let flag = get("FlagCondition")?.object_path().unwrap_or_default();
                list(Selector::Flag(flag), get)?
            }
            "AnimNodeBlendBySpeed" => Kind::Speed {
                constraints: get("Constraints")?.floats()?,
                blend_up: get("BlendUpTime")?.float()?,
                blend_down: get("BlendDownTime")?.float()?,
            },
            "AnimNodeBlendDirectional" | "WillowAnimNodeBlendDirectional" => Kind::Directional,
            "AnimNodeAdditiveBlending" => Kind::Additive {
                child2_weight: get("Child2Weight")?.float()?,
            },
            "AnimNodeBlendPerBone" => Kind::PerBone {
                branches: get("BranchStartBoneName")?.names()?,
                child2_weight: get("Child2Weight")?.float()?,
            },
            "AnimNodeScaleRateBySpeed" => Kind::ScaleRate {
                base_speed: get("BaseSpeed")?.float()?,
            },
            "AnimNodeSpecialMoveBlend" => Kind::SpecialMove,
            "WillowAnimNodeSlot" | "AnimNodeSlot" => Kind::Slot,
            _ => continue,
        };
        return Ok(kind);
    }
    Ok(Kind::NotYet {
        reason: "no rule for this class or its engine base",
    })
}

/// Classes whose own native part changes how poses combine and is not read (checked before their engine base).
pub const NOT_YET: [(&str, &str); 5] = [
    (
        "WillowAnimNode_AddCameraBone",
        "two inputs (Source, Add) combined by an unread native rule",
    ),
    (
        "WillowAnimNode_WeaponRecoil",
        "a special-move blend that plays recoil by an unread native rule",
    ),
    (
        "WillowAnimNodeFeatherBlend",
        "a feathered two-input blend whose per-bone rule is not read",
    ),
    (
        "WillowAnimNode_ClimbLadder",
        "a ladder play-rate rule that is not read",
    ),
    (
        "AnimNodeAimOffset",
        "aim offset profiles are not decoded or evaluated in this lane",
    ),
];

fn list(
    selector: Selector,
    get: &mut dyn FnMut(&str) -> Result<Prop, TreeError>,
) -> Result<Kind, TreeError> {
    let blend_time = match get("BlendTime")? {
        Prop::Absent => None,
        p => Some(p.float()?),
    };
    Ok(Kind::List {
        selector,
        blend_time,
    })
}

/// One property value, read loosely and checked by the accessor that wants it.
pub enum Prop {
    /// The class declares no such property.
    Absent,
    Value(Value, String),
    Names(Vec<String>),
    Text(String),
}

impl Prop {
    fn bad(&self, want: &str) -> TreeError {
        let got = match self {
            Prop::Absent => "absent".to_string(),
            Prop::Value(v, _) => v.kind_name().to_string(),
            Prop::Names(_) => "a name list".into(),
            Prop::Text(_) => "a name".into(),
        };
        TreeError::BadValue {
            node: String::new(),
            prop: String::new(),
            reason: format!("{got}, not {want}"),
        }
    }

    fn float(&self) -> Result<f32, TreeError> {
        match self {
            Prop::Value(Value::Float(f), _) if f.is_finite() => Ok(*f),
            _ => Err(self.bad("a finite float")),
        }
    }

    fn boolean(&self) -> Result<bool, TreeError> {
        match self {
            Prop::Value(Value::Bool(b), _) => Ok(*b),
            _ => Err(self.bad("a bool")),
        }
    }

    /// A name, `None` for `None` or an absent property.
    fn name(&self) -> Option<String> {
        match self {
            Prop::Text(t) if t != "None" => Some(t.clone()),
            _ => None,
        }
    }

    fn names(&self) -> Result<Vec<String>, TreeError> {
        match self {
            Prop::Names(n) => Ok(n.clone()),
            Prop::Text(t) => Ok(vec![t.clone()]),
            _ => Err(self.bad("names")),
        }
    }

    fn object_path(&self) -> Option<String> {
        match self {
            Prop::Value(Value::Object(Some(_)), path) => Some(path.clone()),
            _ => None,
        }
    }

    fn floats(&self) -> Result<Vec<f32>, TreeError> {
        match self {
            Prop::Value(Value::Array(items), _) => items
                .iter()
                .map(|v| match v {
                    Value::Float(f) if f.is_finite() => Ok(*f),
                    _ => Err(self.bad("an array of finite floats")),
                })
                .collect(),
            _ => Err(self.bad("an array of floats")),
        }
    }

    fn bytes3(&self) -> Result<[u8; 3], TreeError> {
        match self {
            Prop::Value(Value::Array(items), _) if items.len() == 3 => {
                let mut out = [0u8; 3];
                for (slot, v) in out.iter_mut().zip(items.iter()) {
                    *slot = match v {
                        Value::Byte(b) => *b,
                        _ => return Err(self.bad("three bytes")),
                    };
                }
                Ok(out)
            }
            Prop::Absent => Ok([0; 3]),
            _ => Err(self.bad("three bytes")),
        }
    }
}

/// The loader's view of one object.
struct Reader<'a> {
    defs: &'a mut DefStore,
    space: &'a mut ObjectSpace,
}

impl Reader<'_> {
    fn path_of(&self, o: ObjRef) -> Result<(String, String, i32), TreeError> {
        let origin = self
            .space
            .heap
            .get(o)
            .map_err(|e| bad("?", "object", &e.to_string()))?
            .origin;
        let (package, index) = match origin {
            Origin::Export { package, index } | Origin::Resource { package, index } => {
                (package, index)
            }
            other => return Err(TreeError::NotAnExport(format!("{other:?}"))),
        };
        let pkg = self.defs.packages().get(package);
        let path = pkg
            .path_of(index)
            .map_err(|e| bad("?", "path", &e.to_string()))?;
        Ok((path, pkg.name.clone(), index))
    }

    fn class_chain(&self, o: ObjRef) -> Result<Vec<String>, TreeError> {
        let mut cursor = Some(
            self.space
                .heap
                .get(o)
                .map_err(|e| bad("?", "class", &e.to_string()))?
                .class,
        );
        let mut chain = Vec::new();
        while let Some(c) = cursor {
            if chain.len() == MAX_CLASS_DEPTH {
                break;
            }
            let Some(layout) = self.space.classes.get_layout(c) else {
                break;
            };
            chain.push(layout.path().as_str().to_string());
            cursor = layout.super_class();
        }
        Ok(chain)
    }

    /// Property `name` of `o` (all elements of a static array; a dynamic array as it is).
    fn prop(&mut self, o: ObjRef, node: &str, name: &str) -> Result<Prop, TreeError> {
        let class = self
            .space
            .heap
            .get(o)
            .map_err(|e| bad(node, name, &e.to_string()))?
            .class;
        let Some(p) = self.space.prop(class, name) else {
            return Ok(Prop::Absent);
        };
        let dim = self
            .space
            .classes
            .decl(p)
            .map(|d| d.array_dim)
            .unwrap_or(1)
            .max(1);
        if dim > 1 {
            let mut items = Vec::new();
            for i in 0..u32::try_from(dim).unwrap_or(1) {
                let v = self
                    .space
                    .read(o, PropAddr::element(p, i))
                    .map_err(|e| bad(node, name, &e.to_string()))?;
                items.push(v.clone());
            }
            return Ok(Prop::Value(Value::Array(items.into()), String::new()));
        }
        let v = self
            .space
            .read(o, PropAddr::scalar(p))
            .map_err(|e| bad(node, name, &e.to_string()))?
            .clone();
        Ok(match v {
            Value::Name(n) => Prop::Text(self.space.names.text(n).to_string()),
            Value::Array(ref items)
                if items.iter().all(|i| matches!(i, Value::Name(_))) && !items.is_empty() =>
            {
                Prop::Names(
                    items
                        .iter()
                        .filter_map(|i| match i {
                            Value::Name(n) => Some(self.space.names.text(*n).to_string()),
                            _ => None,
                        })
                        .collect(),
                )
            }
            Value::Object(Some(r)) => {
                let path = self.path_of(r).map(|(p, _, _)| p).unwrap_or_default();
                Prop::Value(v, path)
            }
            v => Prop::Value(v, String::new()),
        })
    }

    fn struct_field<'v>(&self, v: &'v Value, name: &str) -> Option<&'v Value> {
        let s = v.as_struct()?;
        let layout = self.space.classes.struct_layout(s.ty)?;
        let (entry, _) = layout.field(self.space.names.find(name)?)?;
        s.fields.get(usize::try_from(entry).ok()?)
    }
}

fn bad(node: &str, prop: &str, reason: &str) -> TreeError {
    TreeError::BadValue {
        node: node.into(),
        prop: prop.into(),
        reason: reason.into(),
    }
}

/// Loads the tree at `path` and every node it reaches, then checks the graph (cycles, depth, dangling links).
pub fn load_tree(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    path: &str,
) -> Result<TreeDef, TreeError> {
    let object_path = ObjectPath::parse(path).map_err(|_| TreeError::BadPath(path.into()))?;
    let root = defs
        .find(space, &object_path)?
        .ok_or_else(|| TreeError::NotFound(path.into()))?;
    let mut r = Reader { defs, space };
    let (_, file, root_id) = r.path_of(root)?;
    let mut nodes: DetMap<NodeId, Node> = DetMap::new();
    let mut work = vec![root];
    let mut skel_controls = Vec::new();
    while let Some(o) = work.pop() {
        r.defs.ensure_loaded(r.space, o)?;
        let (node_path, _, id) = r.path_of(o)?;
        if nodes.contains_key(&id) {
            continue;
        }
        if nodes.len() == MAX_NODES {
            return Err(TreeError::TooManyNodes);
        }
        let chain = r.class_chain(o)?;
        let kind = {
            let mut get = |name: &str| r.prop(o, &node_path, name);
            kind_of(&chain, &mut get).map_err(|e| with_node(e, &node_path))?
        };
        let name = r
            .prop(o, &node_path, "NodeName")?
            .name()
            .unwrap_or_default();
        let mut children = Vec::new();
        if let Prop::Value(Value::Array(items), _) = r.prop(o, &node_path, "Children")? {
            for item in items.iter() {
                let child_name = match r.struct_field(item, "Name") {
                    Some(Value::Name(n)) => r.space.names.text(*n).to_string(),
                    _ => return Err(bad(&node_path, "Children.Name", "not a name")),
                };
                let additive =
                    matches!(r.struct_field(item, "bIsAdditive"), Some(Value::Bool(true)));
                let target = match r.struct_field(item, "Anim") {
                    Some(Value::Object(Some(c))) => Some(*c),
                    Some(Value::Object(None)) => None,
                    _ => return Err(bad(&node_path, "Children.Anim", "not an object")),
                };
                let node = match target {
                    Some(c) => {
                        r.defs.ensure_loaded(r.space, c)?;
                        let (_, _, child_id) = r.path_of(c)?;
                        work.push(c);
                        Some(child_id)
                    }
                    None => None,
                };
                let mut child = Child::new(child_name, node);
                child.additive = additive;
                children.push(child);
            }
        }
        if id == root_id {
            skel_controls = controls(&mut r, o, &node_path)?;
        }
        let class = chain.first().cloned().unwrap_or_default();
        nodes.insert(
            id,
            Node::new(node_path, class, kind, children).with_name(name),
        );
    }
    // Export-index order (the map's key order), as the brief asks: deterministic whatever the walk order was.
    let ordered: OrderedMap<NodeId, Node> = nodes.into_iter().collect();
    let mut tree = TreeDef::new(path, root_id, ordered);
    tree.file = file;
    tree.skel_controls = skel_controls;
    graph::check(&tree)?;
    Ok(tree)
}

fn with_node(e: TreeError, node: &str) -> TreeError {
    match e {
        TreeError::BadValue { prop, reason, .. } => TreeError::BadValue {
            node: node.into(),
            prop,
            reason,
        },
        e => e,
    }
}

/// The skeletal controls the root's `SkelControlLists` name, following each `NextControl` chain.
fn controls(
    r: &mut Reader<'_>,
    root: ObjRef,
    root_path: &str,
) -> Result<Vec<SkelControl>, TreeError> {
    let mut out = Vec::new();
    let Prop::Value(Value::Array(lists), _) = r.prop(root, root_path, "SkelControlLists")? else {
        return Ok(out);
    };
    for list in lists.iter() {
        let bone = match r.struct_field(list, "BoneName") {
            Some(Value::Name(n)) => r.space.names.text(*n).to_string(),
            _ => String::new(),
        };
        let mut next = match r.struct_field(list, "ControlHead") {
            Some(Value::Object(o)) => *o,
            _ => None,
        };
        let mut steps = 0;
        while let Some(c) = next {
            if steps == MAX_CONTROL_CHAIN {
                return Err(bad(
                    root_path,
                    "SkelControlLists",
                    "a NextControl chain longer than the budget",
                ));
            }
            steps += 1;
            r.defs.ensure_loaded(r.space, c)?;
            let (path, _, _) = r.path_of(c)?;
            let class = r.class_chain(c)?.first().cloned().unwrap_or_default();
            out.push(SkelControl {
                bone: bone.clone(),
                class,
                path: path.clone(),
            });
            next = match r.prop(c, &path, "NextControl")? {
                Prop::Value(Value::Object(o), _) => o,
                _ => None,
            };
        }
    }
    Ok(out)
}
