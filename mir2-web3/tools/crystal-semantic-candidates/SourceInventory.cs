namespace CrystalSemanticCandidates;

internal sealed record SourceSnapshotFile(
    string AbsolutePath,
    string RelativePath,
    byte[] Bytes,
    long LastWriteUtcTicks,
    SourceFileRecord Record);

internal sealed class SourceSnapshot
{
    public required IReadOnlyList<SourceSnapshotFile> Files { get; init; }

    public required string AggregateSha256 { get; init; }
}

internal static class SourceInventory
{
    private static readonly string[] RequiredRoots = ["Client", "Server", "Shared"];

    public static SourceSnapshot Capture(string crystalRoot)
    {
        var fullRoot = Path.GetFullPath(crystalRoot);
        if (!Directory.Exists(fullRoot))
        {
            throw new DiscoveryException(
                "SOURCE_ROOT_MISSING",
                $"Fixed sibling Crystal root does not exist: '{fullRoot}'.");
        }

        PathSafety.EnsureExistingPathComponentsAreNotReparse(fullRoot);
        var discovered = new List<(string AbsolutePath, string RelativePath)>();

        foreach (var rootName in RequiredRoots)
        {
            var inputRoot = Path.Combine(fullRoot, rootName);
            if (!Directory.Exists(inputRoot))
            {
                throw new DiscoveryException(
                    "SOURCE_ROOT_MISSING",
                    $"Required source root '{rootName}' does not exist under '{fullRoot}'.");
            }

            ScanDirectory(fullRoot, inputRoot, discovered);
        }

        if (discovered.Count == 0)
        {
            throw new DiscoveryException(
                "EMPTY_SOURCE_TREE",
                "Crystal/Client, Crystal/Server, and Crystal/Shared contain no C# source files.");
        }

        PathSafety.EnsureNoAliasCollisions(discovered.Select(entry => entry.RelativePath));
        discovered.Sort((left, right) => StringComparer.Ordinal.Compare(left.RelativePath, right.RelativePath));

        var files = new List<SourceSnapshotFile>(discovered.Count);
        foreach (var entry in discovered)
        {
            PathSafety.EnsureExistingPathComponentsAreNotReparse(entry.AbsolutePath);
            var before = new FileInfo(entry.AbsolutePath);
            PathSafety.RejectReparseAttributes(before.Attributes, entry.AbsolutePath);

            byte[] bytes;
            using (var stream = new FileStream(
                       entry.AbsolutePath,
                       FileMode.Open,
                       FileAccess.Read,
                       FileShare.Read,
                       bufferSize: 64 * 1024,
                       FileOptions.SequentialScan))
            {
                bytes = new byte[stream.Length];
                stream.ReadExactly(bytes);
            }

            var after = new FileInfo(entry.AbsolutePath);
            PathSafety.RejectReparseAttributes(after.Attributes, entry.AbsolutePath);
            if (before.Length != after.Length ||
                before.LastWriteTimeUtc.Ticks != after.LastWriteTimeUtc.Ticks ||
                after.Length != bytes.LongLength)
            {
                throw new DiscoveryException(
                    "SOURCE_DRIFT",
                    $"Source changed while it was read: '{entry.RelativePath}'.");
            }

            var record = new SourceFileRecord
            {
                Path = entry.RelativePath,
                ByteLength = bytes.LongLength,
                Sha256 = Hashing.Sha256(bytes),
            };
            files.Add(new SourceSnapshotFile(
                entry.AbsolutePath,
                entry.RelativePath,
                bytes,
                after.LastWriteTimeUtc.Ticks,
                record));
        }

        return new SourceSnapshot
        {
            Files = files,
            AggregateSha256 = Hashing.Aggregate(files.Select(CanonicalFileLine)),
        };
    }

    public static void AssertUnchanged(SourceSnapshot expected, string crystalRoot)
    {
        SourceSnapshot actual;
        try
        {
            actual = Capture(crystalRoot);
        }
        catch (DiscoveryException exception) when (exception.Code is
            "SOURCE_ROOT_MISSING" or "EMPTY_SOURCE_TREE" or "SOURCE_DRIFT")
        {
            throw new DiscoveryException(
                "SOURCE_DRIFT",
                $"Crystal source became unstable during final validation: {exception.Message}");
        }
        catch (Exception exception) when (exception is IOException or UnauthorizedAccessException)
        {
            throw new DiscoveryException(
                "SOURCE_DRIFT",
                $"Crystal source could not be reopened consistently during final validation: {exception.Message}");
        }

        if (!string.Equals(expected.AggregateSha256, actual.AggregateSha256, StringComparison.Ordinal) ||
            expected.Files.Count != actual.Files.Count)
        {
            throw new DiscoveryException(
                "SOURCE_DRIFT",
                "Crystal source inventory changed during candidate discovery.");
        }

        for (var index = 0; index < expected.Files.Count; index++)
        {
            var left = expected.Files[index];
            var right = actual.Files[index];
            if (!string.Equals(left.RelativePath, right.RelativePath, StringComparison.Ordinal) ||
                left.LastWriteUtcTicks != right.LastWriteUtcTicks ||
                !string.Equals(left.Record.Sha256, right.Record.Sha256, StringComparison.Ordinal) ||
                left.Record.ByteLength != right.Record.ByteLength)
            {
                throw new DiscoveryException(
                    "SOURCE_DRIFT",
                    $"Crystal source changed during candidate discovery near '{left.RelativePath}'.");
            }
        }
    }

    private static void ScanDirectory(
        string crystalRoot,
        string initialDirectory,
        ICollection<(string AbsolutePath, string RelativePath)> discovered)
    {
        var pending = new Stack<string>();
        pending.Push(initialDirectory);

        while (pending.Count > 0)
        {
            var directory = pending.Pop();
            PathSafety.EnsureExistingPathComponentsAreNotReparse(directory);
            var entries = Directory
                .EnumerateFileSystemEntries(directory)
                .OrderBy(path => Path.GetFileName(path), StringComparer.Ordinal)
                .ToArray();

            foreach (var entry in entries)
            {
                var attributes = File.GetAttributes(entry);
                PathSafety.RejectReparseAttributes(attributes, entry);
                var relative = PathSafety.ValidateRelativeSourcePath(
                    Path.GetRelativePath(crystalRoot, entry));

                if ((attributes & FileAttributes.Directory) != 0)
                {
                    if (IsBuildOutputDirectory(Path.GetFileName(entry)))
                    {
                        continue;
                    }

                    pending.Push(entry);
                    continue;
                }

                if (string.Equals(Path.GetExtension(entry), ".cs", StringComparison.OrdinalIgnoreCase))
                {
                    discovered.Add((Path.GetFullPath(entry), relative));
                }
            }
        }
    }

    private static string CanonicalFileLine(SourceSnapshotFile file) =>
        $"{file.RelativePath}\0{file.Record.ByteLength}\0{file.Record.Sha256}";

    private static bool IsBuildOutputDirectory(string name) =>
        string.Equals(name, "bin", StringComparison.OrdinalIgnoreCase) ||
        string.Equals(name, "obj", StringComparison.OrdinalIgnoreCase);
}
