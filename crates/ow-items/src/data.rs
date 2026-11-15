//! Owner: lane W1-ITEMDATA. Item identity: the definition data that rolls, inventory, cards, saves and UI share
//! (architecture section 3.7, report F12).
//!
//! [`DefinitionData`] mirrors the game's two definition-data structs, `WillowWeaponTypes.WeaponDefinitionData` (weapons,
//! mission weapons) and `WillowItemTypes.ItemDefinitionData` (every other kind). Both declare 17 fields in the same
//! shape: the definition, the balance, the manufacturer, the manufacturer grade index, nine part slots, the prefix and
//! title name parts, the game stage and a unique id. The field names, their types, the referenced classes and the
//! declaration order in [`WEAPON_DEFINITION_DATA`] and [`ITEM_DEFINITION_DATA`] were read from the installed
//! `WillowGame.upk` with the C++ reader (`ow-package --exports`, `--payload`): the order is the one the C++ runtime's
//! `childProperties` gives (descending export index, `src/vm.cpp`), and it equals the `Next` chain of the property
//! exports, which ends at `UniqueId` in both structs. Each property has `ArrayDim` 1. Only names and types are here; no
//! value from a package or a save.
//!
//! What is known about the fields' meaning, each with its label:
//! - `ManufacturerGradeIndex` is the item level: verified live (real-game lane L8, `AI_MELEE_TARGETING_LIVE.md`, a gun
//!   rebuilt with grade index 8 had the damage of a stage-8 roll; grade index 0 gave `ExpLevel` 0).
//! - `GameStage` is the stage the item was rolled at (`game_stage`, typed [`GameStage`]); its separate effect on stats
//!   is not read here.
//! - `UniqueId` is carried as read; what writes it and when is not read (UNVERIFIED).
//! - The mission weapon is a normal weapon with a `MissionWeaponBalanceDefinition` balance (`NATIVE_INVENTORY_EQUIP.md`
//!   item 5), so it uses `WeaponDefinitionData`.
//!
//! Not here (later lanes): rolls, the registry, stats, cards, and the save serial packing (decision D9).

use std::fmt;

use ow_balance::types::GameStage;
use ow_core::{NotYet, ObjectPath};

/// Number of fields of `WeaponDefinitionData` and of `ItemDefinitionData` (the same in both).
pub const FIELD_COUNT: usize = 17;
/// Number of part slots in either struct.
pub const SLOT_COUNT: usize = 9;

/// Field positions in declaration order, the same in both structs.
pub mod field {
    /// `WeaponTypeDefinition` / `ItemDefinition`.
    pub const DEFINITION: usize = 0;
    /// `BalanceDefinition`.
    pub const BALANCE: usize = 1;
    /// `ManufacturerDefinition`.
    pub const MANUFACTURER: usize = 2;
    /// `ManufacturerGradeIndex`.
    pub const MANUFACTURER_GRADE: usize = 3;
    /// The first part slot (`BodyPartDefinition` / `AlphaItemPartDefinition`); the nine slots follow in slot order.
    pub const FIRST_PART: usize = 4;
    /// `PrefixPartDefinition` / `PrefixItemNamePartDefinition`.
    pub const PREFIX: usize = 13;
    /// `TitlePartDefinition` / `TitleItemNamePartDefinition`.
    pub const TITLE: usize = 14;
    /// `GameStage`.
    pub const GAME_STAGE: usize = 15;
    /// `UniqueId`.
    pub const UNIQUE_ID: usize = 16;
}

/// The type a struct field is declared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// An `ObjectProperty` whose declared class is `class` (short name, as the package's reference names it).
    Object { class: &'static str },
    /// An `IntProperty`.
    Int,
}

/// One declared field of a definition-data struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldDecl {
    pub name: &'static str,
    pub ty: FieldType,
}

const fn obj(name: &'static str, class: &'static str) -> FieldDecl {
    FieldDecl {
        name,
        ty: FieldType::Object { class },
    }
}

const fn int(name: &'static str) -> FieldDecl {
    FieldDecl {
        name,
        ty: FieldType::Int,
    }
}

