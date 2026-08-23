namespace CrystalSemanticCandidates;

internal static class SourceTreeLocator
{
    public static string LocateFixedCrystalRoot()
    {
        var starts = new[] { Directory.GetCurrentDirectory(), AppContext.BaseDirectory }
            .Select(Path.GetFullPath)
            .Distinct(StringComparer.OrdinalIgnoreCase);

        foreach (var start in starts)
        {
            for (var current = new DirectoryInfo(start); current is not null; current = current.Parent)
            {
                var projectFile = Path.Combine(
                    current.FullName,
                    "tools",
                    "crystal-semantic-candidates",
                    "CrystalSemanticCandidates.csproj");
                if (!File.Exists(projectFile))
                {
                    continue;
                }

                var repositoryRoot = current.Parent?.FullName
                    ?? throw new DiscoveryException(
                        "LAYOUT_ERROR",
                        $"Project root '{current.FullName}' has no sibling directory level.");
                var crystalRoot = Path.Combine(repositoryRoot, "Crystal");
                if (!Directory.Exists(crystalRoot))
                {
                    throw new DiscoveryException(
                        "SOURCE_ROOT_MISSING",
                        $"Expected fixed sibling source root '{crystalRoot}'.");
                }

                return Path.GetFullPath(crystalRoot);
            }
        }

        throw new DiscoveryException(
            "LAYOUT_ERROR",
            "Cannot locate mir2-web3/tools/crystal-semantic-candidates. Run from the mir2-web3 checkout or its tool output directory; the only accepted input is sibling Crystal/{Client,Server,Shared}.");
    }
}
