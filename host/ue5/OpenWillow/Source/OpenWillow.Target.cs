using UnrealBuildTool;
using System.Collections.Generic;

public class OpenWillowTarget : TargetRules
{
    public OpenWillowTarget(TargetInfo Target) : base(Target)
    {
        Type = TargetType.Game;
        DefaultBuildSettings = BuildSettingsVersion.V5;
        ExtraModuleNames.Add("OpenWillow");
    }
}