/// `WillowWeaponTypes.WeaponDefinitionData` in declaration order (read from `WillowGame.upk`, see the module doc).
pub const WEAPON_DEFINITION_DATA: [FieldDecl; FIELD_COUNT] = [
    obj("WeaponTypeDefinition", "WeaponTypeDefinition"),
    obj("BalanceDefinition", "InventoryBalanceDefinition"),
    obj("ManufacturerDefinition", "ManufacturerDefinition"),
    int("ManufacturerGradeIndex"),
    obj("BodyPartDefinition", "WeaponPartDefinition"),
    obj("GripPartDefinition", "WeaponPartDefinition"),
    obj("BarrelPartDefinition", "WeaponPartDefinition"),
    obj("SightPartDefinition", "WeaponPartDefinition"),
    obj("StockPartDefinition", "WeaponPartDefinition"),
    obj("ElementalPartDefinition", "WeaponPartDefinition"),
    obj("Accessory1PartDefinition", "WeaponPartDefinition"),
    obj("Accessory2PartDefinition", "WeaponPartDefinition"),
    obj("MaterialPartDefinition", "WeaponPartDefinition"),
    obj("PrefixPartDefinition", "WeaponNamePartDefinition"),
    obj("TitlePartDefinition", "WeaponNamePartDefinition"),
    int("GameStage"),
    int("UniqueId"),
];

/// `WillowItemTypes.ItemDefinitionData` in declaration order (read from `WillowGame.upk`, see the module doc).
pub const ITEM_DEFINITION_DATA: [FieldDecl; FIELD_COUNT] = [
    obj("ItemDefinition", "ItemDefinition"),
    obj("BalanceDefinition", "InventoryBalanceDefinition"),
    obj("ManufacturerDefinition", "ManufacturerDefinition"),
    int("ManufacturerGradeIndex"),
    obj("AlphaItemPartDefinition", "ItemPartDefinition"),
    obj("BetaItemPartDefinition", "ItemPartDefinition"),
    obj("GammaItemPartDefinition", "ItemPartDefinition"),
    obj("DeltaItemPartDefinition", "ItemPartDefinition"),
    obj("EpsilonItemPartDefinition", "ItemPartDefinition"),
    obj("ZetaItemPartDefinition", "ItemPartDefinition"),
    obj("EtaItemPartDefinition", "ItemPartDefinition"),
    obj("ThetaItemPartDefinition", "ItemPartDefinition"),
    obj("MaterialItemPartDefinition", "ItemPartDefinition"),
    obj("PrefixItemNamePartDefinition", "ItemNamePartDefinition"),
    obj("TitleItemNamePartDefinition", "ItemNamePartDefinition"),
    int("GameStage"),
    int("UniqueId"),
];

/// Which of the two structs an item's definition data is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefinitionStruct {
    /// `WillowWeaponTypes.WeaponDefinitionData`.
    Weapon,
    /// `WillowItemTypes.ItemDefinitionData`.
    Item,
}

impl DefinitionStruct {
    /// The struct's name as the package declares it.
    pub fn name(self) -> &'static str {
        match self {
            DefinitionStruct::Weapon => "WeaponDefinitionData",
            DefinitionStruct::Item => "ItemDefinitionData",
        }
    }

    /// The object the struct is declared in (`WillowGame.upk`).
    pub fn outer(self) -> &'static str {
        match self {
            DefinitionStruct::Weapon => "WillowWeaponTypes",
            DefinitionStruct::Item => "WillowItemTypes",
        }
    }

    /// The struct's fields in declaration order.
    pub fn fields(self) -> &'static [FieldDecl; FIELD_COUNT] {
        match self {
            DefinitionStruct::Weapon => &WEAPON_DEFINITION_DATA,
            DefinitionStruct::Item => &ITEM_DEFINITION_DATA,
        }
    }
}

