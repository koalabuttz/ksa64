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

        if (Target.Platform != UnrealTargetPlatform.Win64)
        {
            throw new BuildException("Ksa64Operations Phase 12B supports Win64 only.");
        }
    }
}
