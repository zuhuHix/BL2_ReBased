//! Owner: lane W4-ROLL. The balance merge readings and the values the roll reads that the registry does not hold.
//!
//! **Two readings of a balance chain** ([`Reading`]). The registry keeps, per balance, the manufacturer list and the
//! inventory definition of the leaf-most step that sets them (as `prepare_vendors` and `item_roll.py` read it), and
//! writes the root-most value too when it differs (`manufacturers_root_first`, `inventory_definition_root_first`).
//! `weapon_recipe.merge` takes the root-most step. Both readings stay named functions here ([`leaf_most`],
//! [`root_most`]); [`reading_for`] chooses one per record, the one its oracle uses: root-most for a weapon balance
//! (`weapon_recipe.merge`), leaf-most for every other kind (`item_roll.py`). ITEM_REGISTRY.md section 4 counts where
//! they differ on the installed game: 12 class mod and 8 usable balances (manufacturer lists) and 1 usable balance
//! (definition), no weapon balance. Which one the game uses is not read (UNVERIFIED either way; NATIVE_LOOT.md's
//! `GetInventoryDefinitionForManufacturerGrade` answers from the balance itself first, which is the leaf-most step).
//! The part lists themselves are the registry's merge (definition lists, then each step's collection, root first).
//!
//! **The values** ([`RollData`]). The roll reads, besides the registry's records, the raw properties of balances,
//! definitions, parts, name parts, attributes, resolvers and the globals: `bInterpolateExpLevel`,
//! `bAllowInventoryDefToModifyPartWeight`, `AttributeSlotEffects`, `AttributeSlotUpgrades`, `BaseRarity`, `Rarity`,
//! `RarityLevelColors`, name parts' `PartName`, level window, `Priority` and `Expressions`, and the damage-type
//! selectors. The oracles read them from each object's tag stream (`ow-package --properties ... --property-offset 4
//! --array-schema <item_roll's schema lines>`), with the class defaults they assume where a property is untagged
//! (`Priority` 1, level window 1..100, `bInterpolateExpLevel` true). [`RollData`] reads them the same way through the
//! registry's [`TagLookup`] (the same owner rule and decoder as the registry, decision 2026-11-15-15) with
//! [`SCHEMA_LINES`], a superset of the registry's, so the census can be compared with the oracle. Reading them from
//! the definition store's heap objects (class defaults applied by the store) is open.

use crate::ItemError;
use crate::registry::{CensusInput, Pv, TagLookup};
pub use crate::registry::{Props, Raises};

/// `item_roll.schema_lines()`: `item_registry.SCHEMA_LINES`, then `prepare_stock_shield.SCHEMA_EXTRA`, then
/// `weapon_stats.SCHEMA_LINES`, one line per property name (the first wins), in that order.
pub const SCHEMA_LINES: &[&str] = &[
    "WeightedParts=StructProperty:WeightedPart",
    "ConsolidatedAttributeInitData=StructProperty:AttributeInitializationData",
    "Manufacturers=StructProperty:ManufacturerGradeData",
    "TitleList=ObjectProperty",
    "PrefixList=ObjectProperty",
    "Expressions=StructProperty:AttributeExpressionData",
    "ValueResolverChain=ObjectProperty",
    "DamageTypes=StructProperty:DamageTypeSelectorData",
    "BalancedItems=StructProperty:BalancedInventoryData",
    "ItemPools=StructProperty:ItemPoolInfo",
    "DefaultItemPoolList=StructProperty:ItemPoolInfo",
    "DefaultItemPoolIncludedLists=ObjectProperty",
    "CustomItemPoolList=StructProperty:ItemPoolInfo",
    "CustomItemPoolIncludedLists=ObjectProperty",
    "PlayThroughs=StructProperty:PlayThroughData",
    "ConditionalExpressionList=StructProperty:ConditionalExpression",
    "DefaultLoot=StructProperty:LootConfigurationData",
    "ItemAttachments=StructProperty:LootAttachmentData",
    "ExternalAttributeEffects=StructProperty:AttributeEffectData",
    "ClassModDefinitions=ObjectProperty",
    "Grades=StructProperty:GradeData",
    "AttributeSlotEffects=StructProperty:AttributeSlotEffectData",
    "AttributeSlotUpgrades=StructProperty:AttributeSlotUpgradeData",
    "WeaponAttributeEffects=StructProperty:AttributeEffectData",
    "CustomPresentations=ObjectProperty",
    "WeaponCardAttributes=StructProperty:WeaponCardAttributeData",
    "WeaponCardPresentations=ObjectProperty",
    "DamageSurfaceChanceModifiers=StructProperty:StatusEffectChanceModifier",
    "RarityLevelColors=StructProperty:RarityLevelColor",
    "MappingData=StructProperty:WeaponStatusEffectPresentationMapping",
];

