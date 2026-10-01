#include "OpenWillowSliceData.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include <stdexcept>

namespace
{
[[noreturn]] void Missing(const FString& What)
{
    throw std::runtime_error(TCHAR_TO_UTF8(*FString::Printf(TEXT("slice manifest: %s"), *What)));
}

TSharedPtr<FJsonObject> ReadJson(const FString& File)
{
    FString Text;
    TSharedPtr<FJsonObject> Data;
    if (!FFileHelper::LoadFileToString(Text, *File)) Missing(TEXT("cannot read ") + File);
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Data) || !Data) Missing(TEXT("invalid JSON in ") + File);
    return Data;
}

TSharedPtr<FJsonObject> Obj(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    const TSharedPtr<FJsonObject>* Out = nullptr;
    if (!J || !J->TryGetObjectField(Key, Out) || !Out || !*Out) Missing(FString::Printf(TEXT("missing object '%s'"), Key));
    return *Out;
}

const TArray<TSharedPtr<FJsonValue>>& Arr(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    const TArray<TSharedPtr<FJsonValue>>* Out = nullptr;
    if (!J || !J->TryGetArrayField(Key, Out) || !Out) Missing(FString::Printf(TEXT("missing array '%s'"), Key));
    return *Out;
}

double Num(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    double Out = 0;
    if (!J || !J->TryGetNumberField(Key, Out) || !FMath::IsFinite(Out)) Missing(FString::Printf(TEXT("missing number '%s'"), Key));
    return Out;
}

FString Str(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    FString Out;
    if (!J || !J->TryGetStringField(Key, Out) || Out.IsEmpty()) Missing(FString::Printf(TEXT("missing string '%s'"), Key));
    return Out;
}

FVector Vec(const TSharedPtr<FJsonObject>& J, const TCHAR* Key)
{
    const auto& A = Arr(J, Key);
    if (A.Num() != 3) Missing(FString::Printf(TEXT("'%s' is not a 3-vector"), Key));
    FVector V;
    for (int32 I = 0; I < 3; ++I)
    {
        double N = 0;
        if (!A[I]->TryGetNumber(N) || !FMath::IsFinite(N) || FMath::Abs(N) > 1e7) Missing(FString::Printf(TEXT("invalid '%s'"), Key));
        V[I] = N;
    }
    return V;
}

// "host" pose block: location and rotation [Pitch, Yaw, Roll] in degrees.
FTransform HostPose(const TSharedPtr<FJsonObject>& Owner)
{
    const auto Host = Obj(Owner, TEXT("host"));
    const FVector R = Vec(Host, TEXT("rotation"));
    return FTransform(FRotator(R.X, R.Y, R.Z), Vec(Host, TEXT("location")));
}

// "/Game/Dir/Asset" -> "/Game/Dir/Asset.Asset" for LoadObject.
FString AssetPath(const FString& Path)
{
    if (Path.IsEmpty() || Path.Contains(TEXT("."))) return Path;
    return Path + TEXT(".") + FPaths::GetBaseFilename(Path);
}
}

FString FOpenWillowSliceData::ObjectPath(const FString& Value)
{
    int32 Colon = INDEX_NONE;
    return Value.FindChar(TEXT(':'), Colon) ? Value.Mid(Colon + 1) : Value;
}

FString FOpenWillowSliceData::OpName(const FString& Value)
{
    int32 Dot = INDEX_NONE;
    return Value.FindLastChar(TEXT('.'), Dot) ? Value.Mid(Dot + 1) : Value;
}

float FOpenWillowSliceData::HealthForLevel(int32 Level) const
{
    return float(FMath::Max(HealthMin, HealthMultiplier * FMath::Pow(HealthScaler, double(Level))));
}

int32 FOpenWillowSliceData::MissionXp(int32 MissionLevel) const
{
    auto Required = [this](int32 L) { return XpMultiplier * FMath::Pow(double(L), XpPower) + XpOffset; };
    return int32(FMath::RoundToDouble(XpPercentage * (Required(MissionLevel + 1) - Required(MissionLevel))));
}

