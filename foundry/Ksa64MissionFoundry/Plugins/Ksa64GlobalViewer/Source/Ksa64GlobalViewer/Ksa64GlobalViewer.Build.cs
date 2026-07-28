using UnrealBuildTool;

public class Ksa64GlobalViewer : ModuleRules
{
    public Ksa64GlobalViewer(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;

        PublicDependencyModuleNames.AddRange(new[]
        {
            "Core",
            "CoreUObject",
            "Engine",
            "Ksa64Operations"
        });

        PrivateDependencyModuleNames.AddRange(new[]
        {
            "ApplicationCore",
            "InputCore",
            "Json",
            "RenderCore",
            "RHI",
            "Slate",
            "SlateCore"
        });

        if (Target.Platform != UnrealTargetPlatform.Win64
            && Target.Platform != UnrealTargetPlatform.Linux
            && Target.Platform != UnrealTargetPlatform.Mac)
        {
            throw new BuildException(
                "Ksa64GlobalViewer supports only Win64, Linux x64, and macOS ARM64 source lanes. "
                + "Packaging remains conditional on a qualified Unreal host.");
        }
    }
}
