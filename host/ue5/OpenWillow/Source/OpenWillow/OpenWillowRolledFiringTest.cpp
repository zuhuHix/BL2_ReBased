// The rolled gun's firing merge (FOpenWillowWeaponGen::MergeFiringTemplate) on an invented template and card: no game data.
// tools/weapon_gen_firing.py's own check is Python and does not reach this C++ or the card overrides.
#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "Dom/JsonObject.h"
#include "OpenWillowFiring.h"
#include "OpenWillowInventory.h"
#include "OpenWillowWeaponGen.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "weapon_firing.hpp"
#include "weapon_gen.hpp"

namespace
{
TSharedPtr<FJsonObject> Json(const FString& Text)
{
    TSharedPtr<FJsonObject> Out;
    FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Out);
    return Out;
}

const TCHAR* RocketTemplate = TEXT(R"({"schema":1,"firing_mode":"Fake.Mode","fire_type":"EWWFT_Rocket","bullet_speed":100,"trace_range":5000,
    "projectile":{"speed":1000,"splash":{"radius":300,"damage":null,"damage_scale":0.5,"falloff":false}}})");

FOpenWillowWeaponItem RolledItem()
{
    FOpenWillowWeaponItem Item;
    Item.Damage = 100.f;
    Item.Projectiles = 3;
    Item.Spread = 0.5f;
    Item.SpinUp = 0.7f;
    Item.SpinMode = TEXT("BSM_SpinUpToFullFireRate");
    Item.SpinStartIntervalScale = 2.f;
    Item.Firing.BurstCount = 2;
    Item.Firing.BurstInterval = 0.25f;
    Item.Firing.bBurstFromManifest = true;
    return Item;
}