// ---- the two readings ----------------------------------------------------------------------------------------------

/// Which step of a balance chain gives its manufacturers and inventory definition (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// The leaf-most step that sets them: the registry's own fields, `item_roll.py`, `prepare_vendors`.
    LeafMost,
    /// The root-most step that sets them: `weapon_recipe.merge`.
    RootMost,
}

/// A balance's manufacturers (in list order, `None` for an entry naming none) and inventory definition under one
/// reading.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceChoice {
    pub manufacturers: Vec<Option<String>>,
    pub definition: Option<String>,
}

fn text(v: &Pv) -> Option<String> {
    v.as_str().map(str::to_string)
}

/// The leaf-most reading: the registry record's `manufacturers` and `inventory_definition`.
pub fn leaf_most(record: &Pv) -> BalanceChoice {
    BalanceChoice {
        manufacturers: record
            .at("manufacturers")
            .items()
            .iter()
            .map(|m| text(m.at("manufacturer")))
            .collect(),
        definition: text(record.at("inventory_definition")),
    }
}

/// The root-most reading: the record's `manufacturers_root_first` and `inventory_definition_root_first` where the
/// registry wrote them (it does only where they differ from the leaf-most values), else the leaf-most values.
pub fn root_most(record: &Pv) -> BalanceChoice {
    let leaf = leaf_most(record);
    BalanceChoice {
        manufacturers: match record.get("manufacturers_root_first") {
            Some(list) => list.items().iter().map(text).collect(),
            None => leaf.manufacturers,
        },
        definition: match record.get("inventory_definition_root_first") {
            Some(v) => text(v),
            None => leaf.definition,
        },
    }
}

/// The weapon balance classes (`item_registry.WEAPON_BALANCES`).
pub const WEAPON_BALANCE_CLASSES: [&str; 2] =
    ["WeaponBalanceDefinition", "MissionWeaponBalanceDefinition"];

/// Whether the record is a weapon balance's (by its class, as the registry classifies it).
pub fn is_weapon_balance(record: &Pv) -> bool {
    record
        .at("class")
        .as_str()
        .is_some_and(|c| WEAPON_BALANCE_CLASSES.contains(&c))
}

/// The reading the record's oracle uses (module documentation).
pub fn reading_for(record: &Pv) -> Reading {
    if is_weapon_balance(record) {
        Reading::RootMost
    } else {
        Reading::LeafMost
    }
}

/// The record's values under `reading`.
pub fn choice(record: &Pv, reading: Reading) -> BalanceChoice {
    match reading {
        Reading::LeafMost => leaf_most(record),
        Reading::RootMost => root_most(record),
    }
}

/// (manufacturers differ, definition differs) between the two readings of one record.
pub fn readings_differ(record: &Pv) -> (bool, bool) {
    let (leaf, root) = (leaf_most(record), root_most(record));
    (
        leaf.manufacturers != root.manufacturers,
        leaf.definition != root.definition,
    )
}

// ---- the values ----------------------------------------------------------------------------------------------------

/// The raw properties the roll reads (module documentation): the registry's [`TagLookup`] over the same census input
/// as the registry, read with [`SCHEMA_LINES`].
pub struct RollData {
    lookup: TagLookup,
}

impl std::fmt::Debug for RollData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RollData").finish_non_exhaustive()
    }
}

/// The last component of a class path.
pub fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

