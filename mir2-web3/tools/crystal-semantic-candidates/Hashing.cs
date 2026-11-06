using System.Security.Cryptography;
using System.Text;

namespace CrystalSemanticCandidates;

internal static class Hashing
{
    public static string Sha256(ReadOnlySpan<byte> bytes) =>
        Convert.ToHexStringLower(SHA256.HashData(bytes));

    public static string Sha256(string value) => Sha256(Encoding.UTF8.GetBytes(value));

    public static string Aggregate(IEnumerable<string> canonicalLines) =>
        Sha256(string.Join("\n", canonicalLines));
}