weapongen::WeaponGenCard RolledCard()
{
    weapongen::WeaponGenCard Card;
    Card.speedMultiplier = 2.0;
    Card.hasStatus = true;
    Card.statusEffectDefinition = "Fake.Status";
    Card.statusEffect = "Fake_Type";
    Card.statusDuration = 4.0;
    Card.statusDps = 9.0;
    Card.statusChanceBySurface.push_back({"generic", 25.0});
    return Card;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowRolledGunFiringTemplateTest, "OpenWillow.Vendor.RolledGunFiringTemplate",
    EAutomationTestFlags::EditorContext | EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowRolledGunFiringTemplateTest::RunTest(const FString& Parameters)
{
    FString Missing;
    {
        FOpenWillowWeaponItem Item = RolledItem();
        const TArray<TSharedPtr<FJsonValue>> Scales = Json(TEXT(R"({"rows":[{"LevelDifference":1,"HigherLevelChanceScale":0.5,"LowerLevelChanceScale":2}]})"))->GetArrayField(TEXT("rows"));
        TestTrue(TEXT("a complete template merges"), FOpenWillowWeaponGen::MergeFiringTemplate(*Json(RocketTemplate), &Scales, RolledCard(), Item, Missing));
        const FOpenWillowFiringData& F = Item.Firing;
        TestTrue(TEXT("the block is loaded"), F.bLoaded);
        TestEqual(TEXT("the kind comes from the template"), F.Kind, EOpenWillowShotKind::Rocket);
        // Card and item overrides.
        TestEqual(TEXT("pellets"), F.Pellets, 3);
        TestEqual(TEXT("spread"), F.Spread, 0.5f);
        TestEqual(TEXT("burst count"), F.BurstCount, 2);
        TestEqual(TEXT("burst interval"), F.BurstInterval, 0.25f);
        TestTrue(TEXT("the burst stays known"), F.bBurstFromManifest);
        TestEqual(TEXT("spin-up"), F.SpinUpDuration, 0.7f);
        TestEqual(TEXT("spin start"), F.SpinStartMultiplier, 2.f);
        TestTrue(TEXT("spins to full rate"), F.bSpinsToFullRate);
        // Scaling: speeds by the multiplier, an unevaluated splash by the card's damage.
        TestEqual(TEXT("bullet speed x multiplier"), F.BulletSpeed, 200.f);
        TestEqual(TEXT("projectile speed x multiplier"), F.ProjectileSpeed, 2000.f);
        TestEqual(TEXT("splash = card damage x scale"), F.Splash.Damage, 50.f);
        TestEqual(TEXT("splash radius from the template"), F.Splash.Radius, float(weapon_firing::explosionRadius(300.0)));
        TestTrue(TEXT("no falloff from the template"), F.Splash.bFullDamage);
        // Status from the card, the level scale from the file.
        TestTrue(TEXT("status"), F.Status.bValid);
        TestEqual(TEXT("status definition"), F.Status.Definition, FString(TEXT("Fake.Status")));
        TestEqual(TEXT("status type"), F.Status.Type, FString(TEXT("Fake_Type")));
        TestEqual(TEXT("status duration"), F.Status.Duration, 4.f);
        TestEqual(TEXT("status dps"), F.Status.Dps, 9.f);
        TestEqual(TEXT("status chance"), F.Status.ChancePercent(TEXT("generic")), 25.f);
        TestEqual(TEXT("level scale rows"), int32(F.Status.LevelScales.size()), 1);
    }
    {
        // An evaluated splash stays; no status and no multiplier leave defaults.
        FOpenWillowWeaponItem Item = RolledItem();
        const TSharedPtr<FJsonObject> Template = Json(TEXT(R"({"schema":1,"firing_mode":"Fake.Mode","fire_type":"EWWFT_Rocket","projectile":{"speed":1000,"splash":{"radius":300,"damage":70}}})"));
        TestTrue(TEXT("merges"), FOpenWillowWeaponGen::MergeFiringTemplate(*Template, nullptr, weapongen::WeaponGenCard(), Item, Missing));
        TestEqual(TEXT("an evaluated splash is kept"), Item.Firing.Splash.Damage, 70.f);
        TestEqual(TEXT("no multiplier: speed unchanged"), Item.Firing.ProjectileSpeed, 1000.f);
        TestFalse(TEXT("no status on the card"), Item.Firing.Status.bValid);
    }
    // Refusals: the item keeps what it had, and the reason names the field.
    const auto Refused = [&](const TCHAR* What, const TCHAR* Text, const TCHAR* Field)
    {
        FOpenWillowWeaponItem Item = RolledItem();
        Missing.Reset();
        TestFalse(What, FOpenWillowWeaponGen::MergeFiringTemplate(*Json(Text), nullptr, RolledCard(), Item, Missing));
        TestEqual(*FString::Printf(TEXT("%s: reason"), What), Missing, FString(Field));
        TestFalse(*FString::Printf(TEXT("%s: not loaded"), What), Item.Firing.bLoaded);
        TestEqual(*FString::Printf(TEXT("%s: burst kept"), What), Item.Firing.BurstCount, 2);
    };
    Refused(TEXT("a rocket with no projectile"), TEXT(R"({"schema":1,"firing_mode":"M","fire_type":"EWWFT_Rocket"})"), TEXT("projectile.speed"));
    Refused(TEXT("a rocket with speed 0"), TEXT(R"({"schema":1,"firing_mode":"M","fire_type":"EWWFT_Rocket","projectile":{"speed":0}})"), TEXT("projectile.speed"));
    Refused(TEXT("a bullet with no speed"), TEXT(R"({"schema":1,"firing_mode":"M"})"), TEXT("bullet_speed"));
    Refused(TEXT("no firing mode"), TEXT(R"({"schema":1,"bullet_speed":100})"), TEXT("firing_mode"));
    Refused(TEXT("another schema"), TEXT(R"({"schema":2,"firing_mode":"M","bullet_speed":100})"), TEXT("schema"));
    return true;
}
#endif