impl RollData {
    /// The lookup over `input` (the registry's census input, [`CensusInput::from_store`]).
    pub fn new(input: &CensusInput) -> Result<RollData, ItemError> {
        let lookup = TagLookup::new(input.clone(), SCHEMA_LINES)
            .map_err(|e| ItemError::BadData(format!("the roll's schema lines: {e}")))?;
        Ok(RollData { lookup })
    }

    /// Whether a package holds `path` (`path in census.owner`).
    pub fn holds(&self, path: &str) -> bool {
        self.lookup.holds(path)
    }

    /// The class of `path`'s owner export (`census.classes.get(path, '')`).
    pub fn class_of(&self, path: &str) -> String {
        self.lookup.class_of(path)
    }

    /// The paths below `prefix.` whose class ends with `class_suffix`, in path order.
    pub fn below_of_class(&self, prefix: &str, class_suffix: &str) -> Vec<String> {
        self.lookup
            .below(prefix)
            .into_iter()
            .filter(|(_, c)| c.ends_with(class_suffix))
            .map(|(p, _)| p)
            .collect()
    }

    /// `census.props(path)`: the decoded properties, or why there are none.
    pub fn props(&self, path: &str) -> Result<Props, String> {
        self.lookup.props(path)
    }

    /// `census.get(path)`: the decoded properties, or `None`.
    pub fn get(&self, path: &str) -> Option<Props> {
        self.lookup.get(path)
    }

    /// `weapon_recipe.attribute_value(lookup, init, level, attributes)` (the registry's reading).
    pub fn attribute_value(
        &self,
        init: &Pv,
        attributes: &[(String, f64)],
    ) -> Result<Option<f64>, Raises> {
        self.lookup.attribute_value(init, attributes)
    }

    /// `item_registry.attribute_constant` as a number.
    pub fn attribute_constant(&self, attribute: &Pv) -> Option<f64> {
        self.lookup.attribute_constant(attribute).number()
    }

