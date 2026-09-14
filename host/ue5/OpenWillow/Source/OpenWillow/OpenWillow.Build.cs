using UnrealBuildTool;

public class OpenWillow : ModuleRules
{
    public OpenWillow(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
        PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine" });
        PrivateDependencyModuleNames.AddRange(new string[] { "InputCore", "Json", "RenderCore", "RHI" });
    }
}