void FOpenWillowSliceData::Load(const FString& WorldFile, const FString& NpcFile, const FString& AudioFile)
{
    const auto World = ReadJson(WorldFile);
    if (Str(World, TEXT("schema")) != TEXT("ow-slice-world-v1")) Missing(TEXT("world manifest schema is not ow-slice-world-v1"));

    // Range trigger. Only the shape this route uses is supported; anything else is an error, not a guess.
    const auto Trigger = Obj(World, TEXT("range_trigger"));
    const auto Shape = Obj(Trigger, TEXT("shape"));
    if (Str(Shape, TEXT("type")) != TEXT("cylinder")) Missing(TEXT("range trigger is not a cylinder"));
    if (!Trigger->GetBoolField(TEXT("update_objective_on_player_touch")) || !Trigger->GetBoolField(TEXT("enabled"))
        || Arr(Trigger, TEXT("objective_set_restrictions")).Num() || Arr(Trigger, TEXT("touch_volumes")).Num())
        Missing(TEXT("range trigger uses a touch rule this host does not implement"));
    TriggerObject = ObjectPath(Str(Trigger, TEXT("object")));
    TriggerObjective = Str(Trigger, TEXT("linked_objective"));
    TriggerCenter = HostPose(Trigger).GetLocation();
    TriggerRadius = Num(Shape, TEXT("radius"));
    TriggerHalfHeight = Num(Shape, TEXT("half_height"));

    // Marcus.
    const auto MarcusData = Obj(World, TEXT("marcus"));
    const FTransform Pawn = HostPose(Obj(MarcusData, TEXT("pawn")));
    MarcusLocation = Pawn.GetLocation();
    MarcusRotation = Pawn.Rotator();
    const auto Ai = Obj(MarcusData, TEXT("ai_class"));
    MarcusSpeed = Num(Ai, TEXT("ground_speed")) * Num(Ai, TEXT("walking_pct"));
    MarcusYawRate = Num(Obj(Ai, TEXT("rotation_rate_units")), TEXT("Yaw")) * 360.0 / 65536.0;
    const auto WalkData = Obj(MarcusData, TEXT("walk_to_range"));
    WalkOp = OpName(Str(WalkData, TEXT("action")));
    for (const auto& Value : Arr(WalkData, TEXT("path")))
    {
        const auto Node = Value->AsObject();
        FOpenWillowSliceNode Out;
        Out.Name = ObjectPath(Str(Node, TEXT("node")));
        Out.Location = HostPose(Node).GetLocation();
        Out.ArrivalRadius = Num(Node, TEXT("pawn_arrival_radius"));
        Out.SpeedPercentage = Num(Node, TEXT("ai_speed_percentage"));
        for (const auto& Arrival : Arr(Node, TEXT("on_arrival"))) Out.ArrivalEvents.Add(OpName(Str(Arrival->AsObject(), TEXT("event"))));
        Walk.Add(Out);
    }
    if (Walk.Num() == 0) Missing(TEXT("Marcus walk has no nodes"));
    for (const auto& Value : Arr(WalkData, TEXT("finished")))
    {
        const auto Op = Value->AsObject();
        for (const auto& Behavior : Arr(Op, TEXT("behaviors")))
        {
            const auto B = Behavior->AsObject();
            bool bValue = false;
            if (OpName(Str(B, TEXT("flag"))) == TEXT("Flag_LookAtPlayer") && B->TryGetBoolField(TEXT("value"), bValue) && bValue)
                LookAtPlayerOps.Add(OpName(Str(Op, TEXT("op"))));
        }
    }

    // Target dummy.
    const auto DummyData = Obj(World, TEXT("target_dummy"));
    const auto Den = Obj(DummyData, TEXT("den"));
    DummyObjective = Str(Den, TEXT("mission_objective"));
    DenObject = ObjectPath(Str(Den, TEXT("den")));
    const auto& Points = Arr(Den, TEXT("spawn_points"));
    if (Points.Num() != 1) Missing(TEXT("expected exactly one dummy spawn point"));
    PointObject = ObjectPath(Str(Points[0]->AsObject(), TEXT("object")));
    const FTransform Spawn = HostPose(Points[0]->AsObject());
    DummyLocation = Spawn.GetLocation();
    DummyRotation = Spawn.Rotator();
    HolderObject = ObjectPath(Str(Obj(DummyData, TEXT("holder")), TEXT("object")));
    TargetBinding = Obj(Obj(World, TEXT("target_mover")), TEXT("binding"));

    // Respawn stations (selection rule in UOpenWillowQuest::RespawnPoint).
    const auto Oracle = Obj(Obj(Obj(World, TEXT("respawn")), TEXT("fresh_state_choice")), TEXT("range_trigger"));
    OracleDeathLocation = Vec(Oracle, TEXT("death_location"));
    OracleRespawnLocation = HostPose(Obj(Oracle, TEXT("first_exit_point"))).GetLocation();
    for (const auto& Value : Arr(Obj(World, TEXT("respawn")), TEXT("stations")))
    {
        const auto S = Value->AsObject();
        FOpenWillowSliceStation Out;
        Out.Object = Str(S, TEXT("object"));
        Out.Location = HostPose(S).GetLocation();
        Out.bCanResurrect = S->GetBoolField(TEXT("can_resurrect"));
        for (const auto& Exit : Arr(S, TEXT("exit_points"))) Out.Exits.Add(HostPose(Exit->AsObject()));
        Stations.Add(Out);
    }

    // Health: multiplier and scaler attributes resolved to their constants (attribute over BaseValueConstant).
    const auto Health = Obj(Obj(World, TEXT("values")), TEXT("health"));
    const auto Formula = Obj(Health, TEXT("formula"));
    const auto Constants = Obj(Health, TEXT("constant_attributes"));
    HealthMultiplier = Num(Constants, *Str(Obj(Formula, TEXT("Multiplier")), TEXT("BaseValueAttribute")));
    HealthScaler = Num(Constants, *Str(Obj(Formula, TEXT("Level")), TEXT("BaseValueAttribute")));
    const auto Restriction = Obj(Health, TEXT("restriction"));
    HealthMin = Restriction->GetBoolField(TEXT("bEnableMinValueRestriction"))
        ? Num(Obj(Restriction, TEXT("MinValue")), TEXT("BaseValueConstant")) : 0.0;

    // Mission XP: percentage, required-experience formula constants and the candidate table.
    const auto Xp = Obj(Obj(World, TEXT("values")), TEXT("xp"));
    const auto Percentage = Obj(Xp, TEXT("reward_percentage"));
    XpRewardAttribute = Str(Percentage, TEXT("attribute"));
    XpPercentage = Num(Percentage, TEXT("playthrough1"));
    const auto Required = Obj(Obj(Xp, TEXT("required_formula")), TEXT("formula"));
    XpMultiplier = Num(Obj(Required, TEXT("Multiplier")), TEXT("BaseValueConstant"));
    XpPower = Num(Obj(Required, TEXT("Power")), TEXT("BaseValueConstant"));
    XpOffset = Num(Obj(Required, TEXT("Offset")), TEXT("BaseValueConstant"));
    for (const auto& Pair : Obj(Xp, TEXT("candidate_amount_by_mission_level"))->Values)
        XpCandidateByLevel.Add(FCString::Atoi(*Pair.Key), int32(Pair.Value->AsNumber()));

    // NPC assets (UE paths) and the pawns' mesh-component translations from the identity manifest beside it.
    const auto Npcs = Obj(ReadJson(NpcFile), TEXT("use"));
    PistolMesh = AssetPath(Str(Obj(Npcs, TEXT("MaliwanPistol")), TEXT("rolled_sample_mesh")));
    const auto Identity = Obj(ReadJson(FPaths::Combine(FPaths::GetPath(NpcFile), TEXT("npc_identity.json"))), TEXT("npcs"));
    auto ReadNpc = [&](const TCHAR* Key, const TCHAR* Moving, FOpenWillowNpcAssets& Out)
    {
        const auto Use = Obj(Npcs, Key);
        const auto Anims = Obj(Use, TEXT("anims"));
        Out.Mesh = AssetPath(Str(Use, TEXT("skeletal_mesh")));
        Out.Idle = AssetPath(Str(Anims, TEXT("idle")));
        (FString(Moving) == TEXT("walk") ? Out.Walk : Out.Death) = AssetPath(Str(Anims, Moving));
        const auto T = Obj(Obj(Obj(Identity, Key), TEXT("mesh_component_properties")), TEXT("translation"));
        Out.MeshOffset = FVector(Num(T, TEXT("X")), Num(T, TEXT("Y")), Num(T, TEXT("Z")));
    };
    ReadNpc(TEXT("Marcus"), TEXT("walk"), Marcus);
    ReadNpc(TEXT("TargetDummy"), TEXT("death_fire"), Dummy);

    // Audio lookup.
    const auto AudioData = ReadJson(AudioFile);
    if (Str(AudioData, TEXT("schema")) != TEXT("ow-audio-v1")) Missing(TEXT("audio manifest schema is not ow-audio-v1"));
    for (const auto& Value : Arr(AudioData, TEXT("entries")))
        Audio.Add(Str(Value->AsObject(), TEXT("key")), Value->AsObject());
}
