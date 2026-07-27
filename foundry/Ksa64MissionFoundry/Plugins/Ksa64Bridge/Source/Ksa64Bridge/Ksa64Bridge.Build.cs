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

        if (Target.Platform != UnrealTargetPlatform.Win64)
        {
            throw new BuildException("Ksa64Bridge Phase 12A supports Win64 only.");
        }

        PublicSystemLibraries.Add("bcrypt.lib");

        string BridgeInclude = Path.Combine(ModuleDirectory, "..", "ThirdParty", "ViewerBridge", "include");
        PublicIncludePaths.Add(Path.GetFullPath(BridgeInclude));

        // A separate, explicit phase12/build-bridge.ps1 invocation stages the
        // Rust output. UnrealBuildTool never invokes Cargo.
        string BridgeBinaries = Path.Combine(PluginDirectory, "Binaries", "Win64");
        if (Directory.Exists(BridgeBinaries))
        {
            string[] Manifests = Directory.GetFiles(
                BridgeBinaries,
                "ksa64_viewer_bridge-*.manifest.json",
                SearchOption.TopDirectoryOnly);
            Array.Sort(Manifests, StringComparer.Ordinal);

            if (Manifests.Length > 1)
            {
                throw new BuildException(
                    "Ksa64Bridge found multiple staged manifests. Run phase12/build-bridge.ps1 to produce one qualified artifact.");
            }

            if (Manifests.Length == 1)
            {
                string Manifest = Manifests[0];
                string Dll = Manifest.Substring(0, Manifest.Length - ".manifest.json".Length) + ".dll";
                if (!File.Exists(Dll))
                {
                    throw new BuildException("Ksa64Bridge staged manifest has no matching DLL: " + Dll);
                }

                RuntimeDependencies.Add(
                    "$(PluginDir)/Binaries/Win64/" + Path.GetFileName(Manifest),
                    StagedFileType.NonUFS);
                RuntimeDependencies.Add(
                    "$(PluginDir)/Binaries/Win64/" + Path.GetFileName(Dll),
                    StagedFileType.NonUFS);
                PublicDefinitions.Add("KSA64_BRIDGE_ARTIFACT_STAGED=1");
            }
        }
    }
}
