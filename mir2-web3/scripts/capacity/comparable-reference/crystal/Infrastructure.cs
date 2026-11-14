using System.Diagnostics;
using System.Reflection;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

namespace ComparableCrystal;

internal static class Guard
{
    public static void Need(bool value, string code) { if (!value) throw new InvalidDataException(code); }
}

internal static class Time
{
    private static readonly double Anchor = Now;
    private static readonly DateTimeOffset Wall = DateTimeOffset.UtcNow;
    public static double Now => Stopwatch.GetTimestamp() * 1000.0 / Stopwatch.Frequency;
    public static DateTimeOffset Utc(double at) => Wall.AddMilliseconds(at - Anchor);
    public static async Task Until(double at, CancellationToken token = default)
    {
        while (Now < at) await Task.Delay(TimeSpan.FromMilliseconds(Math.Min(20, Math.Max(1, at - Now))), token);
    }
}

internal static class Files
{
    public const string SharedSha = "fb90224b1ddd836b0bcfc959ddd29a715cc28f8b14552224e5f5a93f87936c25";
    public const string ServerSha = "8104a1cb313efbe58e68fa2229472c9b3335f2ee75e107b435309a116a7bce62";
    public const string DatabaseSha = "0fc51c446117be5c9f6bd7187285d008677b2bdb3abfe4718980153f0aa033b2";
    public const string MapSha = "d34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608";
    public static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase, PropertyNameCaseInsensitive = true, WriteIndented = true };
    public static readonly JsonSerializerOptions Compact = new() { PropertyNamingPolicy = JsonNamingPolicy.CamelCase };
    public static readonly string StateBase = Path.GetFullPath(Path.Combine(Path.GetTempPath(), "mir2-comparable-capacity-20261113"));
    public static string Hash(string path) { using var input = File.OpenRead(path); return Convert.ToHexString(SHA256.HashData(input)).ToLowerInvariant(); }
    public static string HashText(string value) => Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(value))).ToLowerInvariant();
    public static string StatePath(string value)
    {
        var full = Path.GetFullPath(value);
        Guard.Need(full.StartsWith(StateBase.TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase), "state_path_outside_assigned_C_root");
        for (var cursor = new DirectoryInfo(Path.GetDirectoryName(full)!); cursor is not null && cursor.FullName.Length >= StateBase.Length; cursor = cursor.Parent)
            if (cursor.Exists) Guard.Need((cursor.Attributes & FileAttributes.ReparsePoint) == 0, "state_junction_forbidden");
        return full;
    }
    public static JsonElement Read(string path, long maximum = 8 * 1024 * 1024)
    {
        Guard.Need(File.Exists(path) && new FileInfo(path).Length <= maximum, "bounded_json_file_required");
        using var data = JsonDocument.Parse(File.ReadAllBytes(path)); return data.RootElement.Clone();
    }
    public static T Read<T>(string path) => JsonSerializer.Deserialize<T>(File.ReadAllBytes(path), Json) ?? throw new InvalidDataException("json_missing");
    public static void Write(string path, object value)
    {
        path = StatePath(path); Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var bytes = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(value, Json) + "\n");
        using var stream = new FileStream(path, FileMode.CreateNew, FileAccess.Write);
        stream.Write(bytes);
    }
    public static void VerifyRuntime()
    {
        Guard.Need(Hash(typeof(Packet).Assembly.Location) == SharedSha, "frozen_Shared_dll_mismatch");
        Guard.Need(Hash(typeof(Server.MirEnvir.Envir).Assembly.Location) == ServerSha, "frozen_Server_Library_dll_mismatch");
    }
    public static string VerifySourceLock(string path)
    {
        var expected = Assembly.GetExecutingAssembly().GetCustomAttributes<AssemblyMetadataAttribute>().Single(x => x.Key == "ComparableSourceLockSha256").Value;
        Guard.Need(expected is not null && expected.Length == 64 && Hash(path) == expected, "compiled_source_lock_mismatch");
        var manifest = Read(path);
        var directory = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location)!;
        foreach (var dependency in manifest.GetProperty("runtimeDlls").EnumerateArray())
        {
            var name = dependency.GetProperty("name").GetString()!;
            Guard.Need(name == Path.GetFileName(name) && name.EndsWith(".dll", StringComparison.OrdinalIgnoreCase), "runtime_lock_filename_invalid");
            Guard.Need(Hash(Path.Combine(directory, name)) == dependency.GetProperty("sha256").GetString(), "locked_runtime_dependency_changed");
        }
        return expected!;
    }
}

internal sealed class Options
{
    public string Mode { get; }
    private readonly Dictionary<string, string> values = new(StringComparer.Ordinal);
    public Options(string[] args)
    {
        Guard.Need(args.Length > 0, "mode_required"); Mode = args[0];
        for (var i = 1; i < args.Length; i += 2)
        {
            Guard.Need(args[i].StartsWith("--") && i + 1 < args.Length && values.TryAdd(args[i][2..], args[i + 1]), "unique_name_value_options_required");
        }
    }
    public string Required(string key) => values.GetValueOrDefault(key) ?? throw new ArgumentException("missing_option_" + key);
    public string Value(string key, string fallback) => values.GetValueOrDefault(key, fallback);
    public int Integer(string key, int fallback) => int.Parse(Value(key, fallback.ToString(System.Globalization.CultureInfo.InvariantCulture)));
}

internal sealed class Evidence : IDisposable
{
    private readonly StreamWriter writer;
    private readonly object gate = new();
    private readonly long maximum;
    private bool closed;
    public long Rows { get; private set; }
    public bool Exceeded { get; private set; }
    public bool WriteFailed { get; private set; }
    public long RejectedAfterFatalFailure { get; private set; }
    public bool Complete { get { lock (gate) return !Exceeded && !WriteFailed; } }
    public Evidence(string path, long maximumRows)
    {
        Guard.Need(maximumRows is >= 10000 and <= 8000000, "bounded_evidence_rows_required"); maximum = maximumRows;
        path = Files.StatePath(path); Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        writer = new StreamWriter(new FileStream(path, FileMode.CreateNew, FileAccess.Write, FileShare.Read), new UTF8Encoding(false), 65536) { NewLine = "\n" };
    }
    public void Add(object value)
    {
        lock (gate)
        {
            if (closed) { WriteFailed = true; RejectedAfterFatalFailure++; return; }
            if (!Complete) { RejectedAfterFatalFailure++; return; }
            if (Rows >= maximum) { Exceeded = true; RejectedAfterFatalFailure++; return; }
            try { writer.WriteLine(JsonSerializer.Serialize(value, Files.Compact)); Rows++; }
            catch { WriteFailed = true; RejectedAfterFatalFailure++; }
        }
    }
    public void Flush() { lock (gate) try { writer.Flush(); } catch { WriteFailed = true; } }
    public void Seal()
    {
        lock (gate)
        {
            if (closed) return;
            try { writer.Flush(); writer.Dispose(); } catch { WriteFailed = true; }
            closed = true;
        }
    }
    public void Dispose() => Seal();
}
