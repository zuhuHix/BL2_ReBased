"""Synthetic tests for recipe-to-WeaponDefinitionData field mapping; fake paths only."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools" / "real_game" / "scripts"))
import weapon_parity_probe as probe


class WeaponParityRecipeMappingTests(unittest.TestCase):
    def test_maps_exact_object_paths_parts_and_name_parts(self):
        recipe = {
            "balance": "Test.Balance",
            "manufacturer": "Test.Manufacturer",
            "weapon_type": "Test.WeaponType",
            "game_stage": 13,
            "parts": {
                "Body": {"part": "Test.Body"},
                "Grip": {"part": "Test.Grip"},
                "Barrel": {"part": "Test.Barrel"},
                "Sight": {"part": "Test.NoneSight"},
                "Elemental": {"part": "Test.NoneElement"},
                "Material": {"part": "Test.Material"},
                "Accessory1": {"part": "Test.NoneAccessory"},
            },
            "prefix": {"part": "Test.Prefix"},
            "title": {"part": "Test.Title"},
        }
        fields = probe.definition_fields(recipe)
        self.assertEqual(fields["BalanceDefinition"], "Test.Balance")
        self.assertEqual(fields["ManufacturerGradeIndex"], 13)
        self.assertEqual(fields["GameStage"], 13)
        self.assertEqual(fields["BodyPartDefinition"], "Test.Body")
        self.assertEqual(fields["Accessory1PartDefinition"], "Test.NoneAccessory")
        self.assertEqual(fields["MaterialPartDefinition"], "Test.Material")
        self.assertEqual(fields["PrefixPartDefinition"], "Test.Prefix")
        self.assertEqual(fields["TitlePartDefinition"], "Test.Title")
        self.assertEqual(set(fields) - {
            "WeaponTypeDefinition", "BalanceDefinition", "ManufacturerDefinition", "GameStage",
            "ManufacturerGradeIndex", "BodyPartDefinition", "GripPartDefinition", "BarrelPartDefinition",
            "SightPartDefinition", "ElementalPartDefinition", "MaterialPartDefinition", "Accessory1PartDefinition",
            "PrefixPartDefinition", "TitlePartDefinition",
        }, set())

    def test_omits_absent_optional_slots_and_honours_explicit_grade(self):
        fields = probe.definition_fields({
            "balance": "Test.Balance", "manufacturer": "Test.Manufacturer", "weapon_type": "Test.Type",
            "game_stage": 17, "manufacturer_grade_index": 3, "parts": {"Body": "Test.Body", "Grip": None},
        })
        self.assertEqual(fields["GameStage"], 17)
        self.assertEqual(fields["ManufacturerGradeIndex"], 3)
        self.assertEqual(fields["BodyPartDefinition"], "Test.Body")
        self.assertNotIn("GripPartDefinition", fields)
        self.assertNotIn("TitlePartDefinition", fields)

    def test_requires_balance_manufacturer_and_type(self):
        with self.assertRaisesRegex(ValueError, "BalanceDefinition"):
            probe.definition_fields({"manufacturer": "Test.Maker", "weapon_type": "Test.Type"})


class WeaponParityDeepTests(unittest.TestCase):
    def test_enum_uses_member_name(self):
        import enum

        class Mode(enum.IntEnum):
            SPIN_FAST = 2

        self.assertEqual(probe._deep(Mode.SPIN_FAST), "SPIN_FAST")

    def test_object_with_name_attribute_keeps_full_path(self):
        # The SDK resolves attributes case-insensitively, so a UObject answers .name with its short Name.
        class Fake:
            name = "Short"
            Class = object()

            def _path_name(self):
                return "Pkg.Group.Short"

        self.assertEqual(probe._deep(Fake()), "Pkg.Group.Short")


class WeaponParityFieldNameTests(unittest.TestCase):
    def test_reads_property_names_from_the_class_and_skips_functions(self):
        class Field:
            def __init__(self, name, kind):
                self.Name = name
                self.Class = type("C", (), {"Name": kind})()

        class FakeClass:
            def _path_name(self):
                return "Test.FakeWeaponClass"

            def _fields(self):
                return [Field("AmmoInClip", "IntProperty"), Field("StartFire", "Function"),
                        Field("ZoomFOV", "FloatProperty")]

        obj = type("Obj", (), {"Class": FakeClass()})()
        self.assertEqual(probe._field_names(obj), ["AmmoInClip", "ZoomFOV"])


if __name__ == "__main__":
    unittest.main()
