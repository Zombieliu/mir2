using System.Text.Json.Serialization;

namespace CrystalSemanticCandidates;

public sealed class DiscoveryReport
{
    [JsonPropertyOrder(0)]
    public string SchemaVersion => "crystal-semantic-candidates/v2";

    [JsonPropertyOrder(1)]
    public string SourceRootLabel => "Crystal";

    [JsonPropertyOrder(2)]
    public IReadOnlyList<string> SourceRoots => ["Client", "Server", "Shared"];

    [JsonPropertyOrder(3)]
    public required ParserConfiguration ParserConfiguration { get; init; }

    [JsonPropertyOrder(4)]
    public IReadOnlyList<string> SourceDirectoryExclusions => ["bin", "obj"];

    [JsonPropertyOrder(5)]
    public required IReadOnlyList<SourceFileRecord> SourceFiles { get; init; }

    [JsonPropertyOrder(6)]
    public required string SourceSnapshotSha256 { get; init; }

    [JsonPropertyOrder(7)]
    public int SourceSnapshotFileCount => SourceFiles.Count;

    [JsonPropertyOrder(8)]
    public string SourceSnapshotScope =>
        "captured-authored-csharp-under-Crystal-Client-Server-Shared-excluding-bin-obj";

    [JsonPropertyOrder(9)]
    public bool SourceSnapshotCaptured => true;

    [JsonPropertyOrder(10)]
    public bool GitCleanBound => false;

    [JsonPropertyOrder(11)]
    public bool StrongNoFollowBound => false;

    [JsonPropertyOrder(12)]
    public bool SourceFileInventoryComplete =>
        SourceSnapshotCaptured && GitCleanBound && StrongNoFollowBound;

    [JsonPropertyOrder(13)]
    public string SemanticCandidateDiscoveryScope =>
        $"Roslyn syntax candidates for captured snapshot {SourceSnapshotSha256} across {SourceSnapshotFileCount} authored .cs files under Crystal/Client, Crystal/Server, and Crystal/Shared after excluding bin/obj; no Git-clean or strong no-follow inventory claim.";

    [JsonPropertyOrder(14)]
    public bool SemanticCandidateDiscoveryComplete => true;

    [JsonPropertyOrder(15)]
    public bool SemanticDispositionComplete => false;

    [JsonPropertyOrder(16)]
    public bool SemanticLeafInventoryComplete => false;

    [JsonPropertyOrder(17)]
    public bool InventoryComplete => false;

    [JsonPropertyOrder(18)]
    public string CompletionBoundary =>
        $"This report stably captured {SourceSnapshotFileCount} scoped authored C# inputs after excluding bin/obj and completed Roslyn syntax-candidate discovery only for that snapshot. sourceFileInventoryComplete remains false because the report binds neither a clean Crystal Git state nor a strong no-follow filesystem read. The separate existing 403-file inventory includes nine obj/Debug generated C# files and is not the same denominator or aggregate. A trusted denominator migration plus reviewed disposition must map every candidate to semantic leaves, supporting code, or a signed non-observable reason before sourceFileInventoryComplete, semanticLeafInventoryComplete, or inventoryComplete can become true.";

    [JsonPropertyOrder(19)]
    public required IReadOnlyList<SyntaxCandidate> Candidates { get; init; }

    [JsonPropertyOrder(20)]
    public required string CandidateAggregateSha256 { get; init; }

    [JsonPropertyOrder(21)]
    public required string ReportAggregateSha256 { get; init; }
}

public sealed record ParserConfiguration
{
    [JsonPropertyOrder(0)]
    public required string Engine { get; init; }

    [JsonPropertyOrder(1)]
    public required string RoslynAssemblyVersion { get; init; }

    [JsonPropertyOrder(2)]
    public required string LanguageVersion { get; init; }

    [JsonPropertyOrder(3)]
    public required string DocumentationMode { get; init; }

    [JsonPropertyOrder(4)]
    public required string SourceCodeKind { get; init; }

    [JsonPropertyOrder(5)]
    public IReadOnlyList<string> PreprocessorSymbols => [];
}

public sealed record SourceFileRecord
{
    [JsonPropertyOrder(0)]
    public required string Path { get; init; }

    [JsonPropertyOrder(1)]
    public required long ByteLength { get; init; }

    [JsonPropertyOrder(2)]
    public required string Sha256 { get; init; }
}

public sealed record SyntaxCandidate
{
    [JsonPropertyOrder(0)]
    public required string CandidateId { get; init; }

    [JsonPropertyOrder(1)]
    public required string SourcePath { get; init; }

    [JsonPropertyOrder(2)]
    public required string Category { get; init; }

    [JsonPropertyOrder(3)]
    public required string SyntaxKind { get; init; }

    [JsonPropertyOrder(4)]
    public required string ParentSymbol { get; init; }

    [JsonPropertyOrder(5)]
    public string ParentSymbolBasis => "syntactic";

    [JsonPropertyOrder(6)]
    public required int SpanStartUtf16 { get; init; }

    [JsonPropertyOrder(7)]
    public required int SpanLengthUtf16 { get; init; }

    [JsonPropertyOrder(8)]
    public required int StartLine { get; init; }

    [JsonPropertyOrder(9)]
    public required int StartColumn { get; init; }

    [JsonPropertyOrder(10)]
    public required int EndLine { get; init; }

    [JsonPropertyOrder(11)]
    public required int EndColumn { get; init; }

    [JsonPropertyOrder(12)]
    public required string ContentSha256 { get; init; }

    [JsonPropertyOrder(13)]
    public string Disposition => "UNDISPOSITIONED_CANDIDATE";

    [JsonPropertyOrder(14)]
    public bool? SemanticLeaf => null;

    [JsonPropertyOrder(15)]
    public required IReadOnlyList<CandidateHint> Hints { get; init; }
}

public sealed record CandidateHint
{
    [JsonPropertyOrder(0)]
    public required string Tag { get; init; }

    [JsonPropertyOrder(1)]
    public required string Evidence { get; init; }
}