/// The kind of an item. The registry (W4-REGISTRY) decides the kind from the definition's class, as
/// `tools/item_registry.py` does (`definition_kind`); this type only names the kinds.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    Weapon,
    Shield,
    GrenadeMod,
    ClassMod,
    Relic,
    Usable,
    Customization,
    MissionItem,
    MissionWeapon,
}

impl ItemKind {
    /// Every kind, in declaration order.
    pub const ALL: [ItemKind; 9] = [
        ItemKind::Weapon,
        ItemKind::Shield,
        ItemKind::GrenadeMod,
        ItemKind::ClassMod,
        ItemKind::Relic,
        ItemKind::Usable,
        ItemKind::Customization,
        ItemKind::MissionItem,
        ItemKind::MissionWeapon,
    ];

    /// The definition-data struct items of this kind carry.
    pub fn definition_struct(self) -> DefinitionStruct {
        match self {
            ItemKind::Weapon | ItemKind::MissionWeapon => DefinitionStruct::Weapon,
            ItemKind::Shield
            | ItemKind::GrenadeMod
            | ItemKind::ClassMod
            | ItemKind::Relic
            | ItemKind::Usable
            | ItemKind::Customization
            | ItemKind::MissionItem => DefinitionStruct::Item,
        }
    }
}

macro_rules! slot_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $field:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum $name { $($variant),+ }

        impl $name {
            /// Every slot in the game's slot order (the struct's declaration order).
            pub const ALL: [$name; SLOT_COUNT] = [$($name::$variant),+];

            /// The slot's position, 0-based, in the game's slot order.
            pub fn index(self) -> usize {
                self as usize
            }

            /// The slot at `index` in the game's slot order, or `None` past the last slot.
            pub fn from_index(index: usize) -> Option<Self> {
                Self::ALL.get(index).copied()
            }

            /// The struct field that holds this slot's part.
            pub fn field_name(self) -> &'static str {
                match self { $($name::$variant => $field),+ }
            }
        }
    };
}

slot_enum! {
    /// A part slot of `WeaponDefinitionData`.
    WeaponSlot {
        Body => "BodyPartDefinition",
        Grip => "GripPartDefinition",
        Barrel => "BarrelPartDefinition",
        Sight => "SightPartDefinition",
        Stock => "StockPartDefinition",
        Elemental => "ElementalPartDefinition",
        Accessory1 => "Accessory1PartDefinition",
        Accessory2 => "Accessory2PartDefinition",
        Material => "MaterialPartDefinition",
    }
}

slot_enum! {
    /// A part slot of `ItemDefinitionData`.
    ItemSlot {
        Alpha => "AlphaItemPartDefinition",
        Beta => "BetaItemPartDefinition",
        Gamma => "GammaItemPartDefinition",
        Delta => "DeltaItemPartDefinition",
        Epsilon => "EpsilonItemPartDefinition",
        Zeta => "ZetaItemPartDefinition",
        Eta => "EtaItemPartDefinition",
        Theta => "ThetaItemPartDefinition",
        Material => "MaterialItemPartDefinition",
    }
}

/// A field value in struct form: an object reference (by path, `None` for a null reference) or an int.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldValue {
    Object(Option<ObjectPath>),
    Int(i32),
}

/// Why definition data could not be built or read.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataError {
    NotYet(NotYet),
    /// The struct form holds `got` values, not [`FIELD_COUNT`].
    FieldCount {
        got: usize,
    },
    /// The value for `field` is not of the declared type.
    FieldType {
        field: &'static str,
    },
    /// `field` is a null reference, but [`DefinitionData`] requires it (the definition and the balance).
    RequiredNull {
        field: &'static str,
    },
    /// A part was asked for with the slot type of the other struct.
    WrongStruct {
        kind: ItemKind,
    },
    /// A `QuickSelectSlot` value outside 0..=4.
    QuickSlot {
        value: i32,
    },
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::NotYet(n) => write!(f, "{n}"),
            DataError::FieldCount { got } => write!(
                f,
                "definition data has {got} fields, expected {FIELD_COUNT}"
            ),
            DataError::FieldType { field } => {
                write!(f, "definition data field {field} has the wrong type")
            }
            DataError::RequiredNull { field } => {
                write!(f, "definition data field {field} is a null reference")
            }
            DataError::WrongStruct { kind } => {
                write!(f, "{kind:?} items do not have this slot type")
            }
            DataError::QuickSlot { value } => write!(f, "quick slot {value} is outside 0..=4"),
        }
    }
}

