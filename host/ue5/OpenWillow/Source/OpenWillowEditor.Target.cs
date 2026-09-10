using UnrealBuildTool;
using System.Collections.Generic;

public class OpenWillowEditorTarget : TargetRules
{
    public OpenWillowEditorTarget(TargetInfo Target) : base(Target)
    {
        Type = TargetType.Editor;
        DefaultBuildSettings = BuildSettingsVersion.V5;
        ExtraModuleNames.Add("OpenWillow");
    }
}
