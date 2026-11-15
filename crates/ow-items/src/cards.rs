//! Owner: lane W4-STATS. An item's card: the numbers as the card presents them, the text lines, the status rows,
//! rarity, sale value and the level line (architecture section 3.7). The numbers come from [`crate::stats`].
//!
//! **Reference** ([`weapon_card`]) reproduces the card of `tools/weapon_stats.evaluate` field for field, as
//! `tools/weapon_gen_parity.py` compares it (plus the accuracy pool, the refire rule and the projectile speed scale
//! the C++ generator carried). The rules and their status (docs/verification/NATIVE_WEAPON_RULES.md,
//! INVENTORY_CARD_STATS.md; UNVERIFIED in game unless stated):
//!
//! - **Presentation rounding** ([`present`]): `UAttributePresentationDefinition`'s rounding mode and `FloatPrecision`
//!   (clamped to 0..10) from the card's presentation objects; `ATTRROUNDING_Float` rounds half up in single precision
//!   (the x87 precision setting inferred from one card), the integer modes round, floor or ceil; values under 1e-8
//!   print as 0. Status rows round to one decimal (their presentations' data).
//! - **Accuracy**: `AttrPresent_WeaponSpread`'s linear remap of the spread, the input clamped to its range
//!   ([`remap_value`]).
//! - **Status chance mapping**: the card's chance row is the remap of the combined chance modifier by the presentation
//!   `WeapStatusEffectAttrPresent_CombinedStatusEffectChance` maps the damage type to (the first exact damage-type
//!   match, UNVERIFIED); the input clamps at the remap's upper bound, which is how slag's `[0, 3.33]` range clamps its
//!   row (the slag clamp). A damage type without a mapping keeps the Generic surface's `BaseChance` times the modifier.
//! - **Sale value**: the type's `MonetaryValue` calculator fed the part total and the level, truncated, capped by the
//!   currency (`crate::value::sale_value_cap`).
//! - **Level line**: a mission balance or a type without `bUsesPlayerLevelRequirement` requires 0; otherwise the level
//!   (the player's bonus taken as 0), at least 1; the line shows above 1 (read from script).
//! - **Text lines**: the title's red flavour line(s), the zoom and ammo lines, the custom presentations that show no
//!   number, then the card presentations; the red colour is the caller's ([`CardText::red`]: the ignored local colours
//!   file's `card_red_text`, or the neutral grey the tools use without it); the installed `.int` files override the
//!   cooked English strings ([`Localization`]).
//!
//! **Off mode** ([`instance_numbers`]) reads the numbers from the instance [`crate::stats::instantiate_stats`] built,
//! through `ow-attr`, and presents them with the same rounding.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use ow_attr::AttributeSystem;
use ow_attr::api::EvalCx;
use ow_core::fmt::fmt_f;
use ow_core::num::f32r;
use ow_core::{DetMap, OrderedMap};
use ow_object::ids::ObjRef;

use crate::ItemError;
use crate::registry::Pv;
use crate::stats::{
    ATTR, AccuracyPool, Stacks, StatsData, Values, WeaponAttributes, WeaponRecipe, accuracy_pool,
    instance_stat, price_init, short, weapon_attributes,
};
use crate::value::sale_value_cap;

/// `weapon_stats.ACCURACY_PRESENTATION`.
pub const ACCURACY_PRESENTATION: &str = "GD_AttributePresentation.Weapons.AttrPresent_WeaponSpread";
/// `weapon_stats.STATUS_CHANCE_MAPPING`.
pub const STATUS_CHANCE_MAPPING: &str =
    "GD_AttributePresentation.Weapons.WeapStatusEffectAttrPresent_CombinedStatusEffectChance";
/// `weapon_stats.ZOOM_PRESENTATION`.
pub const ZOOM_PRESENTATION: &str = "GD_AttributePresentation.Weapons.AttrPresent_WeaponZoomEndFOV";
/// `weapon_stats.SHOT_COST_PRESENTATION`.
pub const SHOT_COST_PRESENTATION: &str =
    "GD_AttributePresentation.Weapons.AttrPresent_WeaponShotCost";
/// The card presentation's zoom reference (`weapon_stats.CARD_ZOOM_REFERENCE_FOV`; UNVERIFIED against the native
/// resolver).
pub const CARD_ZOOM_REFERENCE_FOV: f64 = 80.0;
/// `weapon_stats.PRESENTATIONS`: a card field, its presentation and the rounding used when the presentation is absent.
pub const PRESENTATIONS: [(&str, &str, &str); 6] = [
    (
        "damage",
        "GD_AttributePresentation.Weapons.AttrPresent_WeaponDamage",
        "ATTRROUNDING_IntCeil",
    ),
    (
        "magazine",
        "GD_AttributePresentation.Weapons.AttrPresent_WeaponClipSize",
        "ATTRROUNDING_IntFloor",
    ),
    (
        "fire_rate",
        "GD_AttributePresentation.Weapons.AttrPresent_WeaponFireRate",
        "ATTRROUNDING_Float",
    ),
    (
        "reload_time",
        "GD_AttributePresentation.Weapons.AttrPresent_WeaponReloadSpeed",
        "ATTRROUNDING_Float",
    ),
    ("accuracy", ACCURACY_PRESENTATION, "ATTRROUNDING_Float"),
    ("weapon_zoom", ZOOM_PRESENTATION, "ATTRROUNDING_Float"),
];
/// The status rows' rounding (`weapon_stats.STATUS_ROUNDING`, one decimal).
pub const STATUS_ROUNDING: &str = "ATTRROUNDING_Float";
/// `weapon_stats.PRICE_CALCULATORS_CHECKED`: the calculators whose output was compared with real cards.
pub const PRICE_CALCULATORS_CHECKED: [&str; 6] = [
    "GD_Economy.PriceCalc.Init_Gun_Shotguns_PriceCalculator",
    "GD_Economy.PriceCalc.Init_Gun_AssaultRifles_PriceCalculator",
    "GD_Economy.PriceCalc.Init_Gun_SMG_PriceCalculator",
    "GD_Economy.PriceCalc.Init_Gun_Pistols_PriceCalculator",
    "GD_Economy.PriceCalc.Init_Gun_SniperRifles_PriceCalculator",
    "GD_Economy.PriceCalc.Init_Gun_Launchers_PriceCalculator",
];
/// `weapon_stats.CURRENCIES`: `ECurrencyType` in `Engine.upk` order.
pub const CURRENCIES: [&str; 13] = [
    "CURRENCY_Credits",
    "CURRENCY_Eridium",
    "CURRENCY_SeraphCrystals",
    "CURRENCY_Reserved_A",
    "CURRENCY_Reserved_B",
    "CURRENCY_Reserved_C",
    "CURRENCY_Reserved_D",
    "CURRENCY_Reserved_E",
    "CURRENCY_Reserved_F",
    "CURRENCY_Reserved_G",
    "CURRENCY_Reserved_H",
    "CURRENCY_Reserved_I",
    "CURRENCY_Reserved_J",
];
/// `weapon_stats.RARITY_NUMBER`: a `RarityRating` on the host's 1..5 scale.
pub const RARITY_NUMBER: [(&str, i64); 5] = [
    ("RARITY_Common", 1),
    ("RARITY_Uncommon", 2),
    ("RARITY_Rare", 3),
    ("RARITY_VeryRare", 4),
    ("RARITY_Legendary", 5),
];
/// `weapon_stats.ELEMENTS_BY_DAMAGE_TYPE`.
pub const ELEMENTS_BY_DAMAGE_TYPE: [(&str, &str); 4] = [
    ("DAMAGE_TYPE_Explosive", "Explosive"),
    ("DAMAGE_TYPE_Incindiary", "Fire"),
    ("DAMAGE_TYPE_Shock", "Shock"),
    ("DAMAGE_TYPE_Corrosive", "Corrosive"),
];
/// The balance class whose weapons require level 0 (`weapon_stats.MISSION_BALANCE_CLASS`).
pub const MISSION_BALANCE_CLASS: &str = "MissionWeaponBalanceDefinition";
/// The neutral grey `tools/game_colours.py` uses for `card_red_text` without the ignored local colours file (plainly
/// not a colour the cards use, so no line counts as red).
pub const NEUTRAL_RED_TEXT: [u8; 3] = [0x90, 0x90, 0x90];
const WHITE: &str = "#FFFFFF";

