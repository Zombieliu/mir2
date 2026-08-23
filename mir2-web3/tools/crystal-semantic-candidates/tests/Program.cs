using System.Text.Json;
using CrystalSemanticCandidates;

internal static class Program
{
    private const string ComprehensiveFixture = """
        using System;
        using System.Collections.Generic;

        namespace Fixture;

        internal enum Mode { First = 1 }

        [PacketHandler]
        internal sealed class Player
        {
            private int field = Create();
            public int Property { get => field; set { field = value; } } = 2;
            public int RenderedValue => field;

            [MessageHandler]
            public Player() { }
            ~Player() { }

            public static Player operator +(Player left, Player right) => new();
            public static explicit operator int(Player value) => value.field;

            public void UpdateTick(object packet)
            {
                void Local() { Send(packet); }
                Func<int, int> transform = value => value + 1;
                var explicitArray = new int[1];
                var anonymous = new { field };
                Span<int> stack = stackalloc int[1];

                if (field > 0)
                {
                    Local();
                }
                else
                {
                    field = transform(field);
                }

                switch (field)
                {
                    case 0:
                        break;
                    default:
                        break;
                }

                var selected = field switch
                {
                    0 => 1,
                    _ => 2,
                };

                for (var index = 0; index < 1; index++) { }
                foreach (var value in new[] { selected }) { field += value; }
                foreach (var (left, right) in new[] { (1, 2) }) { field += left + right; }
                while (false) { }
                do { } while (false);

                try
                {
                    Broadcast(new object());
                    field += 1;
                    return;
                }
                catch (Exception) when (field < 0)
                {
                    throw;
                }
                finally
                {
                    PlaySound();
                }
            }

            public IEnumerable<int> Yielding()
            {
                yield return 1;
                yield break;
            }

            public int Throwing() => throw new InvalidOperationException();
            private static int Create() => 0;
            private void Send(object value) { }
            private void Broadcast(object value) { }
            private void PlaySound() { }
            private void SaveCheckpoint() { }
            private void RollRandomChance() { }
            private void OnMouseDownClick() { }
            private void DrawSprite() { }
        }

        #if NEVER
        internal sealed class Disabled
        {
            public void SavePacket() { NetworkSend(); }
        }
        #else
        internal sealed class Enabled { }
        #endif
        """;

    private static readonly (string Name, Action Body)[] Tests =
    [
        ("Roslyn syntax categories and hints", SyntaxCategoriesAndHints),
        ("Preprocessor and disabled-text boundaries", PreprocessorBoundaries),
        ("Deterministic JSON, ordering, ids, and aggregate hashes", StableOutput),
        ("Parse errors fail closed", ParseErrorsFailClosed),
        ("Source mutation and file-set drift fail closed", SourceDriftFailsClosed),
        ("Unsafe Windows aliases and path collisions fail closed", UnsafePathsFailClosed),
        ("Reparse attributes fail closed", ReparseAttributesFailClosed),
        ("Compiler build-output directories are explicitly excluded", BuildOutputsAreExcluded),
        ("Empty fixed source trees fail closed", EmptySourceTreeFailsClosed),
        ("Candidate discovery never claims full source or semantic inventory completion", CompletionBoundaryIsHardCoded),
        ("Explicit output is flushed and never overwritten", ExplicitOutputIsNeverOverwritten),
    ];

    public static int Main()
    {
        var failures = 0;
        foreach (var test in Tests)
        {
            try
            {
                test.Body();
                Console.WriteLine($"PASS {test.Name}");
            }
            catch (Exception exception)
            {
                failures++;
                Console.Error.WriteLine($"FAIL {test.Name}: {exception}");
            }
        }

        Console.WriteLine($"RESULT {Tests.Length - failures}/{Tests.Length} passed");
        return failures == 0 ? 0 : 1;
    }

