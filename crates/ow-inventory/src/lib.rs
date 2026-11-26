//! Inventory: the inventory manager natives, equip and unequip effects, pickups, the backpack sort, the menu
//! navigation helper and the item spawn natives that call `ow-items` (port of `src/inventory_navigation.*` and the
//! equip notes).
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod anim_tree;
pub mod attach_mesh;
pub mod definition;
pub mod equip;
pub mod gestalt;
pub mod gestalt_data;
pub mod globals;
pub mod manager;
pub mod navigation;
pub mod pickup;
pub mod pickup_cost;
pub mod pickup_take;
pub mod pickup_targeting;
pub mod sort;
pub mod spawn;