// ---- numbers -------------------------------------------------------------------------------------------------------

/// A card number as Python holds it: an `int` or a `float` (the comparison checks the kind).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num {
    Int(i64),
    Float(f64),
}

impl Num {
    pub fn to_pv(self) -> Pv {
        match self {
            Num::Int(i) => Pv::Int(i),
            Num::Float(x) => Pv::Float(x),
        }
    }

    pub fn as_f64(self) -> f64 {
        match self {
            // A card integer is a rounded stat, far below 2^53: exact.
            Num::Int(i) => i as f64,
            Num::Float(x) => x,
        }
    }
}

/// `math.floor` / `math.ceil` to an integer: `BadData` where Python raises (not finite) or the value leaves `i64`.
fn to_int(x: f64, what: &str) -> Result<i64, ItemError> {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0;
    if !x.is_finite() || !(-LIMIT..LIMIT).contains(&x) {
        return Err(ItemError::BadData(format!(
            "{what}: {} has no integer value",
            ow_core::fmt::fmt_py_repr(x)
        )));
    }
    // Integral (the caller floors or ceils) and in range: exact.
    Ok(x as i64)
}

/// `weapon_stats.half_up`: `floor(f32(f32(x * 10^digits) + 0.5)) / 10^digits`, each step in single precision.
pub fn half_up(x: f64, digits: i64) -> Result<f64, ItemError> {
    let mut ten = 1.0f64;
    for _ in 0..digits {
        ten *= 10.0;
    }
    let scale = f32r(ten);
    let floored = f32r(f32r(x * scale) + 0.5).floor();
    // The floored number is in i64 range (to_int checked it) and far below 2^53 for a card: it converts exactly.
    Ok(to_int(floored, "a card number")? as f64 / scale)
}

/// `weapon_stats.present`: a stat as the card prints it (module documentation).
pub fn present(value: f64, rounding: &str, precision: i64) -> Result<Num, ItemError> {
    let mut v = f32r(value);
    if v.abs() < 1e-8 {
        v = 0.0;
    }
    Ok(match rounding {
        "ATTRROUNDING_IntCeil" => Num::Int(to_int(v.ceil(), "a card number")?),
        "ATTRROUNDING_IntFloor" => Num::Int(to_int(v.floor(), "a card number")?),
        "ATTRROUNDING_IntRound" => Num::Int(to_int((v + 0.5).floor(), "a card number")?),
        _ => Num::Float(half_up(v, precision.clamp(0, 10))?),
    })
}

/// `weapon_stats.presentation_rounding`: (mode, precision) per card field from the presentations a package holds.
pub fn presentation_rounding(
    stats: &StatsData<'_>,
) -> Result<DetMap<&'static str, (String, i64)>, ItemError> {
    let mut found = DetMap::new();
    for (field, path, _) in PRESENTATIONS {
        if !stats.holds(path) {
            continue;
        }
        let props = stats.props(path)?;
        let mode = props.at("RoundingMode");
        let mode = if mode.truthy() {
            mode.as_str().unwrap_or("").to_string()
        } else {
            "ATTRROUNDING_IntRound".to_string()
        };
        let precision = match props.at("FloatPrecision") {
            Pv::Null => 1,
            p => {
                let n = p
                    .number()
                    .ok_or_else(|| ItemError::BadData("FloatPrecision is not a number".into()))?;
                to_int(n.trunc(), "FloatPrecision")?
            }
        };
        found.insert(field, (mode, precision));
    }
    Ok(found)
}

fn rounding_of<'r>(
    rounding: &'r DetMap<&'static str, (String, i64)>,
    field: &str,
) -> (&'r str, i64) {
    match rounding.get(field) {
        Some((m, p)) => (m.as_str(), *p),
        None => (
            PRESENTATIONS
                .iter()
                .find(|(f, _, _)| *f == field)
                .map_or("ATTRROUNDING_Float", |(_, _, d)| *d),
            1,
        ),
    }
}

/// `RemappingData` as (input low, input high, output low, output high); an absent bound is 0.
pub fn remap_bounds(remap: &Pv) -> [f64; 4] {
    let at = |k: &str| {
        let v = remap.at(k).at("BaseValueConstant");
        if v.truthy() {
            v.number().unwrap_or(0.0)
        } else {
            0.0
        }
    };
    [
        at("InputValueMn"),
        at("InputValueMx"),
        at("OutputValueMn"),
        at("OutputValueMx"),
    ]
}

/// `weapon_stats.remap_value`: a presentation's linear remap, the input clamped; an empty input range gives the
/// output low.
pub fn remap_value(bounds: [f64; 4], value: f64) -> f64 {
    let [in_low, in_high, out_low, out_high] = bounds;
    if in_high <= in_low {
        return f32r(out_low);
    }
    let slope = f32r((out_high - out_low) / (in_high - in_low));
    let clamped = in_high.min(in_low.max(value));
    f32r((clamped - in_low) * slope + out_low)
}

/// `weapon_stats.status_chance_remaps`: the remap bounds per damage type from the mapping presentation (the first
/// entry of a damage type wins; only presentations with remapping enabled count).
pub fn status_chance_remaps(stats: &StatsData<'_>) -> Result<DetMap<String, [f64; 4]>, ItemError> {
    let mut found = DetMap::new();
    if !stats.holds(STATUS_CHANCE_MAPPING) {
        return Ok(found);
    }
    for row in stats
        .props(STATUS_CHANCE_MAPPING)?
        .at("MappingData")
        .items()
    {
        let (Some(damage_type), Some(presentation)) = (
            row.at("DamageType").as_str().filter(|s| !s.is_empty()),
            row.at("Presentation").as_str().filter(|s| !s.is_empty()),
        ) else {
            continue;
        };
        if found.contains_key(damage_type) {
            continue;
        }
        let props = stats.props(presentation)?;
        if props.at("bValueRemappingEnabled").truthy()
            && matches!(props.at("RemappingData"), Pv::Map(_))
        {
            found.insert(
                damage_type.to_string(),
                remap_bounds(props.at("RemappingData")),
            );
        }
    }
    Ok(found)
}

// ---- text ----------------------------------------------------------------------------------------------------------

/// The installed `.int` files that override cooked English strings (`skill_stats.read_localization`), read on first
/// use per package and kept.
pub struct Localization {
    game: PathBuf,
    files: RefCell<DetMap<String, DetMap<(String, String), String>>>,
}

impl std::fmt::Debug for Localization {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Localization")
            .field("game", &self.game)
            .finish_non_exhaustive()
    }
}

/// Python's `str.isspace` for one character (`\s` of a `str` pattern): Unicode whitespace plus the four
/// information separators `\x1c`..`\x1f`, which Rust's `char::is_whitespace` leaves out.
pub fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}

