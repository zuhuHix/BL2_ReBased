//! Owner: lane H7-ANIMTREE. An AnimTree as plain data: its nodes by export index, what each node does (its
//! [`Kind`], chosen from its class and the class's engine base), and the links to its children.
//!
//! **Which classes are evaluated, and why.** A node's class is matched first by name, then through its super-class
//! chain to an engine base class (the chain is read from the code packages; decision 56 lists it). A Willow or
//! Gearbox subclass whose only addition is *which* child is active (its selection rule is native and unread) is
//! evaluated by its base class's rule, with the selection given by the driver as a named input. A subclass that may
//! change *how* poses combine (its native part unread) is [`Kind::NotYet`].

use ow_core::OrderedMap;

/// A node's key: its export index in the file that holds the tree.
pub type NodeId = i32;

/// The named input a blend list's active child comes from (the native selection rules are not read).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// `AnimNodeBlendByPosture`: standing, crouched, sprinting.
    Posture,
    /// `AnimNodeBlendByPhysics`: the pawn's physics mode, one child per mode.
    Physics,
    /// `WillowAnimNode_AimState`: idle, iron sights and two transitions.
    AimState,
    /// `WillowAnimNode_Falling`: pass through, start, idle, end.
    Falling,
    /// `WillowAnimNodeBlendInjured`: not injured, injured.
    Injured,
    /// `WillowAnimNodeBlendThirdPersonMenu`: not viewing, viewing the menu.
    Menu,
    /// `WillowAnimNode_Condition`: a flag object (its path); child 0 is FALSE, child 1 TRUE.
    Flag(String),
}

impl Selector {
    /// The input's name in [`super::Inputs::select`] (flags use their path in [`super::Inputs::flags`]).
    pub fn input_name(&self) -> &str {
        match self {
            Selector::Posture => "posture",
            Selector::Physics => "physics",
            Selector::AimState => "aim_state",
            Selector::Falling => "falling",
            Selector::Injured => "injured",
            Selector::Menu => "menu",
            Selector::Flag(path) => path,
        }
    }
}

/// What a node does.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// `AnimTree` / `WillowAnimTree`: its one child is the pose.
    Root,
    /// `AnimNodeSequence` and its subclass `WillowAnimNodeSequence`: plays one clip. `add_clip` is set for
    /// `WillowAnimNodeSequenceAdditiveBlend`, which plays a second, additive clip on top.
    Sequence(SequenceParams),
    /// An `AnimNodeBlendList` whose active child comes from a named input.
    List {
        selector: Selector,
        /// The node's own `BlendTime` where its class has one; otherwise the driver's default.
        blend_time: Option<f32>,
    },
    /// `AnimNodeBlendBySpeed`: the active child is the speed band of `constraints`.
    Speed {
        constraints: Vec<f32>,
        blend_up: f32,
        blend_down: f32,
    },
    /// `AnimNodeBlendDirectional` / `WillowAnimNodeBlendDirectional`: forward, backward, left, right by the move
    /// direction.
    Directional,
    /// `AnimNodeAdditiveBlending`: child 0 is the base, child 1 an additive delta at `Child2Weight`.
    Additive { child2_weight: f32 },
    /// `AnimNodeBlendPerBone`: child 1 replaces child 0 at `Child2Weight` on the branches starting at these bones.
    PerBone {
        branches: Vec<String>,
        child2_weight: f32,
    },
    /// `AnimNodeScaleRateBySpeed`: one child, played at `speed / base_speed`.
    ScaleRate { base_speed: f32 },
    /// `AnimNodeSlot` / `WillowAnimNodeSlot`: child 0 is the source; the other children are channels for one-shot
    /// clips the driver requests.
    Slot,
    /// `AnimNodeSpecialMoveBlend`: child 0 is the normal pose; a requested special move plays over it with the
    /// blend rules of `NATIVE_PHASELOCK_PRESENTATION.md` section 2.3.
    SpecialMove,
    /// A class whose rule is not read: named, counted, and passed through or refused at evaluation.
    NotYet { reason: &'static str },
}

impl Kind {
    pub fn is_not_yet(&self) -> bool {
        matches!(self, Kind::NotYet { .. })
    }
}

/// A sequence node's parameters, as the tree data (with the class defaults applied) gives them.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceParams {
    /// `AnimSeqName`; `None` for `None`.
    pub clip: Option<String>,
    /// `AddAnimSeqName` of `WillowAnimNodeSequenceAdditiveBlend`.
    pub add_clip: Option<String>,
    pub rate: f32,
    pub looping: bool,
    pub playing: bool,
    /// The stored `CurrentTime`: the node's time when the tree is made.
    pub start_time: f32,
    /// `WillowAnimNodeSequence.bAutoStart`: its script `OnInit` starts the clip (`PlayAnim` with the node's
    /// `bLooping` and `Rate`).
    pub auto_start: bool,
    /// `RootBoneOption` (three bytes, X Y Z) and `RootRotationOption` (three bytes): read and reported, not applied.
    pub root_bone_option: [u8; 3],
    pub root_rotation_option: [u8; 3],
}

impl SequenceParams {
    pub fn new(clip: Option<String>, rate: f32, looping: bool, playing: bool) -> Self {
        SequenceParams {
            clip,
            add_clip: None,
            rate,
            looping,
            playing,
            start_time: 0.0,
            auto_start: false,
            root_bone_option: [0; 3],
            root_rotation_option: [0; 3],
        }
    }
}

/// One child link (`AnimBlendChild`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Child {
    pub name: String,
    /// `None` for an empty link (a slot's free channel).
    pub node: Option<NodeId>,
    /// The stored `bIsAdditive`.
    pub additive: bool,
}

impl Child {
    pub fn new(name: impl Into<String>, node: Option<NodeId>) -> Self {
        Child {
            name: name.into(),
            node,
            additive: false,
        }
    }
}

/// One node.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Object path (the tree data's), for reports.
    pub path: String,
    /// Class path, such as `Engine.AnimNodeSequence`.
    pub class: String,
    /// `NodeName`, or empty.
    pub name: String,
    pub kind: Kind,
    pub children: Vec<Child>,
}

impl Node {
    pub fn new(
        path: impl Into<String>,
        class: impl Into<String>,
        kind: Kind,
        children: Vec<Child>,
    ) -> Self {
        Node {
            path: path.into(),
            class: class.into(),
            name: String::new(),
            kind,
            children,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// A skeletal control named by the tree (`SkelControlLists`): counted and reported, not applied.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkelControl {
    pub bone: String,
    pub class: String,
    pub path: String,
}

/// A whole tree, nodes in export-index order.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TreeDef {
    /// The tree's object path.
    pub path: String,
    /// The file the store took it from.
    pub file: String,
    pub root: NodeId,
    pub nodes: OrderedMap<NodeId, Node>,
    pub skel_controls: Vec<SkelControl>,
    /// Node classes the tree names but whose object is not a node (none expected); kept for the report.
    pub notes: Vec<String>,
}

impl TreeDef {
    pub fn new(path: impl Into<String>, root: NodeId, nodes: OrderedMap<NodeId, Node>) -> Self {
        TreeDef {
            path: path.into(),
            file: String::new(),
            root,
            nodes,
            skel_controls: Vec::new(),
            notes: Vec::new(),
        }
    }
}
