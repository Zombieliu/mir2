namespace CrystalSemanticCandidates;

internal static class Program
{
    public static int Main(string[] args)
    {
        try
        {
            var outputPath = ParseArguments(args);
            var crystalRoot = SourceTreeLocator.LocateFixedCrystalRoot();
            var report = DiscoveryEngine.Discover(crystalRoot);
            var json = DeterministicJson.Serialize(report);

            if (outputPath is null)
            {
                Console.Out.Write(json);
            }
            else
            {
                WriteAtomically(outputPath, json);
            }

            return 0;
        }
        catch (DiscoveryException exception)
        {
            Console.Error.WriteLine(exception.Message);
            return 2;
        }
        catch (Exception exception)
        {
            Console.Error.WriteLine($"UNEXPECTED_ERROR: {exception.GetType().Name}: {exception.Message}");
            return 3;
        }
    }

    private static string? ParseArguments(IReadOnlyList<string> args)
    {
        if (args.Count == 0)
        {
            return null;
        }

        if (args.Count == 1 && args[0] is "--help" or "-h")
        {
            Console.WriteLine(
                "Usage: dotnet run --project tools/crystal-semantic-candidates/CrystalSemanticCandidates.csproj -- [--output <file>]\n" +
                "Input is fixed to sibling Crystal/{Client,Server,Shared}; source-root overrides are intentionally unsupported.");
            Environment.Exit(0);
        }

        if (args.Count == 2 && args[0] == "--output" && !string.IsNullOrWhiteSpace(args[1]))
        {
            return Path.GetFullPath(args[1]);
        }

        throw new DiscoveryException(
            "INVALID_ARGUMENTS",
            "Only '--output <file>' is accepted. Source-root overrides are forbidden.");
    }

    private static void WriteAtomically(string outputPath, string json)
    {
        var fullPath = Path.GetFullPath(outputPath);
        var directory = Path.GetDirectoryName(fullPath)
            ?? throw new DiscoveryException("OUTPUT_PATH_INVALID", $"No output directory for '{outputPath}'.");
        Directory.CreateDirectory(directory);
        PathSafety.EnsureExistingPathComponentsAreNotReparse(directory);
        PathSafety.ValidateRelativeSourcePath(Path.GetFileName(fullPath));

        var temporaryPath = Path.Combine(directory, $".{Path.GetFileName(fullPath)}.{Guid.NewGuid():N}.tmp");
        try
        {
            File.WriteAllText(temporaryPath, json, new System.Text.UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
            File.Move(temporaryPath, fullPath, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporaryPath))
            {
                File.Delete(temporaryPath);
            }
        }
    }
}