    private static void SyntaxCategoriesAndHints()
    {
        using var tree = TemporaryCrystalTree.WithSource(ComprehensiveFixture);
        var report = DiscoveryEngine.Discover(tree.Root);
        var categories = report.Candidates.Select(candidate => candidate.Category).ToHashSet(StringComparer.Ordinal);
        var expected = new[]
        {
            "type-declaration",
            "callable.method",
            "callable.constructor",
            "callable.destructor",
            "callable.operator",
            "callable.conversion",
            "callable.accessor",
            "callable.local-function",
            "callable.lambda-or-anonymous-function",
            "control.if",
            "control.else",
            "control.switch-section",
            "control.switch-expression-arm",
            "control.loop.for",
            "control.loop.foreach",
            "control.loop.foreach-variable",
            "control.loop.while",
            "control.loop.do",
            "control.catch",
            "control.catch-filter",
            "control.finally",
            "operation.invocation",
            "operation.object-creation",
            "operation.implicit-object-creation",
            "operation.array-creation",
            "operation.implicit-array-creation",
            "operation.anonymous-object-creation",
            "operation.stackalloc-array-creation",
            "operation.assignment",
            "operation.return",
            "operation.throw-statement",
            "operation.throw-expression",
            "operation.yield",
            "initializer.field-or-property",
            "initializer.enum-member",
            "initializer.property-expression-body",
            "attribute",
        };

        foreach (var category in expected)
        {
            Assert(categories.Contains(category), $"Missing syntax category '{category}'.");
        }

        var hintTags = report.Candidates
            .SelectMany(candidate => candidate.Hints)
            .Select(hint => hint.Tag)
            .ToHashSet(StringComparer.Ordinal);
        foreach (var hint in new[]
                 {
                     "packet-handler",
                     "timer-update-tick",
                     "save-load-persistence",
                     "rng",
                     "network-send-broadcast",
                     "ui-event",
                     "render",
                     "audio",
                     "attribute-declared-handler",
                 })
        {
            Assert(hintTags.Contains(hint), $"Missing explainable hint '{hint}'.");
        }

        Assert(report.Candidates.All(candidate => candidate.StartLine > 0 && candidate.StartColumn > 0),
            "Every candidate must include a one-based line and column.");
        Assert(report.Candidates.All(candidate => candidate.ParentSymbol.Length > 0),
            "Every candidate must include a syntactic parent symbol.");
        Assert(report.Candidates.All(candidate => candidate.ContentSha256.Length == 64),
            "Every candidate must bind exact syntax content with SHA-256.");
    }

    private static void PreprocessorBoundaries()
    {
        using var tree = TemporaryCrystalTree.WithSource(ComprehensiveFixture);
        var report = DiscoveryEngine.Discover(tree.Root);
        Assert(report.Candidates.Any(candidate => candidate.Category == "preprocessor-directive-boundary"),
            "Preprocessor directive boundaries were not emitted.");
        Assert(report.Candidates.Any(candidate => candidate.Category == "preprocessor-disabled-text-boundary"),
            "Disabled preprocessor text was not emitted as an explicit boundary.");
        Assert(report.Candidates
                .Where(candidate => candidate.Category.StartsWith("preprocessor-", StringComparison.Ordinal))
                .All(candidate => candidate.Hints.Any(hint => hint.Tag == "preprocessor")),
            "Preprocessor candidates must explain their boundary tag.");
    }