impl std::error::Error for DataError {}

impl From<NotYet> for DataError {
    fn from(n: NotYet) -> Self {
        DataError::NotYet(n)
    }
}

/// An item's identity: the definition-data struct of its kind, with typed fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionData {
    pub kind: ItemKind,
    /// `BalanceDefinition`.
    pub balance: ObjectPath,
    /// `WeaponTypeDefinition` for weapon kinds, `ItemDefinition` for the others.
    pub definition: ObjectPath,
    /// `ManufacturerDefinition`.
    pub manufacturer: Option<ObjectPath>,
    /// `ManufacturerGradeIndex`: the item level (verified live, see the module doc).
    pub manufacturer_grade: i32,
    /// The nine part slots, indexed by [`WeaponSlot::index`] or [`ItemSlot::index`] according to the kind.
    pub parts: [Option<ObjectPath>; SLOT_COUNT],
    /// `PrefixPartDefinition` / `PrefixItemNamePartDefinition`.
    pub prefix: Option<ObjectPath>,
    /// `TitlePartDefinition` / `TitleItemNamePartDefinition`.
    pub title: Option<ObjectPath>,
    /// `GameStage`.
    pub game_stage: GameStage,
    /// `UniqueId`, carried as read (its writer is not read: UNVERIFIED).
    pub unique_id: i32,
}

impl DefinitionData {
    /// The struct this item's data is stored in.
    pub fn definition_struct(&self) -> DefinitionStruct {
        self.kind.definition_struct()
    }

    /// The part in a weapon slot; an error for a kind that uses `ItemDefinitionData`.
    pub fn weapon_part(&self, slot: WeaponSlot) -> Result<Option<&ObjectPath>, DataError> {
        match self.definition_struct() {
            DefinitionStruct::Weapon => Ok(self.parts[slot.index()].as_ref()),
            DefinitionStruct::Item => Err(DataError::WrongStruct { kind: self.kind }),
        }
    }

    /// The part in an item slot; an error for a kind that uses `WeaponDefinitionData`.
    pub fn item_part(&self, slot: ItemSlot) -> Result<Option<&ObjectPath>, DataError> {
        match self.definition_struct() {
            DefinitionStruct::Item => Ok(self.parts[slot.index()].as_ref()),
            DefinitionStruct::Weapon => Err(DataError::WrongStruct { kind: self.kind }),
        }
    }

    /// The struct form: one value per field of [`Self::definition_struct`], in declaration order.
    pub fn to_fields(&self) -> [FieldValue; FIELD_COUNT] {
        let mut out: [FieldValue; FIELD_COUNT] = std::array::from_fn(|_| FieldValue::Int(0));
        out[field::DEFINITION] = FieldValue::Object(Some(self.definition.clone()));
        out[field::BALANCE] = FieldValue::Object(Some(self.balance.clone()));
        out[field::MANUFACTURER] = FieldValue::Object(self.manufacturer.clone());
        out[field::MANUFACTURER_GRADE] = FieldValue::Int(self.manufacturer_grade);
        for (i, part) in self.parts.iter().enumerate() {
            out[field::FIRST_PART + i] = FieldValue::Object(part.clone());
        }
        out[field::PREFIX] = FieldValue::Object(self.prefix.clone());
        out[field::TITLE] = FieldValue::Object(self.title.clone());
        out[field::GAME_STAGE] = FieldValue::Int(self.game_stage.0);
        out[field::UNIQUE_ID] = FieldValue::Int(self.unique_id);
        out
    }

