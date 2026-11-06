namespace CrystalSemanticCandidates;

public sealed class DiscoveryException : Exception
{
    public DiscoveryException(string code, string message)
        : base($"{code}: {message}")
    {
        Code = code;
    }

    public string Code { get; }
}
