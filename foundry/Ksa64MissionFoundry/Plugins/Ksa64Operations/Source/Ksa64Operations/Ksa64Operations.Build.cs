using UnrealBuildTool;

public class Ksa64Operations : ModuleRules
{
    public Ksa64Operations(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;

        PublicDependencyModuleNames.AddRange(new[]
        {
            "Core",
            "CoreUObject",
            "Engine",
            "Ksa64Bridge"
        });

        PrivateDependencyModuleNames.AddRange(new[]
        {
            "ApplicationCore",
            "ImageCore",
            "ImageWrapper",
            "InputCore",
            "Json",
            "RHI",
            "Slate",
            "SlateCore"
        });

        if (Target.Platform != UnrealTargetPlatform.Win64
            && Target.Platform != UnrealTargetPlatform.Linux
            && Target.Platform != UnrealTargetPlatform.Mac)
        {
            throw new BuildException(
                "Ksa64Operations supports only Win64, Linux x64, and macOS ARM64 source lanes. "
                + "Packaging remains conditional on a qualified Unreal host.");
        }
    }
}