    /// Builds the data of a `kind` item from its struct form (one value per field, declaration order). The values'
    /// types must match the declaration; the definition and the balance must not be null.
    pub fn from_fields(kind: ItemKind, values: &[FieldValue]) -> Result<Self, DataError> {
        if values.len() != FIELD_COUNT {
            return Err(DataError::FieldCount { got: values.len() });
        }
        let decls = kind.definition_struct().fields();
        let object = |i: usize| match (&values[i], decls[i].ty) {
            (FieldValue::Object(v), FieldType::Object { .. }) => Ok(v.clone()),
            _ => Err(DataError::FieldType {
                field: decls[i].name,
            }),
        };
        let required = |i: usize| {
            object(i)?.ok_or(DataError::RequiredNull {
                field: decls[i].name,
            })
        };
        let int = |i: usize| match (&values[i], decls[i].ty) {
            (FieldValue::Int(v), FieldType::Int) => Ok(*v),
            _ => Err(DataError::FieldType {
                field: decls[i].name,
            }),
        };
        let mut parts: [Option<ObjectPath>; SLOT_COUNT] = Default::default();
        for (i, part) in parts.iter_mut().enumerate() {
            *part = object(field::FIRST_PART + i)?;
        }
        Ok(DefinitionData {
            kind,
            definition: required(field::DEFINITION)?,
            balance: required(field::BALANCE)?,
            manufacturer: object(field::MANUFACTURER)?,
            manufacturer_grade: int(field::MANUFACTURER_GRADE)?,
            parts,
            prefix: object(field::PREFIX)?,
            title: object(field::TITLE)?,
            game_stage: GameStage(int(field::GAME_STAGE)?),
            unique_id: int(field::UNIQUE_ID)?,
        })
    }
}

/// An inventory item's mark, the byte enum `WillowInventory.PlayerMark` (`Engine.upk`): `PM_Trash` 0, `PM_Standard`
/// 1, `PM_Favorite` 2, then `PM_MAX` 3. Any byte is kept, because the game writes 3 as a "new" mark
/// (`NATIVE_INVENTORY_EQUIP.md`, `AddBackpack*FromDefinitionData`; that reading is UNVERIFIED in the running game).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mark(pub u8);

impl Mark {
    pub const TRASH: Mark = Mark(0);
    pub const STANDARD: Mark = Mark(1);
    pub const FAVORITE: Mark = Mark(2);

    /// The enum value's name, `None` past the three declared marks.
    pub fn name(self) -> Option<&'static str> {
        match self.0 {
            0 => Some("PM_Trash"),
            1 => Some("PM_Standard"),
            2 => Some("PM_Favorite"),
            _ => None,
        }
    }
}

/// An item held by an inventory: its identity and the per-instance state the inventory keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryInstance {
    pub data: DefinitionData,
    pub quantity: i32,
    /// Readied (in a weapon slot or an equipped gear slot) rather than in the backpack.
    pub equipped: bool,
    pub mark: Mark,
    /// The weapon's `QuickSelectSlot`, 1..=4; `None` for 0, not slotted (`NATIVE_INVENTORY_EQUIP.md` item 3).
    pub quick_slot: Option<u8>,
}

impl InventoryInstance {
    /// Reads a `QuickSelectSlot` value: 0 is not slotted, 1..=4 a quick slot, anything else an error.
    pub fn quick_slot_from_field(value: i32) -> Result<Option<u8>, DataError> {
        match value {
            0 => Ok(None),
            1..=4 => Ok(Some(value as u8)),
            _ => Err(DataError::QuickSlot { value }),
        }
    }