/// Python's `str.splitlines()` after universal newlines turned `\r\n` and `\r` into `\n`.
fn py_lines(text: &str) -> Vec<String> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = Vec::new();
    let mut line = String::new();
    for c in text.chars() {
        if matches!(
            c,
            '\n' | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            out.push(std::mem::take(&mut line));
        } else {
            line.push(c);
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

/// A UTF-16 file's text (Python's `utf-16` codec: a byte-order mark picks the order, little-endian without one);
/// `Err` where Python raises (an odd length, an unpaired surrogate).
fn utf16(bytes: &[u8]) -> Result<String, String> {
    let (big, body) = match bytes {
        [0xFF, 0xFE, rest @ ..] => (false, rest),
        [0xFE, 0xFF, rest @ ..] => (true, rest),
        rest => (false, rest),
    };
    if body.len() % 2 != 0 {
        return Err("a UTF-16 file of odd length".to_string());
    }
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|c| {
            if big {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16(&units).map_err(|e| e.to_string())
}

impl Localization {
    /// The `.int` files of the install at `game` (`<game>/WillowGame/Localization/INT/<Package>.int`).
    pub fn new(game: &Path) -> Self {
        Localization {
            game: game.to_path_buf(),
            files: RefCell::new(DetMap::new()),
        }
    }

    /// `skill_stats.read_localization`: `{(section, key): value}`, quotes around a value removed; a missing file is
    /// empty.
    pub fn read(path: &Path) -> Result<DetMap<(String, String), String>, ItemError> {
        let mut entries = DetMap::new();
        let Ok(bytes) = std::fs::read(path) else {
            return Ok(entries);
        };
        let text =
            utf16(&bytes).map_err(|e| ItemError::BadData(format!("{}: {e}", path.display())))?;
        let mut section: Option<String> = None;
        for line in py_lines(&text) {
            if line.starts_with('[') {
                let s: Vec<char> = py_strip(&line).chars().collect();
                let inner: String = if s.len() >= 2 {
                    s[1..s.len() - 1].iter().collect()
                } else {
                    String::new()
                };
                let name = match inner.rsplit_once(' ') {
                    Some((head, _)) => head.to_string(),
                    None => inner,
                };
                section = Some(name);
            } else if let Some(sec) = section.as_ref().filter(|s| !s.is_empty())
                && let Some((key, value)) = line.split_once('=')
            {
                let mut value = py_strip(value).to_string();
                let chars: Vec<char> = value.chars().collect();
                if chars.len() >= 2 && chars[0] == '"' && chars[chars.len() - 1] == '"' {
                    value = chars[1..chars.len() - 1].iter().collect();
                }
                entries.insert((sec.clone(), py_strip(key).to_string()), value);
            }
        }
        Ok(entries)
    }

    /// The override of `property` of the object at `path`, if its package's file has one.
    pub fn get(
        &self,
        stats: &StatsData<'_>,
        path: &str,
        property: &str,
    ) -> Result<Option<String>, ItemError> {
        let package = path.split('.').next().unwrap_or(path).to_string();
        if !self.files.borrow().contains_key(&package) {
            let file = self
                .game
                .join("WillowGame")
                .join("Localization")
                .join("INT")
                .join(format!("{package}.int"));
            let read = Localization::read(&file)?;
            self.files.borrow_mut().insert(package.clone(), read);
        }
        let key = (localization_key(stats, path)?, property.to_string());
        Ok(self
            .files
            .borrow()
            .get(&package)
            .and_then(|f| f.get(&key).cloned()))
    }
}

/// `skill_stats.localization_key`: an object path as its `.int` section name (the package dropped, the first object
/// inside a non-package outer joined with `:`).
pub fn localization_key(stats: &StatsData<'_>, path: &str) -> Result<String, ItemError> {
    let parts: Vec<&str> = path.split('.').collect();
    let Some(first) = parts.get(1) else {
        return Err(ItemError::BadData(format!(
            "{path}: no object inside the package"
        )));
    };
    let mut key = (*first).to_string();
    let mut joined = false;
    for i in 2..parts.len() {
        let outer = parts[..i].join(".");
        let class = stats.data().class_of(&outer);
        let class = if class.is_empty() {
            "Core.Package".to_string()
        } else {
            class
        };
        let sep = if !joined && class != "Core.Package" {
            ':'
        } else {
            '.'
        };
        joined = joined || sep == ':';
        key.push(sep);
        key.push_str(parts[i]);
    }
    Ok(key)
}

/// What the card text needs besides the package data: the red flavour colour and the installed overrides.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct CardText<'a> {
    /// `card_red_text` as R, G, B ([`NEUTRAL_RED_TEXT`] without the local colours file).
    pub red: [u8; 3],
    pub localization: Option<&'a Localization>,
}

impl<'a> CardText<'a> {
    pub fn new(red: [u8; 3], localization: Option<&'a Localization>) -> Self {
        CardText { red, localization }
    }
}

impl Default for CardText<'_> {
    fn default() -> Self {
        CardText {
            red: NEUTRAL_RED_TEXT,
            localization: None,
        }
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The `[name]` opening at `i` (`\[([A-Za-z][A-Za-z0-9_]*)\]`): the name and the index after `]`.
fn open_tag(s: &[char], i: usize) -> Option<(String, usize)> {
    if s.get(i) != Some(&'[') || !s.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut j = i + 1;
    while j < s.len() && is_name_char(s[j]) {
        j += 1;
    }
    if s.get(j) != Some(&']') {
        return None;
    }
    Some((s[i + 1..j].iter().collect(), j + 1))
}

/// `GAME_TEXT_TAG.finditer`: every `[name]...[-name]` span (lazy, non-overlapping, left to right) as (start, end,
/// name, inner).
fn game_tags(s: &[char]) -> Vec<(usize, usize, String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((name, body)) = open_tag(s, i) {
            let close: Vec<char> = format!("[-{name}]").chars().collect();
            let found =
                (body..=s.len().saturating_sub(close.len())).find(|&k| s[k..].starts_with(&close));
            if let Some(k) = found {
                out.push((i, k + close.len(), name, s[body..k].iter().collect()));
                i = k + close.len();
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `UNMATCHED_GAME_TAG.sub('', text)`: every `[name]` and `[-name]` removed.
fn drop_unmatched(text: &str) -> String {
    let s: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i] == '[' {
            let start = if s.get(i + 1) == Some(&'-') {
                i + 2
            } else {
                i + 1
            };
            if s.get(start).is_some_and(|c| c.is_ascii_alphabetic()) {
                let mut j = start;
                while j < s.len() && is_name_char(s[j]) {
                    j += 1;
                }
                if s.get(j) == Some(&']') {
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

/// `re.sub(r'<[^>]*>', '', text)`.
fn drop_markup(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find('<') {
        match rest[i..].find('>') {
            Some(j) => {
                out.push_str(&rest[..i]);
                rest = &rest[i + j + 1..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// `re.sub(r'\s+', ' ', text)`.
fn squeeze(text: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in text.chars() {
        if py_space(c) {
            if !space {
                out.push(' ');
            }
            space = true;
        } else {
            out.push(c);
            space = false;
        }
    }
    out
}

/// The `cp1252` characters `html.unescape` gives a numeric reference in 0x80..0x9F.
fn cp1252(n: u32) -> Option<char> {
    const TABLE: [u32; 32] = [
        0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160,
        0x2039, 0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013,
        0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
    ];
    let i = n.checked_sub(0x80)?;
    TABLE
        .get(usize::try_from(i).ok()?)
        .and_then(|c| char::from_u32(*c))
}

/// `html.unescape` for the references game text uses: numeric ones (with Python's replacements) and the named
/// `amp`, `lt`, `gt`, `quot`, `apos`, `nbsp` (the first five without `;` too, as HTML5 allows). Any other named
/// reference is kept as written (Python would replace the rest of the HTML5 table; none was met on the cards).
pub fn html_unescape(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string();
    }
    let s: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < s.len() {
        if s[i] != '&' {
            out.push(s[i]);
            i += 1;
            continue;
        }
        if s.get(i + 1) == Some(&'#') {
            let hex = matches!(s.get(i + 2), Some('x') | Some('X'));
            let start = if hex { i + 3 } else { i + 2 };
            let mut j = start;
            while j < s.len()
                && (if hex {
                    s[j].is_ascii_hexdigit()
                } else {
                    s[j].is_ascii_digit()
                })
            {
                j += 1;
            }
            if j > start {
                let digits: String = s[start..j].iter().collect();
                let n = u32::from_str_radix(&digits, if hex { 16 } else { 10 }).unwrap_or(u32::MAX);
                let end = if s.get(j) == Some(&';') { j + 1 } else { j };
                if n == 0 {
                    out.push('\u{FFFD}');
                } else if n == 0x0D {
                    out.push('\r');
                } else if let Some(c) = cp1252(n) {
                    out.push(c);
                } else if (0xD800..=0xDFFF).contains(&n) || n > 0x10FFFF {
                    out.push('\u{FFFD}');
                } else if (0x1..=0x8).contains(&n)
                    || (0xE..=0x1F).contains(&n)
                    || (0x7F..=0x9F).contains(&n)
                    || (0xFDD0..=0xFDEF).contains(&n)
                    || n == 0xB
                    || (n & 0xFFFE) == 0xFFFE
                {
                    // Python drops the invalid code points.
                } else if let Some(c) = char::from_u32(n) {
                    out.push(c);
                }
                i = end;
                continue;
            }
        }
        let named: [(&str, char, bool); 6] = [
            ("amp", '&', true),
            ("lt", '<', true),
            ("gt", '>', true),
            ("quot", '"', true),
            ("nbsp", '\u{a0}', true),
            ("apos", '\'', false),
        ];
        let rest: String = s[i + 1..].iter().take(8).collect();
        let mut done = false;
        for (name, c, legacy) in named {
            if rest.starts_with(&format!("{name};")) {
                out.push(c);
                i += name.len() + 2;
                done = true;
                break;
            }
            if legacy && rest.starts_with(name) {
                out.push(c);
                i += name.len() + 1;
                done = true;
                break;
            }
        }
        if !done {
            out.push('&');
            i += 1;
        }
    }
    out
}

/// `html.escape(text, quote=False)`.
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// `weapon_stats._plain_game_text`: style tags removed, whitespace squeezed, `;` made `,` (the HUD splits on it).
pub fn plain_game_text(text: &str) -> String {
    let s: Vec<char> = text.chars().collect();
    let mut kept = String::new();
    let mut cursor = 0;
    for (start, end, _, inner) in game_tags(&s) {
        kept.extend(&s[cursor..start]);
        kept.push_str(&inner);
        cursor = end;
    }
    kept.extend(&s[cursor..]);
    let text = drop_markup(&drop_unmatched(&kept));
    py_strip(&squeeze(&html_unescape(&text))).replace(';', ",")
}

/// `weapon_stats._markup_text`: the game text escaped, its named element tags as coloured spans.
fn markup_text(raw: &str, color: &str, tag_color: &str) -> String {
    let s: Vec<char> = raw.chars().collect();
    let mut out = String::new();
    let mut cursor = 0;
    for (start, end, name, inner) in game_tags(&s) {
        let before: String = s[cursor..start].iter().collect();
        out.push_str(&html_escape(&drop_unmatched(&before)));
        let tag = name.to_lowercase();
        let span = if ["explosive", "corrosive", "shock", "fire"].contains(&tag.as_str()) {
            tag_color
        } else {
            color
        };
        out.push_str(&format!(
            "<font color=\"{span}\">{}</font>",
            html_escape(&inner)
        ));
        cursor = end;
    }
    let after: String = s[cursor..].iter().collect();
    out.push_str(&html_escape(&drop_unmatched(&after)));
    out
}

/// One card text line (`weapon_stats.presentation_line`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TextLine {
    pub text: String,
    pub color: String,
    pub source_color: Pv,
    pub raw: String,
    pub tag_color: String,
}

impl TextLine {
    fn plain(text: String) -> TextLine {
        TextLine {
            raw: text.clone(),
            text,
            color: WHITE.to_string(),
            source_color: Pv::Null,
            tag_color: WHITE.to_string(),
        }
    }
}

fn white() -> Pv {
    let mut m = DetMap::new();
    for k in ["R", "G", "B"] {
        m.insert(k.to_string(), Pv::Int(255));
    }
    Pv::Map(m)
}

fn hex_color(color: &Pv) -> Result<String, ItemError> {
    let mut out = String::from("#");
    for k in ["R", "G", "B"] {
        let v = match color.get(k) {
            None => 255,
            Some(v) => {
                let n = v.number().ok_or_else(|| {
                    ItemError::BadData(format!("a colour channel {k} is not a number"))
                })?;
                to_int(n.trunc(), "a colour channel")?
            }
        };
        out.push_str(&format!("{v:02X}"));
    }
    Ok(out)
}

/// `weapon_stats._is_red`.
fn is_red(color: &Pv, red: [u8; 3]) -> bool {
    ["R", "G", "B"].iter().zip(red).all(|(k, v)| {
        color
            .get(k)
            .is_some_and(|c| c.number() == Some(f64::from(v)))
    })
}

/// `weapon_stats.presentation_line`: one text-only presentation, or `None` when its text is empty.
pub fn presentation_line(
    stats: &StatsData<'_>,
    path: &str,
    text: &CardText<'_>,
) -> Result<Option<TextLine>, ItemError> {
    let props = stats.props(path)?;
    let key = if props.at("NoConstraintText").truthy() {
        "NoConstraintText"
    } else {
        "Description"
    };
    let cooked = props.at(key);
    let raw = match text.localization {
        Some(l) => l
            .get(stats, path, key)?
            .map(Pv::Str)
            .unwrap_or_else(|| cooked.clone()),
        None => cooked.clone(),
    };
    let raw = match &raw {
        Pv::Str(s) => s.clone(),
        v if !v.truthy() => String::new(),
        other => {
            return Err(ItemError::BadData(format!(
                "{path}: a card text that is not a string: {}",
                other.to_json()
            )));
        }
    };
    let plain = plain_game_text(&raw);
    if plain.is_empty() {
        return Ok(None);
    }
    let source = props.at("TextColor");
    let source = if source.truthy() {
        source.clone()
    } else {
        white()
    };
    let line_color = if props.get("bEnableTextColor") == Some(&Pv::Bool(false)) {
        white()
    } else {
        source.clone()
    };
    Ok(Some(TextLine {
        text: plain,
        color: hex_color(&line_color)?,
        tag_color: hex_color(&source)?,
        source_color: source,
        raw: py_strip(&squeeze(&raw)).replace(';', ","),
    }))
}

/// `weapon_stats._presentation_sources`: the balance, the type, the parts, the prefix and the title, each once.
fn presentation_sources(recipe: &WeaponRecipe) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |p: &str| {
        if !p.is_empty() && !out.iter().any(|o| o == p) {
            out.push(p.to_string());
        }
    };
    if let Some(b) = &recipe.balance {
        push(b);
    }
    push(&recipe.weapon_type);
    for p in recipe.ordered_parts() {
        push(p);
    }
    for n in recipe.name_parts() {
        push(n);
    }
    out
}

/// `weapon_stats.shot_cost_text`: the ammo line for a shot cost above 1.
fn shot_cost_text(
    stats: &StatsData<'_>,
    shot_cost: Option<f64>,
) -> Result<Option<String>, ItemError> {
    let Some(cost) = shot_cost.filter(|c| *c > 1.0) else {
        return Ok(None);
    };
    let amount = if cost.fract() == 0.0 {
        to_int(cost.trunc(), "a shot cost")?.to_string()
    } else {
        ow_core::fmt::fmt_g(cost, 6)
    };
    let props = stats.optional(SHOT_COST_PRESENTATION)?;
    let text = |k: &str, default: &str| {
        let v = props
            .as_ref()
            .and_then(|p| p.at(k).as_str().map(plain_game_text))
            .unwrap_or_default();
        if v.is_empty() { default.to_string() } else { v }
    };
    Ok(Some(format!(
        "{} {amount} {}",
        text("Prefix", "Consumes"),
        text("Suffix", "ammo per shot.")
    )))
}

/// `weapon_stats.card_text_lines`: the card's text lines in order (module documentation).
pub fn card_text_lines(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    damage_type: Option<&str>,
    shot_cost: Option<f64>,
    zoom: Option<Num>,
    text: &CardText<'_>,
) -> Result<Vec<TextLine>, ItemError> {
    let mut sources = presentation_sources(recipe);
    if let Some(d) = damage_type.filter(|d| !d.is_empty())
        && !sources.iter().any(|s| s == d)
    {
        sources.push(d.to_string());
    }
    let mut seen: Vec<String> = Vec::new();
    let mut red: Vec<TextLine> = Vec::new();
    let mut custom: Vec<TextLine> = Vec::new();
    let add = |seen: &mut Vec<String>, target: &mut Vec<TextLine>, line: Option<TextLine>| {
        if let Some(line) = line
            && !seen.contains(&line.text)
        {
            seen.push(line.text.clone());
            target.push(line);
        }
    };
    for owner in &sources {
        let Some(props) = stats.optional(owner)? else {
            continue;
        };
        for path in props.at("CustomPresentations").items() {
            let Some(path) = path.as_str().filter(|p| !p.is_empty()) else {
                continue;
            };
            let Some(line) = presentation_line(stats, path, text)? else {
                continue;
            };
            if is_red(&line.source_color, text.red) {
                add(&mut seen, &mut red, Some(line));
            } else if stats.props(path)?.at("bDontDisplayNumber").truthy() {
                add(&mut seen, &mut custom, Some(line));
            }
        }
    }
    let mut ordered = red;
    if let Some(z) = zoom {
        ordered.push(TextLine::plain(format!(
            "{}x Weapon Zoom",
            fmt_f(z.as_f64(), 1)
        )));
    }
    if let Some(ammo) = shot_cost_text(stats, shot_cost)? {
        ordered.push(TextLine::plain(ammo));
    }
    ordered.extend(custom);
    for owner in &sources {
        let Some(props) = stats.optional(owner)? else {
            continue;
        };
        for path in props.at("WeaponCardPresentations").items() {
            let Some(path) = path.as_str().filter(|p| !p.is_empty()) else {
                continue;
            };
            if stats.props(path)?.at("bDontDisplayNumber").truthy() {
                let line = presentation_line(stats, path, text)?;
                add(&mut seen, &mut ordered, line);
            }
        }
    }
    Ok(ordered)
}

/// `weapon_stats.fun_stats_markup`: the Scaleform markup of the lines (never for browser HTML).
pub fn fun_stats_markup(lines: &[TextLine]) -> Option<String> {
    let text: Vec<String> = lines
        .iter()
        .map(|l| {
            format!(
                "\u{2022} <font color=\"{}\">{}</font>",
                l.color,
                markup_text(&l.raw, &l.color, &l.tag_color)
            )
        })
        .collect();
    let joined = text.join("\n");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

// ---- the card ------------------------------------------------------------------------------------------------------

/// The status rows of a damage type with a status effect (`weapon_stats.status_rows`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct StatusRows {
    pub status_effect: Pv,
    pub status_effect_definition: String,
    pub status_chance: Option<f64>,
    /// By surface (lower case, without `DMGSURFACE_`), in the tool's insertion order.
    pub status_chance_by_surface: OrderedMap<String, f64>,
    pub status_dps: Option<f64>,
    pub status_duration: Option<f64>,
}

/// A weapon's card (Reference: `weapon_stats.evaluate`'s card and the generator's extra rows).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct WeaponCard {
    pub name: String,
    pub level: i64,
    pub level_requirement: i64,
    pub level_line: bool,
    pub damage: Option<f64>,
    /// The `WeaponDamage` stack's terms (base, PreAdd, PostAdd, the two scale sides); `None` without a damage base.
    pub damage_stack: Option<[f64; 5]>,
    pub fire_rate: Option<f64>,
    pub reload_time: Option<f64>,
    pub magazine: Option<f64>,
    pub spread: Option<f64>,
    pub shot_cost: Option<f64>,
    pub spin_up: Option<f64>,
    pub spin_mode: Pv,
    pub spin_start_interval_scale: Pv,
    pub element: Option<String>,
    pub rarity: Option<i64>,
    pub rarity_level: i64,
    pub rarity_rating: Pv,
    pub rarity_color: Option<String>,
    pub manufacturer: Option<String>,
    pub accuracy: Option<f64>,
    pub sale_value: Option<i64>,
    pub sale_value_known: bool,
    pub weapon_zoom: Option<f64>,
    pub text_lines: Vec<TextLine>,
    pub projectiles: Option<i64>,
    pub firing_mode: Pv,
    pub projectile_speed: Option<f64>,
    pub damage_type: Option<String>,
    pub status: Option<StatusRows>,
    /// `display`: the numbers as the card prints them, in the tool's order.
    pub display: Vec<(&'static str, Num)>,
    pub accuracy_pool: Option<AccuracyPool>,
    pub burst_count: Option<f64>,
    pub burst_interval: Option<f64>,
    pub speed_multiplier: Option<f64>,
    /// The attribute operands the evaluation could not resolve.
    pub unresolved: Vec<String>,
}

fn opt_f(x: Option<f64>) -> Pv {
    x.map_or(Pv::Null, Pv::Float)
}

fn opt_s(x: &Option<String>) -> Pv {
    x.as_ref().map_or(Pv::Null, |s| Pv::Str(s.clone()))
}

fn map(entries: Vec<(&str, Pv)>) -> Pv {
    Pv::Map(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

impl WeaponCard {
    /// The card in the generator's JSON shape (`weapon_gen_parity.python_gun`'s card: every field of
    /// `weapon_stats.evaluate`'s card except `fun_stats_lines`, plus `accuracy_pool`, `burst_count`,
    /// `burst_interval`, `burst_known` and `speed_multiplier`).
    pub fn to_pv(&self) -> Pv {
        let fun_stats: Vec<&str> = self.text_lines.iter().map(|l| l.text.as_str()).collect();
        let fun_stats = fun_stats.join("; ");
        let mut entries = vec![
            ("name", Pv::Str(self.name.clone())),
            ("level", Pv::Int(self.level)),
            ("level_requirement", Pv::Int(self.level_requirement)),
            ("level_line", Pv::Bool(self.level_line)),
            ("damage", opt_f(self.damage)),
            (
                "damage_stack",
                self.damage_stack
                    .map_or(Pv::Null, |[base, pre, post, up, down]| {
                        map(vec![
                            ("base", Pv::Float(base)),
                            ("pre_add", Pv::Float(pre)),
                            ("post_add", Pv::Float(post)),
                            ("scale_up", Pv::Float(up)),
                            ("scale_down", Pv::Float(down)),
                        ])
                    }),
            ),
            ("fire_rate", opt_f(self.fire_rate)),
            ("reload_time", opt_f(self.reload_time)),
            ("magazine", opt_f(self.magazine)),
            ("spread", opt_f(self.spread)),
            ("shot_cost", opt_f(self.shot_cost)),
            ("spin_up", opt_f(self.spin_up)),
            ("spin_mode", self.spin_mode.clone()),
            (
                "spin_start_interval_scale",
                self.spin_start_interval_scale.clone(),
            ),
            ("element", opt_s(&self.element)),
            ("rarity", self.rarity.map_or(Pv::Null, Pv::Int)),
            ("rarity_level", Pv::Int(self.rarity_level)),
            ("rarity_rating", self.rarity_rating.clone()),
            ("rarity_color", opt_s(&self.rarity_color)),
            ("manufacturer", opt_s(&self.manufacturer)),
            ("accuracy", opt_f(self.accuracy)),
            ("accuracy_known", Pv::Bool(true)),
            ("sale_value", self.sale_value.map_or(Pv::Null, Pv::Int)),
            ("sale_value_known", Pv::Bool(self.sale_value_known)),
            ("weapon_zoom", opt_f(self.weapon_zoom)),
            (
                "fun_stats",
                if fun_stats.is_empty() {
                    Pv::Null
                } else {
                    Pv::Str(fun_stats)
                },
            ),
            (
                "fun_stats_markup",
                fun_stats_markup(&self.text_lines).map_or(Pv::Null, Pv::Str),
            ),
            ("projectiles", self.projectiles.map_or(Pv::Null, Pv::Int)),
            ("firing_mode", self.firing_mode.clone()),
            ("projectile_speed", opt_f(self.projectile_speed)),
            ("damage_type", opt_s(&self.damage_type)),
            (
                "display",
                Pv::Map(
                    self.display
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_pv()))
                        .collect(),
                ),
            ),
            (
                "accuracy_pool",
                self.accuracy_pool.as_ref().map_or(Pv::Null, |p| {
                    map(vec![
                        ("min", opt_f(p.min)),
                        ("max", opt_f(p.max)),
                        ("idle_rate", opt_f(p.idle_rate)),
                    ])
                }),
            ),
            ("burst_count", opt_f(self.burst_count)),
            ("burst_interval", opt_f(self.burst_interval)),
            ("burst_known", Pv::Bool(self.burst_count.is_some())),
            ("speed_multiplier", opt_f(self.speed_multiplier)),
        ];
        if let Some(s) = &self.status {
            entries.extend([
                ("status_effect", s.status_effect.clone()),
                (
                    "status_effect_definition",
                    Pv::Str(s.status_effect_definition.clone()),
                ),
                ("status_chance", opt_f(s.status_chance)),
                (
                    "status_chance_by_surface",
                    Pv::Map(
                        s.status_chance_by_surface
                            .iter()
                            .map(|(k, v)| (k.clone(), Pv::Float(*v)))
                            .collect(),
                    ),
                ),
                ("status_dps", opt_f(s.status_dps)),
                ("status_duration", opt_f(s.status_duration)),
            ]);
        }
        map(entries)
    }

    /// The `fun_stats_lines` field the generator leaves out: each line's text and colour.
    pub fn fun_stats_lines(&self) -> Pv {
        Pv::List(
            self.text_lines
                .iter()
                .map(|l| {
                    map(vec![
                        ("text", Pv::Str(l.text.clone())),
                        ("color", Pv::Str(l.color.clone())),
                    ])
                })
                .collect(),
        )
    }
}

/// `weapon_recipe.weapon_damage_type`: the elemental part's `CustomDamageTypeDefinition`, else any part's in slot
/// order, else the type's `DefaultDamageTypeDefinition` (the card's rule; the name rule reads only the elemental part).
pub fn damage_type_of(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
) -> Result<Option<String>, ItemError> {
    let mut ordered: Vec<&(String, String)> = recipe
        .parts
        .iter()
        .filter(|(s, _)| s == "Elemental")
        .collect();
    ordered.extend(recipe.parts.iter().filter(|(s, _)| s != "Elemental"));
    for (_, part) in ordered {
        if part.is_empty() {
            continue;
        }
        if let Some(custom) = stats
            .prop(part, "CustomDamageTypeDefinition")?
            .as_str()
            .filter(|c| !c.is_empty())
        {
            return Ok(Some(custom.to_string()));
        }
    }
    Ok(stats
        .prop(&recipe.weapon_type, "DefaultDamageTypeDefinition")?
        .as_str()
        .map(str::to_string))
}

/// `weapon_stats.card_element`: the badge from the damage type, else the elemental part's name suffix.
fn card_element(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    damage_type: Option<&str>,
) -> Result<Option<String>, ItemError> {
    if let Some(d) = damage_type.filter(|d| !d.is_empty()) {
        let kind = stats.prop(d, "DamageType")?;
        if let Some((_, e)) = ELEMENTS_BY_DAMAGE_TYPE
            .iter()
            .find(|(k, _)| Some(*k) == kind.as_str())
        {
            return Ok(Some((*e).to_string()));
        }
    }
    Ok(recipe
        .part("Elemental")
        .map(|p| p.rsplit('_').next().unwrap_or(p).to_string()))
}

/// `weapon_stats.status_rows` (module documentation).
fn status_rows(
    v: &mut Values<'_, '_>,
    attributes: &WeaponAttributes,
    damage_type: Option<&str>,
) -> Result<Option<StatusRows>, ItemError> {
    let stats = v.stats();
    let Some(d) = damage_type.filter(|d| !d.is_empty()) else {
        return Ok(None);
    };
    let Some(effect_path) = stats
        .prop(d, "StatusEffect")?
        .as_str()
        .filter(|p| !p.is_empty())
        .map(str::to_string)
    else {
        return Ok(None);
    };
    let effect = stats.props(&effect_path)?;
    let final_ = |n: &str| {
        attributes
            .values
            .get(&format!("{ATTR}{n}"))
            .copied()
            .unwrap_or(1.0)
    };
    let factor =
        final_("WeaponBaseStatusEffectChanceModifier") * final_("WeaponStatusEffectChanceModifier");
    let mut surfaces: OrderedMap<String, f64> = OrderedMap::new();
    for row in effect.at("DamageSurfaceChanceModifiers").items() {
        if let Some(chance) = v.value(row.at("BaseChance"))? {
            let surface = row.at("SurfaceType").as_str().unwrap_or("");
            let surface = surface
                .strip_prefix("DMGSURFACE_")
                .unwrap_or(surface)
                .to_lowercase();
            surfaces.insert(surface, chance * factor);
        }
    }
    let bounds = status_chance_remaps(stats)?.get(d).copied();
    let chance = match bounds {
        Some(b) => Some(remap_value(b, f32r(factor))),
        None => surfaces.get("generic").copied(),
    };
    let dps = if effect.at("bDoesDamageOverTime").truthy() {
        attributes
            .values
            .get(&format!("{ATTR}WeaponStatusEffectDamage"))
            .copied()
    } else {
        None
    };
    Ok(Some(StatusRows {
        status_effect: effect.at("StatusEffectType").clone(),
        status_effect_definition: effect_path,
        status_chance: chance,
        status_chance_by_surface: surfaces,
        status_dps: dps,
        status_duration: v.value(effect.at("BaseDuration"))?,
    }))
}

/// `weapon_stats.rarity_of`: (level, rating, colour): the type's `BaseRarity` and every part's `Rarity`, each
/// truncated, and the first `RarityLevelColors` row whose level range holds the sum.
fn rarity_of(
    v: &mut Values<'_, '_>,
    weapon_type: &Pv,
    parts: &[&str],
) -> Result<(i64, Pv, Option<String>), ItemError> {
    let stats = v.stats();
    let base = if weapon_type.at("BaseRarity").truthy() {
        v.value(weapon_type.at("BaseRarity"))?
    } else {
        Some(0.0)
    };
    let mut level = crate::rarity::trunc_i64(base.unwrap_or(0.0), "a type's BaseRarity")?;
    for part in parts {
        let rarity = stats.prop(part, "Rarity")?;
        let amount = if rarity.truthy() {
            v.value(&rarity)?
        } else {
            None
        };
        let t = crate::rarity::trunc_i64(amount.unwrap_or(0.0), "a part's Rarity")?;
        level = level
            .checked_add(t)
            .ok_or_else(|| ItemError::BadData("the rarity sum overflows".into()))?;
    }
    if stats.holds(crate::rarity::GLOBALS) {
        for row in stats
            .props(crate::rarity::GLOBALS)?
            .at("RarityLevelColors")
            .items()
        {
            let bound = |k: &str| {
                row.at(k)
                    .number()
                    .filter(|_| row.at(k).truthy())
                    .unwrap_or(0.0)
            };
            // `level` is a small sum of truncated rarities: exact as a double.
            let l = level as f64;
            if bound("MinLevel") <= l && l <= bound("MaxLevel") {
                let color = row.at("Color");
                let channel = |k: &str| -> Result<i64, ItemError> {
                    match color.get(k) {
                        None => Ok(0),
                        Some(c) => to_int(c.number().unwrap_or(0.0).trunc(), "a colour channel"),
                    }
                };
                let text = format!(
                    "#{:02X}{:02X}{:02X}",
                    channel("R")?,
                    channel("G")?,
                    channel("B")?
                );
                return Ok((level, row.at("RarityRating").clone(), Some(text)));
            }
        }
    }
    Ok((level, Pv::Null, None))
}

/// `weapon_stats.level_requirement` with the player's bonus 0.
fn level_requirement(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    weapon_type: &Pv,
    level: i64,
) -> i64 {
    let class = recipe
        .balance
        .as_deref()
        .map(|b| stats.data().class_of(b))
        .unwrap_or_default();
    if short(&class) == MISSION_BALANCE_CLASS
        || !weapon_type.at("bUsesPlayerLevelRequirement").truthy()
    {
        return 0;
    }
    level.max(1)
}

/// `weapon_stats.sale_value`: (price, whether its calculator was checked), or `(None, false)`.
fn sale_value(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    weapon_type: &Pv,
    level: i64,
    unresolved: &mut ow_core::DetSet<String>,
) -> Result<(Option<i64>, bool), ItemError> {
    let init = weapon_type.at("MonetaryValue");
    if !matches!(init, Pv::Map(_)) {
        return Ok((None, false));
    }
    let start = match weapon_type.get("BaseMonetaryValueModifier") {
        None => 0.0,
        Some(v) => v.number().ok_or_else(|| {
            ItemError::BadData("BaseMonetaryValueModifier is not a number".into())
        })?,
    };
    let mut total = f32r(start);
    let mut sources: Vec<&str> = recipe.ordered_parts();
    sources.extend(recipe.name_parts());
    for path in sources {
        let modifier = stats.prop(path, "MonetaryValueMod")?;
        if !modifier.truthy() {
            continue;
        }
        let m = modifier.as_str().unwrap_or("");
        let Some(factor) = stats.resolver_constant(m)? else {
            unresolved.insert(m.to_string());
            return Ok((None, false));
        };
        let factor = factor
            .number()
            .ok_or_else(|| ItemError::BadData(format!("{m}'s constant is not a number")))?;
        total = f32r(total * factor);
    }
    // An item level is far below 2^53: exact.
    let known = vec![
        (crate::stats::PART_PRICE_TOTAL.to_string(), total),
        (crate::stats::WEAPON_LEVEL.to_string(), level as f64),
    ];
    let Some(value) = price_init(stats, init, &known, unresolved, 0)? else {
        return Ok((None, false));
    };
    let currency = weapon_type.at("FormOfCurrency");
    let currency = if currency.truthy() {
        currency.as_str()
    } else {
        None
    };
    let cap = sale_value_cap(Some(currency.unwrap_or("CURRENCY_Credits")), &CURRENCIES);
    let checked = init
        .at("InitializationDefinition")
        .as_str()
        .is_some_and(|d| PRICE_CALCULATORS_CHECKED.contains(&d));
    Ok((
        Some(crate::rarity::trunc_i64(value, "a sale value")?.min(cap)),
        checked,
    ))
}

/// `weapon_stats.zoom_multiplier`: 80 over the type's `ZoomedEndFOV` after the zoom slot's modifiers, or `None`.
fn zoom_multiplier(weapon_type: &Pv, stacks: &Stacks, attribute: &str) -> Option<f64> {
    let fov = weapon_type.at("ZoomedEndFOV").number()?;
    if fov <= 0.0 {
        return None;
    }
    // `combine(f32(fov), mods)`: the same fold as every weapon attribute (Stacks::fold, ow-attr's WeaponGen form).
    let effective = stacks.fold(attribute, f32r(fov), false);
    if effective <= 0.0 {
        return None;
    }
    Some(f32r(CARD_ZOOM_REFERENCE_FOV / effective))
}

fn name_text(stats: &StatsData<'_>, part: Option<&str>) -> Result<Option<String>, ItemError> {
    let Some(p) = part.filter(|p| !p.is_empty()) else {
        return Ok(None);
    };
    Ok(stats
        .prop(p, "PartName")?
        .as_str()
        .filter(|t| !t.is_empty())
        .map(str::to_string))
}

/// A weapon's card in Reference mode (module documentation) at item level `level`.
pub fn weapon_card(
    stats: &StatsData<'_>,
    recipe: &WeaponRecipe,
    level: i64,
    text: &CardText<'_>,
) -> Result<WeaponCard, ItemError> {
    let mut v = Values::new(stats, recipe.maker(), level);
    let attributes = weapon_attributes(&mut v, recipe)?;
    let weapon_type = stats.props(&recipe.weapon_type)?;
    let parts = recipe.ordered_parts();
    let stat = |n: &str| attributes.stat(n);
    let interval = stat("WeaponFireInterval");
    let mut unresolved = std::mem::take(&mut v.unresolved);
    let (price, price_checked) = sale_value(stats, recipe, &weapon_type, level, &mut unresolved)?;
    v.unresolved = unresolved;
    let (rarity_level, rarity_rating, rarity_color) = rarity_of(&mut v, &weapon_type, &parts)?;
    let damage_type = damage_type_of(stats, recipe)?;
    let element = card_element(stats, recipe, damage_type.as_deref())?;
    let status = status_rows(&mut v, &attributes, damage_type.as_deref())?;
    let projectiles = stat("WeaponProjectilesPerShot");
    let zoom_attribute = format!("{ATTR}WeaponZoomEndFOV");
    let mut asks_zoom = false;
    for source in presentation_sources(recipe) {
        if let Some(p) = stats.optional(&source)? {
            asks_zoom |= p
                .at("WeaponCardAttributes")
                .items()
                .iter()
                .any(|e| e.at("Attribute").as_str() == Some(zoom_attribute.as_str()));
        }
    }
    let zoom = if asks_zoom {
        zoom_multiplier(&weapon_type, &attributes.stacks, &zoom_attribute)
    } else {
        None
    };
    let rounding = presentation_rounding(stats)?;
    let zoom_shown = match zoom {
        Some(z) => {
            let (mode, precision) = rounding_of(&rounding, "weapon_zoom");
            Some(present(z, mode, precision)?)
        }
        None => None,
    };
    let text_lines = card_text_lines(
        stats,
        recipe,
        damage_type.as_deref(),
        stat("WeaponShotCost"),
        zoom_shown,
        text,
    )?;
    let barrel = match &attributes.barrel {
        Some(b) => Some(stats.props(b)?),
        None => None,
    };
    let barrel_at = |k: &str| barrel.as_ref().and_then(|b| b.get(k).cloned());
    let firing_mode = match barrel_at("CustomFiringModeDefinition").filter(Pv::truthy) {
        Some(f) => f,
        None => weapon_type.at("DefaultFiringModeDefinition").clone(),
    };
    let bullet_speed = match firing_mode.as_str().filter(|f| !f.is_empty()) {
        Some(f) => stats.prop(f, "Speed")?,
        None => Pv::Null,
    };
    let speed_scale = stat("WeaponProjectileSpeedMultiplier");
    let projectile_speed = match (bullet_speed.truthy(), bullet_speed.number(), speed_scale) {
        (true, Some(b), Some(s)) => Some(b * s),
        _ => None,
    };
    let rarity = match RARITY_NUMBER
        .iter()
        .find(|(r, _)| Some(*r) == rarity_rating.as_str())
    {
        Some((_, n)) => Some(*n),
        None if rarity_level != 0 => Some(rarity_level.clamp(1, 5)),
        None => None,
    };
    let required = level_requirement(stats, recipe, &weapon_type, level);
    let damage_base = attributes.base.get(&format!("{ATTR}WeaponDamage")).copied();
    let damage_stack = damage_base.map(|b| {
        let m = attributes
            .stacks
            .get(&format!("{ATTR}WeaponDamage"))
            .cloned()
            .unwrap_or_default();
        [b, m.pre_add, m.post_add, m.scale_up, m.scale_down]
    });
    let spin_start = if attributes.spinning {
        match barrel_at("StartingSpinUpFireIntervalMultiplier") {
            None => Pv::Float(1.0),
            // weapon_gen_parity compares it as a float (the decoder prints a whole value as an integer); Python's
            // float(int) rounds the same way above 2^53.
            Some(Pv::Int(i)) => Pv::Float(i as f64),
            Some(other) => other,
        }
    } else {
        Pv::Null
    };
    let names = [
        name_text(stats, recipe.prefix.as_deref())?,
        name_text(stats, recipe.title.as_deref())?,
    ];
    let name: Vec<String> = names.into_iter().flatten().collect();
    let accuracy = match stat("WeaponSpread") {
        Some(s) if stats.holds(ACCURACY_PRESENTATION) => {
            match stats.props(ACCURACY_PRESENTATION)?.at("RemappingData") {
                r @ Pv::Map(_) => Some(remap_value(remap_bounds(r), s)),
                _ => None,
            }
        }
        _ => None,
    };
    let mut card = WeaponCard {
        name: name.join(" "),
        level,
        level_requirement: required,
        level_line: required > 1,
        damage: stat("WeaponDamage"),
        damage_stack,
        fire_rate: interval.filter(|i| *i != 0.0).map(|i| f32r(1.0 / i)),
        reload_time: stat("WeaponReloadSpeed"),
        magazine: stat("WeaponClipSize"),
        spread: stat("WeaponSpread"),
        shot_cost: stat("WeaponShotCost"),
        spin_up: stat("WeaponBarrelSpinUpDuration"),
        spin_mode: if attributes.spinning {
            weapon_type.at("BarrelSpinMode").clone()
        } else {
            Pv::Null
        },
        spin_start_interval_scale: spin_start,
        element,
        rarity,
        rarity_level,
        rarity_rating,
        rarity_color,
        manufacturer: Some(recipe.maker().to_string()).filter(|m| !m.is_empty()),
        accuracy,
        sale_value: price,
        sale_value_known: price_checked,
        weapon_zoom: zoom,
        text_lines,
        projectiles: match projectiles {
            Some(p) => Some(crate::rarity::trunc_i64(p, "a projectile count")?),
            None => None,
        },
        firing_mode,
        projectile_speed,
        damage_type,
        status,
        display: Vec::new(),
        accuracy_pool: None,
        burst_count: stat("WeaponAutomaticBurstCount"),
        burst_interval: stat("WeaponBurstInterval"),
        speed_multiplier: speed_scale,
        unresolved: Vec::new(),
    };
    card.display = display(&card, &rounding)?;
    // The accuracy pool as the firing resolver gives it for the same roll (weapon_gen_parity.python_gun).
    // `float(card['level'] or recipe['game_stage'] or 1)`: the level is the recipe's stage, so 0 falls to 1.
    let pool_level = if card.level != 0 { card.level } else { 1 };
    // An item level is far below 2^53: exact.
    card.accuracy_pool = accuracy_pool(stats, recipe, pool_level as f64, card.damage)?;
    card.unresolved = v.unresolved.into_iter().collect();
    Ok(card)
}

/// `weapon_stats.display`: the card's numbers as it prints them, in the tool's order.
fn display(
    card: &WeaponCard,
    rounding: &DetMap<&'static str, (String, i64)>,
) -> Result<Vec<(&'static str, Num)>, ItemError> {
    let mut shown = Vec::new();
    for (field, _, _) in PRESENTATIONS {
        let value = match field {
            "damage" => card.damage,
            "magazine" => card.magazine,
            "fire_rate" => card.fire_rate,
            "reload_time" => card.reload_time,
            "accuracy" => card.accuracy,
            "weapon_zoom" => card.weapon_zoom,
            _ => None,
        };
        if let Some(x) = value {
            let (mode, precision) = rounding_of(rounding, field);
            shown.push((field, present(x, mode, precision)?));
        }
    }
    if let Some(s) = &card.status {
        for (field, value) in [
            ("status_chance", s.status_chance),
            ("status_dps", s.status_dps),
        ] {
            if let Some(x) = value {
                shown.push((field, present(x, STATUS_ROUNDING, 1)?));
            }
        }
    }
    if let Some(p) = card.projectiles.filter(|p| *p > 1) {
        shown.push(("projectiles", Num::Int(p)));
    }
    Ok(shown)
}

/// The shield card's three numbers as `prepare_stock_shield` writes them: three decimals, trailing zeros and a
/// trailing point removed (not as the game's card rounds them: the shield presentation was not read).
pub fn shield_number_text(value: f64) -> String {
    let text = fmt_f(value, 3);
    let text = text.trim_end_matches('0');
    text.trim_end_matches('.').to_string()
}

/// Off mode: a weapon instance's main numbers read through `ow-attr` (`value_of` with the instance as the source)
/// and presented with the card's rounding: damage, fire rate (1 / fire interval), reload, magazine, spread. The
/// rounding modes are the [`PRESENTATIONS`] defaults; reading them from the presentation objects is open.
pub fn instance_numbers(
    weapon: ObjRef,
    sys: &AttributeSystem,
    cx: &mut EvalCx<'_>,
) -> Result<Vec<(&'static str, Option<f64>, Option<Num>)>, ItemError> {
    let mut out = Vec::new();
    for (field, attribute) in [
        ("damage", "WeaponDamage"),
        ("fire_rate", "WeaponFireInterval"),
        ("reload_time", "WeaponReloadSpeed"),
        ("magazine", "WeaponClipSize"),
        ("spread", "WeaponSpread"),
    ] {
        let mut value = instance_stat(weapon, attribute, sys, cx)?;
        if field == "fire_rate" {
            value = value.filter(|i| *i != 0.0).map(|i| f32r(1.0 / i));
        }
        let mode = PRESENTATIONS
            .iter()
            .find(|(f, _, _)| *f == field)
            .map(|(_, _, m)| *m);
        let shown = match (value, mode) {
            (Some(x), Some(m)) => Some(present(x, m, 1)?),
            _ => None,
        };
        out.push((field, value, shown));
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
