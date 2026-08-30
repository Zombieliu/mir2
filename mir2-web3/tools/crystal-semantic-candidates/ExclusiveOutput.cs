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
        EnsureSafeDirectory(directory);
        PathSafety.ValidateRelativeSourcePath(Path.GetFileName(fullPath));

        if (File.Exists(fullPath) || Directory.Exists(fullPath))
        {
            throw new DiscoveryException(
                "OUTPUT_ALREADY_EXISTS",
                $"Refusing to overwrite existing output '{fullPath}'.");
        }

        var temporaryPath = Path.Combine(
            directory,
            $".{Path.GetFileName(fullPath)}.{Guid.NewGuid():N}.tmp");
        Exception? failure = null;
        try
        {
            using var stream = new FileStream(
                temporaryPath,
                FileMode.CreateNew,
                FileAccess.Write,
                FileShare.None,
                bufferSize: 64 * 1024,
                FileOptions.WriteThrough);
            var bytes = Utf8WithoutBom.GetBytes(contents);
            stream.Write(bytes);
            stream.Flush(flushToDisk: true);
            stream.Dispose();
            File.Move(temporaryPath, fullPath, overwrite: false);
            return;
        }
        catch (IOException exception) when (File.Exists(fullPath) || Directory.Exists(fullPath))
        {
            failure = new DiscoveryException(
                "OUTPUT_ALREADY_EXISTS",
                $"Refusing to overwrite existing output '{fullPath}': {exception.Message}");
        }
        catch (Exception exception)
        {
            failure = new DiscoveryException(
                "OUTPUT_WRITE_FAILED",
                $"Failed to create and flush output '{fullPath}': {exception.Message}.");
        }

        string? cleanupFailure = null;
        if (File.Exists(temporaryPath))
        {
            try
            {
                File.Delete(temporaryPath);
            }
            catch (Exception cleanupException)
            {
                cleanupFailure = cleanupException.Message;
            }
        }
        if (cleanupFailure is not null)
        {
            throw new DiscoveryException(
                "OUTPUT_CLEANUP_FAILED",
                $"{failure?.Message} Temporary output '{temporaryPath}' cleanup also failed: {cleanupFailure}");
        }
        throw failure ?? new DiscoveryException(
            "OUTPUT_WRITE_FAILED",
            $"Failed to create output '{fullPath}' for an unknown reason.");
    }

    private static void EnsureSafeDirectory(string directory)
    {
        var nearestExisting = directory;
        while (!Directory.Exists(nearestExisting) && !File.Exists(nearestExisting))
        {
            nearestExisting = Path.GetDirectoryName(nearestExisting)
                ?? throw new DiscoveryException(
                    "OUTPUT_PATH_INVALID",
                    $"Cannot locate an existing ancestor for output directory '{directory}'.");
        }
        PathSafety.EnsureExistingPathComponentsAreNotReparse(nearestExisting);
        Directory.CreateDirectory(directory);
        PathSafety.EnsureExistingPathComponentsAreNotReparse(directory);
    }
}