    private static void StableOutput()
    {
        using var tree = TemporaryCrystalTree.WithSource(ComprehensiveFixture);
        var first = DiscoveryEngine.Discover(tree.Root);
        var second = DiscoveryEngine.Discover(tree.Root);
        var firstJson = DeterministicJson.Serialize(first);
        var secondJson = DeterministicJson.Serialize(second);

        Assert(firstJson == secondJson, "Repeated unchanged runs must be byte-for-byte stable.");
        Assert(first.ParserConfiguration.Engine == "Microsoft.CodeAnalysis.CSharp",
            "The report must identify Roslyn as the parser engine.");
        Assert(first.ParserConfiguration.PreprocessorSymbols.Count == 0,
            "The report must disclose the empty default preprocessor-symbol set.");
        Assert(first.CandidateAggregateSha256 == second.CandidateAggregateSha256,
            "Candidate aggregate hash changed between stable runs.");
        Assert(first.ReportAggregateSha256 == second.ReportAggregateSha256,
            "Report aggregate hash changed between stable runs.");
        Assert(first.Candidates.Select(candidate => candidate.CandidateId).SequenceEqual(
                second.Candidates.Select(candidate => candidate.CandidateId),
                StringComparer.Ordinal),
            "Candidate ids changed between stable runs.");

        var sorted = first.Candidates.OrderBy(candidate => candidate.SourcePath, StringComparer.Ordinal)
            .ThenBy(candidate => candidate.SpanStartUtf16)
            .ThenBy(candidate => candidate.SpanLengthUtf16)
            .ThenBy(candidate => candidate.SyntaxKind, StringComparer.Ordinal)
            .ThenBy(candidate => candidate.CandidateId, StringComparer.Ordinal)
            .Select(candidate => candidate.CandidateId);
        Assert(first.Candidates.Select(candidate => candidate.CandidateId).SequenceEqual(sorted),
            "Candidate output is not canonically sorted.");

        using var json = JsonDocument.Parse(firstJson);
        Assert(json.RootElement.GetProperty("schemaVersion").GetString() == "crystal-semantic-candidates/v2",
            "Serialized schema version is missing or unstable.");
        Assert(json.RootElement.GetProperty("sourceSnapshotCaptured").GetBoolean(),
            "Serialized output must machine-mark the scoped snapshot as captured.");
        Assert(!json.RootElement.GetProperty("sourceFileInventoryComplete").GetBoolean(),
            "Serialized output must not promote snapshot capture to source inventory completion.");
    }

    private static void ParseErrorsFailClosed()
    {
        using var tree = TemporaryCrystalTree.WithSource("namespace Broken; class Missing { void M( { }");
        AssertDiscoveryError("PARSE_ERROR", () => DiscoveryEngine.Discover(tree.Root));
    }

    private static void SourceDriftFailsClosed()
    {
        using var mutationTree = TemporaryCrystalTree.WithSource("namespace Fixture; class A { void M() { } }");
        var source = mutationTree.SourcePath;
        AssertDiscoveryError(
            "SOURCE_DRIFT",
            () => DiscoveryEngine.Discover(
                mutationTree.Root,
                new DiscoveryOptions
                {
                    BeforeFinalSourceValidation = () => File.AppendAllText(source, "\n// mutation"),
                }));

        using var fileSetTree = TemporaryCrystalTree.WithSource("namespace Fixture; class A { }");
        AssertDiscoveryError(
            "SOURCE_DRIFT",
            () => DiscoveryEngine.Discover(
                fileSetTree.Root,
                new DiscoveryOptions
                {
                    BeforeFinalSourceValidation = () => File.WriteAllText(
                        Path.Combine(fileSetTree.Root, "Server", "Added.cs"),
                        "namespace Fixture; class Added { }"),
                }));

        using var deletionTree = TemporaryCrystalTree.WithSource("namespace Fixture; class A { }");
        AssertDiscoveryError(
            "SOURCE_DRIFT",
            () => DiscoveryEngine.Discover(
                deletionTree.Root,
                new DiscoveryOptions
                {
                    BeforeFinalSourceValidation = () => File.Delete(deletionTree.SourcePath),
                }));
    }

