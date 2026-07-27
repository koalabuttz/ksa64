using UnrealBuildTool;

public class Ksa64MissionFoundryTarget : TargetRules
{
    public Ksa64MissionFoundryTarget(TargetInfo Target) : base(Target)
    {
        Type = TargetType.Game;
        // Phase 12A pins an exact UE 5.8 Launcher build in the toolchain lock.
        // Use that engine's defaults instead of carrying an older UE 5.5
        // include-order contract into a newly created project.
        DefaultBuildSettings = BuildSettingsVersion.Latest;
        IncludeOrderVersion = EngineIncludeOrderVersion.Latest;
        ExtraModuleNames.Add("Ksa64MissionFoundry");
    }
}