    /// The `QuickSelectSlot` value of this instance (0 when not slotted).
    pub fn quick_slot_field(&self) -> i32 {
        self.quick_slot.map_or(0, i32::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(s: &str) -> ObjectPath {
        ObjectPath::parse(s).unwrap()
    }

    /// Invented paths and numbers, not game data.
    fn synthetic(kind: ItemKind) -> DefinitionData {
        let mut parts: [Option<ObjectPath>; SLOT_COUNT] = Default::default();
        for (i, part) in parts.iter_mut().enumerate() {
            if i % 3 != 2 {
                *part = Some(path(&format!("Test_Items.Parts.Slot{i}")));
            }
        }
        DefinitionData {
            kind,
            balance: path("Test_Items.Balance.B1"),
            definition: path("Test_Items.Types.T1"),
            manufacturer: Some(path("Test_Items.Makers.M1")),
            manufacturer_grade: 13,
            parts,
            prefix: None,
            title: Some(path("Test_Items.Names.Title1")),
            game_stage: GameStage(11),
            unique_id: -123_456,
        }
    }

    #[test]
    fn weapon_slots_round_trip_in_game_order() {
        for (i, slot) in WeaponSlot::ALL.iter().enumerate() {
            assert_eq!(slot.index(), i);
            assert_eq!(WeaponSlot::from_index(i), Some(*slot));
            assert_eq!(
                slot.field_name(),
                WEAPON_DEFINITION_DATA[field::FIRST_PART + i].name
            );
        }
        assert_eq!(WeaponSlot::from_index(SLOT_COUNT), None);
        assert_eq!(WeaponSlot::Body.index(), 0);
        assert_eq!(WeaponSlot::Material.index(), 8);
    }

    #[test]
    fn item_slots_round_trip_in_game_order() {
        for (i, slot) in ItemSlot::ALL.iter().enumerate() {
            assert_eq!(slot.index(), i);
            assert_eq!(ItemSlot::from_index(i), Some(*slot));
            assert_eq!(
                slot.field_name(),
                ITEM_DEFINITION_DATA[field::FIRST_PART + i].name
            );
        }
        assert_eq!(ItemSlot::from_index(SLOT_COUNT), None);
        assert_eq!(ItemSlot::Alpha.index(), 0);
        assert_eq!(ItemSlot::Material.index(), 8);
    }

    #[test]
    fn declarations_have_the_same_shape() {
        for (w, i) in WEAPON_DEFINITION_DATA
            .iter()
            .zip(ITEM_DEFINITION_DATA.iter())
        {
            assert_eq!(
                matches!(w.ty, FieldType::Int),
                matches!(i.ty, FieldType::Int)
            );
        }
        let ints: Vec<usize> = (0..FIELD_COUNT)
            .filter(|&k| WEAPON_DEFINITION_DATA[k].ty == FieldType::Int)
            .collect();
        assert_eq!(
            ints,
            vec![
                field::MANUFACTURER_GRADE,
                field::GAME_STAGE,
                field::UNIQUE_ID
            ]
        );
        assert_eq!(WEAPON_DEFINITION_DATA[field::GAME_STAGE].name, "GameStage");
        assert_eq!(ITEM_DEFINITION_DATA[field::UNIQUE_ID].name, "UniqueId");
        assert_eq!(
            ITEM_DEFINITION_DATA[field::PREFIX].name,
            "PrefixItemNamePartDefinition"
        );
    }

    #[test]
    fn kinds_pick_their_struct() {
        let weapons: Vec<ItemKind> = ItemKind::ALL
            .into_iter()
            .filter(|k| k.definition_struct() == DefinitionStruct::Weapon)
            .collect();
        assert_eq!(weapons, vec![ItemKind::Weapon, ItemKind::MissionWeapon]);
        assert_eq!(DefinitionStruct::Weapon.name(), "WeaponDefinitionData");
        assert_eq!(DefinitionStruct::Item.outer(), "WillowItemTypes");
    }

    #[test]
    fn synthetic_data_round_trips_through_struct_order() {
        for kind in ItemKind::ALL {
            let data = synthetic(kind);
            let fields = data.to_fields();
            assert_eq!(
                fields[field::DEFINITION],
                FieldValue::Object(Some(path("Test_Items.Types.T1")))
            );
            assert_eq!(
                fields[field::BALANCE],
                FieldValue::Object(Some(path("Test_Items.Balance.B1")))
            );
            assert_eq!(fields[field::MANUFACTURER_GRADE], FieldValue::Int(13));
            assert_eq!(fields[field::FIRST_PART + 2], FieldValue::Object(None));
            assert_eq!(fields[field::PREFIX], FieldValue::Object(None));
            assert_eq!(fields[field::GAME_STAGE], FieldValue::Int(11));
            assert_eq!(fields[field::UNIQUE_ID], FieldValue::Int(-123_456));
            for (value, decl) in fields.iter().zip(data.definition_struct().fields()) {
                let is_int = matches!(value, FieldValue::Int(_));
                assert_eq!(is_int, decl.ty == FieldType::Int, "{}", decl.name);
            }
            assert_eq!(DefinitionData::from_fields(kind, &fields), Ok(data));
        }
    }

    #[test]
    fn slot_access_follows_the_struct() {
        let weapon = synthetic(ItemKind::Weapon);
        assert_eq!(
            weapon.weapon_part(WeaponSlot::Grip),
            Ok(Some(&path("Test_Items.Parts.Slot1")))
        );
        assert_eq!(weapon.weapon_part(WeaponSlot::Barrel), Ok(None));
        assert_eq!(
            weapon.item_part(ItemSlot::Alpha),
            Err(DataError::WrongStruct {
                kind: ItemKind::Weapon
            })
        );
        let shield = synthetic(ItemKind::Shield);
        assert_eq!(
            shield.item_part(ItemSlot::Theta),
            Ok(Some(&path("Test_Items.Parts.Slot7")))
        );
        assert_eq!(
            shield.weapon_part(WeaponSlot::Body),
            Err(DataError::WrongStruct {
                kind: ItemKind::Shield
            })
        );
    }

    #[test]
    fn malformed_struct_forms_are_refused() {
        let fields = synthetic(ItemKind::Relic).to_fields();
        assert_eq!(
            DefinitionData::from_fields(ItemKind::Relic, &fields[..16]),
            Err(DataError::FieldCount { got: 16 })
        );
        let mut wrong = fields.clone();
        wrong[field::GAME_STAGE] = FieldValue::Object(None);
        assert_eq!(
            DefinitionData::from_fields(ItemKind::Relic, &wrong),
            Err(DataError::FieldType { field: "GameStage" })
        );
        let mut wrong = fields.clone();
        wrong[field::FIRST_PART] = FieldValue::Int(1);
        assert_eq!(
            DefinitionData::from_fields(ItemKind::Relic, &wrong),
            Err(DataError::FieldType {
                field: "AlphaItemPartDefinition"
            })
        );
        let mut null = synthetic(ItemKind::Weapon).to_fields();
        null[field::DEFINITION] = FieldValue::Object(None);
        assert_eq!(
            DefinitionData::from_fields(ItemKind::Weapon, &null),
            Err(DataError::RequiredNull {
                field: "WeaponTypeDefinition"
            })
        );
        let mut null = fields;
        null[field::BALANCE] = FieldValue::Object(None);
        assert_eq!(
            DefinitionData::from_fields(ItemKind::Relic, &null),
            Err(DataError::RequiredNull {
                field: "BalanceDefinition"
            })
        );
    }

    #[test]
    fn marks_and_quick_slots() {
        assert_eq!(Mark::TRASH.name(), Some("PM_Trash"));
        assert_eq!(Mark::STANDARD.name(), Some("PM_Standard"));
        assert_eq!(Mark::FAVORITE.name(), Some("PM_Favorite"));
        assert_eq!(Mark(3).name(), None);

        assert_eq!(InventoryInstance::quick_slot_from_field(0), Ok(None));
        assert_eq!(InventoryInstance::quick_slot_from_field(4), Ok(Some(4)));
        assert_eq!(
            InventoryInstance::quick_slot_from_field(5),
            Err(DataError::QuickSlot { value: 5 })
        );
        assert_eq!(
            InventoryInstance::quick_slot_from_field(-1),
            Err(DataError::QuickSlot { value: -1 })
        );
        let mut held = InventoryInstance {
            data: synthetic(ItemKind::Weapon),
            quantity: 1,
            equipped: true,
            mark: Mark::STANDARD,
            quick_slot: Some(2),
        };
        assert_eq!(held.quick_slot_field(), 2);
        held.quick_slot = None;
        assert_eq!(held.quick_slot_field(), 0);
    }
}
