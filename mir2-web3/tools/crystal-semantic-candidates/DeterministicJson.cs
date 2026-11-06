using System.Text.Encodings.Web;
using System.Text.Json;

namespace CrystalSemanticCandidates;

public static class DeterministicJson
{
    private static readonly JsonSerializerOptions Options = new()
    {
        Encoder = JavaScriptEncoder.Default,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        WriteIndented = true,
    };

    public static string Serialize(DiscoveryReport report) =>
        JsonSerializer.Serialize(report, Options) + "\n";
}
