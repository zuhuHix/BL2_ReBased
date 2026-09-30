using UnrealBuildTool;
using System.IO;

public class OpenWillow : ModuleRules
{
    public OpenWillow(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
        PublicDependencyModuleNames.AddRange(new string[] { "Core", "CoreUObject", "Engine" });
        PrivateDependencyModuleNames.AddRange(new string[] { "InputCore", "Json", "RenderCore", "RHI", "UMG", "Slate", "SlateCore", "WebBrowser" });
        // The engine-independent VM is built by CMake; no game data is linked.
        string RepoRoot = Path.GetFullPath(Path.Combine(ModuleDirectory, "../../../../.."));
        PrivateIncludePaths.Add(Path.Combine(RepoRoot, "src"));
        foreach (string Library in new string[] { "ow-core.lib", "ow-lzokay.lib" })
        {
            string LibraryPath = Path.Combine(RepoRoot, "build", "Release", Library);
            if (!File.Exists(LibraryPath)) throw new BuildException("Build the VM first: cmake --build build --config Release (missing " + LibraryPath + ")");
            PublicAdditionalLibraries.Add(LibraryPath);
            ExternalDependencies.Add(LibraryPath);
        }
        bEnableExceptions = true;
    }
}
