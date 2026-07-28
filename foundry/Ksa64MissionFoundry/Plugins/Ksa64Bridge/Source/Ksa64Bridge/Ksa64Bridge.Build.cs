using System;
using System.IO;
using UnrealBuildTool;

public class Ksa64Bridge : ModuleRules
{
    public Ksa64Bridge(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;

        PublicDependencyModuleNames.AddRange(new[]
        {
            "Core",
            "Json",
            "Projects"
        });

        string PlatformDirectory;
        string LibraryPattern;
        if (Target.Platform == UnrealTargetPlatform.Win64)
        {
            PlatformDirectory = "Win64";
            LibraryPattern = "ksa64_viewer_bridge-*.*";
        }
        else if (Target.Platform == UnrealTargetPlatform.Linux)
        {
            PlatformDirectory = "Linux";
            LibraryPattern = "libksa64_viewer_bridge-*.*";
        }
        else if (Target.Platform == UnrealTargetPlatform.Mac)
        {
            PlatformDirectory = "Mac";
            LibraryPattern = "libksa64_viewer_bridge-*.*";
        }
        else
        {
            throw new BuildException(
                "Ksa64Bridge supports only Win64, Linux x64, and macOS ARM64 source lanes. "
                + "A qualified Unreal host and staged bridge are still required for packaging.");
        }

        string BridgeInclude = Path.Combine(ModuleDirectory, "..", "ThirdParty", "ViewerBridgePortable", "include");
        PublicIncludePaths.Add(Path.GetFullPath(BridgeInclude));

        // A separate explicit staging command builds Rust outside UnrealBuildTool.
        // UnrealBuildTool only stages one already-qualified library/manifest pair.
        string BridgeBinaries = Path.Combine(PluginDirectory, "Binaries", PlatformDirectory);
        if (!Directory.Exists(BridgeBinaries))
        {
            return;
        }

        string[] Manifests = Directory.GetFiles(
            BridgeBinaries,
            "*.manifest.json",
            SearchOption.TopDirectoryOnly);
        Array.Sort(Manifests, StringComparer.Ordinal);
        if (Manifests.Length > 1)
        {
            throw new BuildException(
                "Ksa64Bridge found multiple staged manifests. Stage one qualified artifact for this platform.");
        }
        if (Manifests.Length == 0)
        {
            return;
        }

        string[] Libraries = Directory.GetFiles(BridgeBinaries, LibraryPattern, SearchOption.TopDirectoryOnly);
        Libraries = Array.FindAll(Libraries, path =>
            path.EndsWith(".dll", StringComparison.OrdinalIgnoreCase)
            || path.EndsWith(".so", StringComparison.OrdinalIgnoreCase)
            || path.EndsWith(".dylib", StringComparison.OrdinalIgnoreCase));
        Array.Sort(Libraries, StringComparer.Ordinal);
        if (Libraries.Length != 1)
        {
            throw new BuildException(
                "Ksa64Bridge staged manifest must have exactly one matching platform library.");
        }

        RuntimeDependencies.Add(
            "$(PluginDir)/Binaries/" + PlatformDirectory + "/" + Path.GetFileName(Manifests[0]),
            StagedFileType.NonUFS);
        RuntimeDependencies.Add(
            "$(PluginDir)/Binaries/" + PlatformDirectory + "/" + Path.GetFileName(Libraries[0]),
            StagedFileType.NonUFS);
        PublicDefinitions.Add("KSA64_BRIDGE_ARTIFACT_STAGED=1");
    }
}