    /// `item_roll.number`: a number as it is; an attribute init through [`RollData::attribute_value`], and, when
    /// that needs runtime state and the base is an attribute with a constant resolver, again with that constant as the
    /// attribute's value. `None` when it still needs runtime state or the tool would raise.
    pub fn number(&self, init: &Pv) -> Option<f64> {
        match init {
            Pv::Null => return None,
            Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => return init.number(),
            _ => {}
        }
        let value = self.attribute_value(init, &[]).ok()?;
        if value.is_some() {
            return value;
        }
        let attribute = init.at("BaseValueAttribute");
        if !attribute.truthy() {
            return None;
        }
        let constant = self.attribute_constant(attribute)?;
        let path = attribute.as_str()?.to_string();
        self.attribute_value(init, &[(path, constant)])
            .ok()
            .flatten()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn map(entries: Vec<(&str, Pv)>) -> Pv {
        Pv::Map(
            entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        )
    }

    pub(crate) fn s(v: &str) -> Pv {
        Pv::Str(v.to_string())
    }

    /// An invented balance record in the registry's shape.
    fn record(
        class: &str,
        leaf: &[&str],
        root: Option<&[&str]>,
        def: &str,
        root_def: Option<Option<&str>>,
    ) -> Pv {
        let makers = leaf
            .iter()
            .map(|m| map(vec![("manufacturer", s(m)), ("grades", Pv::List(vec![]))]))
            .collect();
        let mut fields = vec![
            ("class", s(class)),
            ("manufacturers", Pv::List(makers)),
            ("inventory_definition", s(def)),
        ];
        if let Some(root) = root {
            fields.push((
                "manufacturers_root_first",
                Pv::List(root.iter().map(|m| s(m)).collect()),
            ));
        }
        if let Some(d) = root_def {
            fields.push(("inventory_definition_root_first", d.map_or(Pv::Null, s)));
        }
        map(fields)
    }

    #[test]
    fn the_two_readings_are_kept_and_one_is_chosen_per_record() {
        // A class mod balance whose leaf lists manufacturer A and whose root lists B and C (the registry's
        // `manufacturers_root_first`), as the 12 class mod balances of ITEM_REGISTRY.md section 4 do.
        let class_mod = record(
            "ClassModBalanceDefinition",
            &["Test.MakerA"],
            Some(&["Test.MakerB", "Test.MakerC"]),
            "Test.Def",
            None,
        );
        assert_eq!(
            leaf_most(&class_mod).manufacturers,
            vec![Some("Test.MakerA".to_string())]
        );
        assert_eq!(
            root_most(&class_mod).manufacturers,
            vec![
                Some("Test.MakerB".to_string()),
                Some("Test.MakerC".to_string())
            ]
        );
        assert_eq!(readings_differ(&class_mod), (true, false));
        assert_eq!(reading_for(&class_mod), Reading::LeafMost);
        assert_eq!(
            choice(&class_mod, reading_for(&class_mod)),
            leaf_most(&class_mod)
        );

        // A usable balance whose root-most definition differs (the one usable balance of section 4), including a
        // root-first value of None.
        let usable = record(
            "InventoryBalanceDefinition",
            &["Test.MakerA"],
            None,
            "Test.LeafDef",
            Some(None),
        );
        assert_eq!(readings_differ(&usable), (false, true));
        assert_eq!(root_most(&usable).definition, None);

        // A weapon balance takes the root-most values (weapon_recipe.merge); without differing values both agree.
        let weapon = record(
            "WeaponBalanceDefinition",
            &["Test.MakerA"],
            Some(&["Test.MakerB"]),
            "Test.Type",
            Some(Some("Test.RootType")),
        );
        assert_eq!(reading_for(&weapon), Reading::RootMost);
        let chosen = choice(&weapon, reading_for(&weapon));
        assert_eq!(chosen.manufacturers, vec![Some("Test.MakerB".to_string())]);
        assert_eq!(chosen.definition, Some("Test.RootType".to_string()));
        let plain = record(
            "WeaponBalanceDefinition",
            &["Test.MakerA"],
            None,
            "Test.Type",
            None,
        );
        assert_eq!(readings_differ(&plain), (false, false));
        assert_eq!(leaf_most(&plain), root_most(&plain));
    }

    use ow_package::schema::SchemaTyper;
    use std::path::PathBuf;

    #[test]
    fn the_schema_holds_one_line_per_property() {
        let mut seen: Vec<&str> = Vec::new();
        for line in SCHEMA_LINES {
            let name = line.split('=').next().unwrap();
            assert!(!seen.contains(&name), "{name} twice");
            seen.push(name);
        }
        let mut typer = SchemaTyper::new_empty();
        typer.add_text(&SCHEMA_LINES.join("\n")).unwrap();
        assert_eq!(typer.len(), SCHEMA_LINES.len());
        // The registry's lines come first and unchanged (a superset of the registry's typing).
        assert_eq!(
            &SCHEMA_LINES[..crate::registry::SCHEMA_LINES.len()],
            crate::registry::SCHEMA_LINES
        );
    }

    #[test]
    fn attribute_values_follow_the_oracles_subset() {
        let data = RollData::new(&CensusInput::new(PathBuf::from("."))).unwrap();
        let k = |v: f64| {
            map(vec![
                ("BaseValueConstant", Pv::Float(v)),
                ("BaseValueScaleConstant", Pv::Float(2.0)),
            ])
        };
        assert_eq!(data.attribute_value(&k(1.5), &[]), Ok(Some(3.0)));
        // An attribute base needs runtime state unless a value is given for it.
        let a = map(vec![
            ("BaseValueAttribute", s("Test.Attr")),
            ("BaseValueScaleConstant", Pv::Float(1.0)),
        ]);
        assert_eq!(data.attribute_value(&a, &[]), Ok(None));
        assert_eq!(
            data.attribute_value(&a, &[("Test.Attr".to_string(), 4.0)]),
            Ok(Some(4.0))
        );
        // A definition no package holds raises; number() reads that as no value.
        let d = map(vec![("InitializationDefinition", s("Test.Missing"))]);
        assert_eq!(data.attribute_value(&d, &[]), Err(Raises));
        assert_eq!(data.number(&d), None);
        assert_eq!(data.number(&Pv::Int(3)), Some(3.0));
        assert_eq!(data.number(&Pv::Null), None);
    }
}