    private static void UnsafePathsFailClosed()
    {
        foreach (var path in new[]
                 {
                     "Client/CON.cs",
                     "Client/aux.txt.cs",
                     "Server/Trailing.cs.",
                     "Shared/control\u0001.cs",
                     "Client/stream:name.cs",
                     "Client/invalid?.cs",
                     "Client/invalid*.cs",
                     "Client/invalid|.cs",
                     "Client/../escape.cs",
                     "Client/e\u0301.cs",
                 })
        {
            AssertDiscoveryError("", () => PathSafety.ValidateRelativeSourcePath(path));
        }

        AssertDiscoveryError(
            "PATH_ALIAS_COLLISION",
            () => PathSafety.EnsureNoAliasCollisions(["Client/Player.cs", "client/player.cs"]));
        AssertDiscoveryError(
            "PATH_ALIAS_COLLISION",
            () => PathSafety.EnsureNoAliasCollisions(["Client/Player.cs", "Client/Player.cs"]));
    }

    private static void ReparseAttributesFailClosed()
    {
        AssertDiscoveryError(
            "REPARSE_POINT_REJECTED",
            () => PathSafety.RejectReparseAttributes(FileAttributes.ReparsePoint, "fixture-link"));

        using var tree = TemporaryCrystalTree.WithSource("namespace Fixture; class A { }");
        var target = Path.Combine(tree.Root, "Server", "Target.cs");
        var link = Path.Combine(tree.Root, "Client", "Linked.cs");
        File.WriteAllText(target, "namespace Fixture; class Target { }");
        try
        {
            File.CreateSymbolicLink(link, target);
        }
        catch (Exception exception) when (exception is UnauthorizedAccessException or IOException or PlatformNotSupportedException)
        {
            throw new InvalidOperationException(
                "The test host cannot create a fixture symlink; reparse rejection remains fail-closed but end-to-end validation requires symlink capability.",
                exception);
        }

        AssertDiscoveryError("REPARSE_POINT_REJECTED", () => DiscoveryEngine.Discover(tree.Root));
    }

    private static void EmptySourceTreeFailsClosed()
    {
        using var tree = TemporaryCrystalTree.Empty();
        AssertDiscoveryError("EMPTY_SOURCE_TREE", () => DiscoveryEngine.Discover(tree.Root));
    }

    private static void BuildOutputsAreExcluded()
    {
        using var tree = TemporaryCrystalTree.WithSource("namespace Fixture; class A { }");
        var generatedDirectory = Path.Combine(tree.Root, "Client", "obj", "Debug");
        Directory.CreateDirectory(generatedDirectory);
        File.WriteAllText(
            Path.Combine(generatedDirectory, "Generated.cs"),
            "namespace Generated; class MustNotEnterSourceInventory { }");

        var report = DiscoveryEngine.Discover(tree.Root);
        Assert(report.SourceDirectoryExclusions.SequenceEqual(["bin", "obj"]),
            "The report must disclose the fixed compiler-output exclusions.");
        Assert(report.SourceFiles.Count == 1, "obj/bin generated C# files must not enter the input inventory.");
        Assert(report.SourceSnapshotCaptured, "The scoped authored-input snapshot should be marked captured.");
        Assert(!report.SourceFileInventoryComplete,
            "Excluding obj is a scoped snapshot, not a Git-clean strong-no-follow source inventory.");
        Assert(report.SourceFiles.All(file => !file.Path.Contains("/obj/", StringComparison.OrdinalIgnoreCase)),
            "An obj path leaked into the source inventory.");
    }

