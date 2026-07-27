using UnrealBuildTool;

public class Ksa64MissionFoundryEditorTarget : TargetRules
{
    public Ksa64MissionFoundryEditorTarget(TargetInfo Target) : base(Target)
    {
        Type = TargetType.Editor;
        // Phase 12A pins an exact UE 5.8 Launcher build in the toolchain lock.
        // Use that engine's defaults instead of carrying an older UE 5.5
        // include-order contract into a newly created project.
        DefaultBuildSettings = BuildSettingsVersion.V7;
        IncludeOrderVersion = EngineIncludeOrderVersion.Unreal5_8;
        ExtraModuleNames.Add("Ksa64MissionFoundry");
    }
}