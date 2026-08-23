using System.Text;

namespace CrystalSemanticCandidates;

public static class ExclusiveOutput
{
    private static readonly UTF8Encoding Utf8WithoutBom = new(encoderShouldEmitUTF8Identifier: false);

    public static void Write(string outputPath, string contents)
    {
        var fullPath = Path.GetFullPath(outputPath);
        var directory = Path.GetDirectoryName(fullPath)
            ?? throw new DiscoveryException(
                "OUTPUT_PATH_INVALID",
                $"No output directory for '{outputPath}'.");
        Directory.CreateDirectory(directory);
        PathSafety.EnsureExistingPathComponentsAreNotReparse(directory);
        PathSafety.ValidateRelativeSourcePath(Path.GetFileName(fullPath));

        if (File.Exists(fullPath) || Directory.Exists(fullPath))
        {
            throw new DiscoveryException(
                "OUTPUT_ALREADY_EXISTS",
                $"Refusing to overwrite existing output '{fullPath}'.");
        }

        var created = false;
        try
        {
            using var stream = new FileStream(
                fullPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 64 * 1024,
                FileOptions.WriteThrough);
            created = true;
            var bytes = Utf8WithoutBom.GetBytes(contents);
            stream.Write(bytes);
            stream.Flush(flushToDisk: true);
        }
        catch (IOException exception) when (!created && (File.Exists(fullPath) || Directory.Exists(fullPath)))
        {
            throw new DiscoveryException(
                "OUTPUT_ALREADY_EXISTS",
                $"Refusing to overwrite existing output '{fullPath}': {exception.Message}");
        }
        catch (Exception exception)
        {
            string? cleanupFailure = null;
            if (created && File.Exists(fullPath))
            {
                try
                {
                    File.Delete(fullPath);
                }
                catch (Exception cleanupException)
                {
                    cleanupFailure = cleanupException.Message;
                }
            }

            var suffix = cleanupFailure is null
                ? string.Empty
                : $" Partial output cleanup also failed: {cleanupFailure}";
            throw new DiscoveryException(
                "OUTPUT_WRITE_FAILED",
                $"Failed to create and flush output '{fullPath}': {exception.Message}.{suffix}");
        }
    }
}