    private static void CompletionBoundaryIsHardCoded()
    {
        using var tree = TemporaryCrystalTree.WithSource("namespace Fixture; class A { void M() { } }");
        var report = DiscoveryEngine.Discover(tree.Root);
        Assert(report.SourceSnapshotCaptured, "The stable scoped authored-source snapshot should be captured.");
        Assert(report.SourceSnapshotFileCount == 1, "Snapshot file count must bind the captured scope.");
        Assert(!report.GitCleanBound, "This tool does not bind a clean Crystal Git state.");
        Assert(!report.StrongNoFollowBound, "This managed Windows scanner does not provide strong no-follow.");
        Assert(!report.SourceFileInventoryComplete,
            "Snapshot capture must not be promoted to source-file inventory completion.");
        Assert(report.SemanticCandidateDiscoveryComplete,
            "AST syntax candidate discovery should be complete only for the captured scoped snapshot.");
        Assert(report.SemanticCandidateDiscoveryScope.Contains("captured snapshot", StringComparison.Ordinal),
            "The machine-readable discovery scope must name the captured snapshot boundary.");
        Assert(report.CompletionBoundary.Contains("403-file inventory", StringComparison.Ordinal),
            "The completion boundary must distinguish the existing generated-file inventory.");
        Assert(!report.SemanticDispositionComplete, "Candidate discovery must not claim disposition completion.");
        Assert(!report.SemanticLeafInventoryComplete, "Candidate discovery must not claim semantic-leaf completion.");
        Assert(!report.InventoryComplete, "Candidate discovery must never claim total inventory completion.");
        Assert(report.Candidates.All(candidate => candidate.SemanticLeaf is null),
            "Hints or candidates must not automatically become semantic leaves.");
    }

    private static void ExplicitOutputIsNeverOverwritten()
    {
        using var tree = TemporaryCrystalTree.Empty();
        var output = Path.Combine(tree.Root, "candidate-output.json");
        const string original = "{\"first\":true}\n";

        ExclusiveOutput.Write(output, original);
        Assert(File.ReadAllBytes(output).SequenceEqual(new System.Text.UTF8Encoding(false).GetBytes(original)),
            "Explicit output must be exact UTF-8 without a byte-order mark.");
        AssertDiscoveryError(
            "OUTPUT_ALREADY_EXISTS",
            () => ExclusiveOutput.Write(output, "{\"second\":true}\n"));
        Assert(File.ReadAllText(output) == original,
            "A refused second write changed the existing evidence bytes.");
    }

    private static void AssertDiscoveryError(string expectedCode, Action action)
    {
        try
        {
            action();
        }
        catch (DiscoveryException exception)
        {
            if (expectedCode.Length == 0 || exception.Code == expectedCode)
            {
                return;
            }

            throw new InvalidOperationException(
                $"Expected DiscoveryException '{expectedCode}', got '{exception.Code}'.",
                exception);
        }

        throw new InvalidOperationException($"Expected DiscoveryException '{expectedCode}', but no exception was thrown.");
    }

    private static void Assert(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }

    private sealed class TemporaryCrystalTree : IDisposable
    {
        private TemporaryCrystalTree()
        {
            Root = Path.Combine(
                Path.GetTempPath(),
                "crystal-semantic-candidates-tests",
                Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(Path.Combine(Root, "Client"));
            Directory.CreateDirectory(Path.Combine(Root, "Server"));
            Directory.CreateDirectory(Path.Combine(Root, "Shared"));
            SourcePath = Path.Combine(Root, "Client", "Fixture.cs");
        }

        public string Root { get; }

        public string SourcePath { get; }

        public static TemporaryCrystalTree Empty() => new();

        public static TemporaryCrystalTree WithSource(string source)
        {
            var tree = new TemporaryCrystalTree();
            File.WriteAllText(tree.SourcePath, source, new System.Text.UTF8Encoding(false));
            return tree;
        }

        public void Dispose()
        {
            if (!Directory.Exists(Root))
            {
                return;
            }

            var resolved = Path.GetFullPath(Root);
            var expectedParent = Path.GetFullPath(
                Path.Combine(Path.GetTempPath(), "crystal-semantic-candidates-tests"));
            if (!resolved.StartsWith(expectedParent + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            {
                throw new InvalidOperationException($"Refusing to remove unexpected test directory '{resolved}'.");
            }

            Directory.Delete(resolved, recursive: true);
        }
    }
}
