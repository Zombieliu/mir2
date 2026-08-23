using System.Globalization;
using System.Text;

namespace CrystalSemanticCandidates;

public static class PathSafety
{
    private static readonly HashSet<string> ReservedDosNames = new(StringComparer.OrdinalIgnoreCase)
    {
        "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$",
        "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
        "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    };

    public static string ValidateRelativeSourcePath(string relativePath)
    {
        if (string.IsNullOrWhiteSpace(relativePath))
        {
            throw new DiscoveryException("UNSAFE_PATH", "A source path is empty.");
        }

        var normalized = relativePath.Replace('\\', '/');
        if (Path.IsPathRooted(normalized) || normalized.StartsWith("/", StringComparison.Ordinal))
        {
            throw new DiscoveryException("UNSAFE_PATH", $"Source path must be relative: '{relativePath}'.");
        }

        var segments = normalized.Split('/');
        foreach (var segment in segments)
        {
            ValidateSegment(segment, relativePath);
        }

        return string.Join('/', segments.Select(segment => segment.Normalize(NormalizationForm.FormC)));
    }

    public static void EnsureNoAliasCollisions(IEnumerable<string> relativePaths)
    {
        var aliases = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (var candidate in relativePaths)
        {
            var normalized = ValidateRelativeSourcePath(candidate);
            var aliasKey = normalized.Normalize(NormalizationForm.FormC).ToUpperInvariant();
            if (aliases.TryGetValue(aliasKey, out var prior) &&
                !string.Equals(prior, normalized, StringComparison.Ordinal))
            {
                throw new DiscoveryException(
                    "PATH_ALIAS_COLLISION",
                    $"'{prior}' and '{normalized}' collapse to the same Windows path alias.");
            }

            if (aliases.ContainsKey(aliasKey))
            {
                throw new DiscoveryException(
                    "PATH_ALIAS_COLLISION",
                    $"Duplicate source path '{normalized}' was observed.");
            }

            aliases.Add(aliasKey, normalized);
        }
    }

    public static void RejectReparseAttributes(FileAttributes attributes, string path)
    {
        if ((attributes & FileAttributes.ReparsePoint) != 0)
        {
            throw new DiscoveryException(
                "REPARSE_POINT_REJECTED",
                $"Symlink, junction, or other reparse input is forbidden: '{path}'.");
        }
    }

    public static void EnsureExistingPathComponentsAreNotReparse(string fullPath)
    {
        var normalized = Path.GetFullPath(fullPath);
        var root = Path.GetPathRoot(normalized)
            ?? throw new DiscoveryException("UNSAFE_PATH", $"Cannot determine path root for '{fullPath}'.");
        var remainder = normalized[root.Length..];
        var current = root;

        foreach (var segment in remainder.Split(
                     Path.DirectorySeparatorChar,
                     StringSplitOptions.RemoveEmptyEntries))
        {
            current = Path.Combine(current, segment);
            if (!File.Exists(current) && !Directory.Exists(current))
            {
                break;
            }

            var attributes = File.GetAttributes(current);
            RejectReparseAttributes(attributes, current);
            if (new FileInfo(current).LinkTarget is not null || new DirectoryInfo(current).LinkTarget is not null)
            {
                throw new DiscoveryException(
                    "REPARSE_POINT_REJECTED",
                    $"Symbolic-link input is forbidden: '{current}'.");
            }
        }
    }

    private static void ValidateSegment(string segment, string originalPath)
    {
        if (segment is "" or "." or "..")
        {
            throw new DiscoveryException("UNSAFE_PATH", $"Unsafe segment in '{originalPath}'.");
        }

        if (segment.EndsWith(' ') || segment.EndsWith('.'))
        {
            throw new DiscoveryException(
                "WINDOWS_PATH_ALIAS",
                $"Trailing dot/space segment is forbidden in '{originalPath}'.");
        }

        if (segment.Any(character => char.IsControl(character)))
        {
            throw new DiscoveryException("UNSAFE_PATH", $"Control character in '{originalPath}'.");
        }

        if (segment.IndexOfAny(['<', '>', ':', '"', '|', '?', '*']) >= 0)
        {
            throw new DiscoveryException(
                "WINDOWS_PATH_ALIAS",
                $"Windows-invalid filename character is forbidden in '{originalPath}'.");
        }

        var stem = segment.Split('.')[0].TrimEnd(' ', '.');
        if (ReservedDosNames.Contains(stem))
        {
            throw new DiscoveryException(
                "WINDOWS_PATH_ALIAS",
                $"Reserved DOS device segment '{segment}' is forbidden in '{originalPath}'.");
        }

        if (!string.Equals(segment, segment.Normalize(NormalizationForm.FormC), StringComparison.Ordinal))
        {
            throw new DiscoveryException(
                "WINDOWS_PATH_ALIAS",
                $"Non-NFC path segment '{segment}' is forbidden in '{originalPath}'.");
        }
    }
}
