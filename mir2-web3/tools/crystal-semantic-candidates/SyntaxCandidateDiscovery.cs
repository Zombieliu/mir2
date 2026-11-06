using System.Text;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.CSharp.Syntax;
using Microsoft.CodeAnalysis.Text;

namespace CrystalSemanticCandidates;

public sealed class DiscoveryOptions
{
    public Action? BeforeFinalSourceValidation { get; init; }
}

public static class DiscoveryEngine
{
    private static readonly CSharpParseOptions ParseOptions = new(
        languageVersion: LanguageVersion.Latest,
        documentationMode: DocumentationMode.Parse,
        kind: SourceCodeKind.Regular,
        preprocessorSymbols: []);

    public static DiscoveryReport Discover(string crystalRoot, DiscoveryOptions? options = null)
    {
        var fullRoot = Path.GetFullPath(crystalRoot);
        var sourceSnapshot = SourceInventory.Capture(fullRoot);
        var candidates = new List<SyntaxCandidate>();

        foreach (var file in sourceSnapshot.Files)
        {
            var sourceText = ReadSourceText(file);
            var syntaxTree = CSharpSyntaxTree.ParseText(
                sourceText,
                ParseOptions,
                path: file.RelativePath);
            RejectParseErrors(syntaxTree, file.RelativePath);
            var root = syntaxTree.GetRoot();

            foreach (var node in root.DescendantNodesAndSelf(descendIntoTrivia: true))
            {
                var category = CandidateClassifier.Classify(node);
                if (category is not null)
                {
                    candidates.Add(CandidateFactory.FromNode(
                        file.RelativePath,
                        syntaxTree,
                        sourceText,
                        node,
                        category));
                }
            }

            foreach (var trivia in root.DescendantTrivia(descendIntoTrivia: true))
            {
                if (trivia.IsKind(SyntaxKind.DisabledTextTrivia))
                {
                    candidates.Add(CandidateFactory.FromTrivia(
                        file.RelativePath,
                        syntaxTree,
                        sourceText,
                        trivia,
                        "preprocessor-disabled-text-boundary"));
                }
            }
        }

        candidates.Sort(CandidateComparer.Instance);
        RejectDuplicateCandidateIds(candidates);
        options?.BeforeFinalSourceValidation?.Invoke();
        SourceInventory.AssertUnchanged(sourceSnapshot, fullRoot);

        var candidateAggregate = Hashing.Aggregate(candidates.Select(CandidateFactory.CanonicalLine));
        var parserConfiguration = CreateParserConfiguration();
        var reportAggregate = Hashing.Aggregate(
        [
            "crystal-semantic-candidates/v2",
            $"parser={parserConfiguration.Engine},{parserConfiguration.RoslynAssemblyVersion},{parserConfiguration.LanguageVersion},{parserConfiguration.DocumentationMode},{parserConfiguration.SourceCodeKind},symbols=[]",
            sourceSnapshot.AggregateSha256,
            candidateAggregate,
            $"sourceSnapshotFileCount={sourceSnapshot.Files.Count}",
            "sourceSnapshotScope=captured-authored-csharp-under-Crystal-Client-Server-Shared-excluding-bin-obj",
            "sourceSnapshotCaptured=true",
            "sourceDirectoryExclusions=bin,obj",
            "gitCleanBound=false",
            "strongNoFollowBound=false",
            "sourceFileInventoryComplete=false",
            "semanticCandidateDiscoveryScope=captured-snapshot-syntax-only-excluding-bin-obj",
            "semanticCandidateDiscoveryComplete=true",
            "semanticDispositionComplete=false",
            "semanticLeafInventoryComplete=false",
            "inventoryComplete=false",
        ]);

        return new DiscoveryReport
        {
            ParserConfiguration = parserConfiguration,
            SourceFiles = sourceSnapshot.Files.Select(file => file.Record).ToArray(),
            SourceSnapshotSha256 = sourceSnapshot.AggregateSha256,
            Candidates = candidates,
            CandidateAggregateSha256 = candidateAggregate,
            ReportAggregateSha256 = reportAggregate,
        };
    }

    private static ParserConfiguration CreateParserConfiguration() => new()
    {
        Engine = "Microsoft.CodeAnalysis.CSharp",
        RoslynAssemblyVersion = typeof(CSharpSyntaxTree).Assembly.GetName().Version?.ToString()
            ?? throw new DiscoveryException("ROSLYN_VERSION_UNKNOWN", "Cannot determine the loaded Roslyn C# assembly version."),
        LanguageVersion = ParseOptions.LanguageVersion.ToString(),
        DocumentationMode = ParseOptions.DocumentationMode.ToString(),
        SourceCodeKind = ParseOptions.Kind.ToString(),
    };

    private static SourceText ReadSourceText(SourceSnapshotFile file)
    {
        try
        {
            using var stream = new MemoryStream(file.Bytes, writable: false);
            return SourceText.From(
                stream,
                encoding: null,
                checksumAlgorithm: SourceHashAlgorithm.Sha256,
                throwIfBinaryDetected: true,
                canBeEmbedded: false);
        }
        catch (Exception exception) when (exception is DecoderFallbackException or InvalidDataException)
        {
            throw new DiscoveryException(
                "SOURCE_DECODE_ERROR",
                $"Cannot decode C# source '{file.RelativePath}': {exception.Message}");
        }
    }

    private static void RejectParseErrors(SyntaxTree tree, string relativePath)
    {
        var errors = tree.GetDiagnostics()
            .Where(diagnostic => diagnostic.Severity == DiagnosticSeverity.Error)
            .OrderBy(diagnostic => diagnostic.Location.SourceSpan.Start)
            .ThenBy(diagnostic => diagnostic.Id, StringComparer.Ordinal)
            .ToArray();
        if (errors.Length == 0)
        {
            return;
        }

        var first = errors[0];
        var line = first.Location.GetLineSpan().StartLinePosition;
        throw new DiscoveryException(
            "PARSE_ERROR",
            $"{relativePath}:{line.Line + 1}:{line.Character + 1} {first.Id} {first.GetMessage()} " +
            $"({errors.Length} parse error(s)).");
    }

    private static void RejectDuplicateCandidateIds(IEnumerable<SyntaxCandidate> candidates)
    {
        var ids = new HashSet<string>(StringComparer.Ordinal);
        foreach (var candidate in candidates)
        {
            if (!ids.Add(candidate.CandidateId))
            {
                throw new DiscoveryException(
                    "CANDIDATE_ID_COLLISION",
                    $"Stable candidate id collision at '{candidate.SourcePath}:{candidate.StartLine}'.");
            }
        }
    }
}

internal static class CandidateClassifier
{
    public static string? Classify(SyntaxNode node) => node switch
    {
        DelegateDeclarationSyntax => "type-declaration.delegate",
        BaseTypeDeclarationSyntax => "type-declaration",
        MethodDeclarationSyntax => "callable.method",
        ConstructorDeclarationSyntax => "callable.constructor",
        DestructorDeclarationSyntax => "callable.destructor",
        OperatorDeclarationSyntax => "callable.operator",
        ConversionOperatorDeclarationSyntax => "callable.conversion",
        AccessorDeclarationSyntax => "callable.accessor",
        LocalFunctionStatementSyntax => "callable.local-function",
        AnonymousFunctionExpressionSyntax => "callable.lambda-or-anonymous-function",
        IfStatementSyntax => "control.if",
        ElseClauseSyntax => "control.else",
        SwitchSectionSyntax => "control.switch-section",
        SwitchExpressionArmSyntax => "control.switch-expression-arm",
        ConditionalExpressionSyntax => "control.conditional-expression",
        ForStatementSyntax => "control.loop.for",
        ForEachStatementSyntax => "control.loop.foreach",
        ForEachVariableStatementSyntax => "control.loop.foreach-variable",
        WhileStatementSyntax => "control.loop.while",
        DoStatementSyntax => "control.loop.do",
        CatchClauseSyntax => "control.catch",
        CatchFilterClauseSyntax => "control.catch-filter",
        FinallyClauseSyntax => "control.finally",
        InvocationExpressionSyntax => "operation.invocation",
        ObjectCreationExpressionSyntax => "operation.object-creation",
        ImplicitObjectCreationExpressionSyntax => "operation.implicit-object-creation",
        ArrayCreationExpressionSyntax => "operation.array-creation",
        ImplicitArrayCreationExpressionSyntax => "operation.implicit-array-creation",
        AnonymousObjectCreationExpressionSyntax => "operation.anonymous-object-creation",
        StackAllocArrayCreationExpressionSyntax => "operation.stackalloc-array-creation",
        AssignmentExpressionSyntax => "operation.assignment",
        ReturnStatementSyntax => "operation.return",
        ThrowStatementSyntax => "operation.throw-statement",
        ThrowExpressionSyntax => "operation.throw-expression",
        YieldStatementSyntax => "operation.yield",
        EqualsValueClauseSyntax equalsValue when IsFieldOrPropertyInitializer(equalsValue) =>
            "initializer.field-or-property",
        EqualsValueClauseSyntax equalsValue when equalsValue.Parent is EnumMemberDeclarationSyntax =>
            "initializer.enum-member",
        ArrowExpressionClauseSyntax arrow when IsPropertyLikeExpressionBody(arrow) =>
            "initializer.property-expression-body",
        AttributeSyntax => "attribute",
        DirectiveTriviaSyntax => "preprocessor-directive-boundary",
        _ => null,
    };

    private static bool IsFieldOrPropertyInitializer(EqualsValueClauseSyntax equalsValue) =>
        equalsValue.Parent is PropertyDeclarationSyntax or EventDeclarationSyntax ||
        equalsValue.Parent is VariableDeclaratorSyntax variable &&
        variable.Parent?.Parent is FieldDeclarationSyntax or EventFieldDeclarationSyntax;

    private static bool IsPropertyLikeExpressionBody(ArrowExpressionClauseSyntax arrow) =>
        arrow.Parent is PropertyDeclarationSyntax or IndexerDeclarationSyntax or EventDeclarationSyntax;
}

internal static class CandidateFactory
{
    public static SyntaxCandidate FromNode(
        string sourcePath,
        SyntaxTree syntaxTree,
        SourceText sourceText,
        SyntaxNode node,
        string category)
    {
        var span = node.Span;
        var symbol = SyntacticSymbol.Build(node);
        var contentDigest = Hashing.Sha256(sourceText.ToString(span));
        var kind = node.Kind().ToString();
        return Create(
            sourcePath,
            syntaxTree,
            span,
            category,
            kind,
            symbol,
            contentDigest,
            HintClassifier.Classify(node, category));
    }

    public static SyntaxCandidate FromTrivia(
        string sourcePath,
        SyntaxTree syntaxTree,
        SourceText sourceText,
        SyntaxTrivia trivia,
        string category)
    {
        var span = trivia.Span;
        var symbol = trivia.Token.Parent is null
            ? "<compilation-unit>"
            : SyntacticSymbol.Build(trivia.Token.Parent);
        var contentDigest = Hashing.Sha256(sourceText.ToString(span));
        return Create(
            sourcePath,
            syntaxTree,
            span,
            category,
            trivia.Kind().ToString(),
            symbol,
            contentDigest,
            [new CandidateHint { Tag = "preprocessor", Evidence = "disabled-text-boundary" }]);
    }

    public static string CanonicalLine(SyntaxCandidate candidate)
    {
        var hints = string.Join(
            "|",
            candidate.Hints.Select(hint => $"{hint.Tag}={hint.Evidence}"));
        return string.Join(
            '\0',
            candidate.CandidateId,
            candidate.SourcePath,
            candidate.Category,
            candidate.SyntaxKind,
            candidate.ParentSymbol,
            candidate.SpanStartUtf16.ToString(System.Globalization.CultureInfo.InvariantCulture),
            candidate.SpanLengthUtf16.ToString(System.Globalization.CultureInfo.InvariantCulture),
            candidate.StartLine.ToString(System.Globalization.CultureInfo.InvariantCulture),
            candidate.StartColumn.ToString(System.Globalization.CultureInfo.InvariantCulture),
            candidate.EndLine.ToString(System.Globalization.CultureInfo.InvariantCulture),
            candidate.EndColumn.ToString(System.Globalization.CultureInfo.InvariantCulture),
            candidate.ContentSha256,
            hints);
    }

    private static SyntaxCandidate Create(
        string sourcePath,
        SyntaxTree syntaxTree,
        TextSpan span,
        string category,
        string kind,
        string parentSymbol,
        string contentDigest,
        IReadOnlyList<CandidateHint> hints)
    {
        var lineSpan = syntaxTree.GetLineSpan(span);
        var idMaterial = string.Join(
            '\0',
            "crystal-semantic-candidate/v1",
            sourcePath,
            category,
            kind,
            span.Start.ToString(System.Globalization.CultureInfo.InvariantCulture),
            span.Length.ToString(System.Globalization.CultureInfo.InvariantCulture),
            contentDigest);
        var candidateId = $"CSC-{Hashing.Sha256(idMaterial)}";

        return new SyntaxCandidate
        {
            CandidateId = candidateId,
            SourcePath = sourcePath,
            Category = category,
            SyntaxKind = kind,
            ParentSymbol = parentSymbol,
            SpanStartUtf16 = span.Start,
            SpanLengthUtf16 = span.Length,
            StartLine = lineSpan.StartLinePosition.Line + 1,
            StartColumn = lineSpan.StartLinePosition.Character + 1,
            EndLine = lineSpan.EndLinePosition.Line + 1,
            EndColumn = lineSpan.EndLinePosition.Character + 1,
            ContentSha256 = contentDigest,
            Hints = hints,
        };
    }
}

internal sealed class CandidateComparer : IComparer<SyntaxCandidate>
{
    public static CandidateComparer Instance { get; } = new();

    public int Compare(SyntaxCandidate? left, SyntaxCandidate? right)
    {
        if (ReferenceEquals(left, right))
        {
            return 0;
        }

        if (left is null)
        {
            return -1;
        }

        if (right is null)
        {
            return 1;
        }

        var comparison = StringComparer.Ordinal.Compare(left.SourcePath, right.SourcePath);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.SpanStartUtf16.CompareTo(right.SpanStartUtf16);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = left.SpanLengthUtf16.CompareTo(right.SpanLengthUtf16);
        if (comparison != 0)
        {
            return comparison;
        }

        comparison = StringComparer.Ordinal.Compare(left.SyntaxKind, right.SyntaxKind);
        return comparison != 0
            ? comparison
            : StringComparer.Ordinal.Compare(left.CandidateId, right.CandidateId);
    }
}

internal static class SyntacticSymbol
{
    public static string Build(SyntaxNode node)
    {
        var segments = new List<string>();
        foreach (var ancestor in node.AncestorsAndSelf().Reverse())
        {
            var segment = Segment(ancestor);
            if (segment is not null)
            {
                segments.Add(segment);
            }
        }

        return segments.Count == 0 ? "<compilation-unit>" : string.Join('.', segments);
    }

    private static string? Segment(SyntaxNode node) => node switch
    {
        BaseNamespaceDeclarationSyntax declaration => declaration.Name.ToString(),
        TypeDeclarationSyntax declaration => WithArity(
            declaration.Identifier.ValueText,
            declaration.TypeParameterList?.Parameters.Count ?? 0),
        EnumDeclarationSyntax declaration => declaration.Identifier.ValueText,
        DelegateDeclarationSyntax declaration => WithArity(
            declaration.Identifier.ValueText,
            declaration.TypeParameterList?.Parameters.Count ?? 0),
        MethodDeclarationSyntax declaration =>
            $"{WithArity(declaration.Identifier.ValueText, declaration.TypeParameterList?.Parameters.Count ?? 0)}({declaration.ParameterList.Parameters.Count})",
        ConstructorDeclarationSyntax declaration => $".ctor({declaration.ParameterList.Parameters.Count})",
        DestructorDeclarationSyntax => ".dtor()",
        OperatorDeclarationSyntax declaration => $"operator {declaration.OperatorToken.ValueText}({declaration.ParameterList.Parameters.Count})",
        ConversionOperatorDeclarationSyntax declaration =>
            $"{declaration.ImplicitOrExplicitKeyword.ValueText} operator {declaration.Type}({declaration.ParameterList.Parameters.Count})",
        PropertyDeclarationSyntax declaration => declaration.Identifier.ValueText,
        IndexerDeclarationSyntax declaration => $"this[{declaration.ParameterList.Parameters.Count}]",
        EventDeclarationSyntax declaration => declaration.Identifier.ValueText,
        VariableDeclaratorSyntax declaration when declaration.Parent?.Parent is FieldDeclarationSyntax or EventFieldDeclarationSyntax =>
            declaration.Identifier.ValueText,
        AccessorDeclarationSyntax declaration => declaration.Keyword.ValueText,
        LocalFunctionStatementSyntax declaration =>
            $"local {WithArity(declaration.Identifier.ValueText, declaration.TypeParameterList?.Parameters.Count ?? 0)}({declaration.ParameterList.Parameters.Count})",
        AnonymousFunctionExpressionSyntax declaration => $"lambda@{declaration.SpanStart}",
        _ => null,
    };

    private static string WithArity(string name, int arity) => arity == 0 ? name : $"{name}`{arity}";
}

internal static class HintClassifier
{
    private sealed record Rule(string Tag, string[] Terms);

    private static readonly Rule[] Rules =
    [
        new("packet-handler", ["packet", "handlepacket", "processpacket", "receivepacket", "messagehandler"]),
        new("timer-update-tick", ["tick", "timer", "update", "processdelay", "elapsed"]),
        new("save-load-persistence", ["save", "load", "persist", "serialize", "deserialize", "database", "repository", "checkpoint"]),
        new("rng", ["random", "rng", "chance", "roll", "dice"]),
        new("network-send-broadcast", ["send", "broadcast", "socket", "network", "packet"]),
        new("ui-event", ["click", "mousedown", "mouseup", "keydown", "keyup", "input", "uievent", "eventhandler"]),
        new("render", ["render", "draw", "paint", "sprite"]),
        new("audio", ["audio", "sound", "music", "playsound"]),
    ];

    public static IReadOnlyList<CandidateHint> Classify(SyntaxNode node, string category)
    {
        var identifiers = node.DescendantTokens(descendIntoTrivia: false)
            .Where(token => token.IsKind(SyntaxKind.IdentifierToken))
            .Select(token => token.ValueText)
            .Where(value => value.Length > 0)
            .Distinct(StringComparer.Ordinal)
            .OrderBy(value => value, StringComparer.Ordinal)
            .ToArray();
        var hints = new List<CandidateHint>();

        foreach (var rule in Rules)
        {
            var evidence = identifiers.FirstOrDefault(identifier =>
                rule.Terms.Any(term => identifier.Contains(term, StringComparison.OrdinalIgnoreCase)));
            if (evidence is not null)
            {
                hints.Add(new CandidateHint
                {
                    Tag = rule.Tag,
                    Evidence = $"identifier:{evidence}",
                });
            }
        }

        if (category == "attribute")
        {
            var handlerEvidence = identifiers.FirstOrDefault(identifier =>
                identifier.Contains("handler", StringComparison.OrdinalIgnoreCase) ||
                identifier.Contains("packet", StringComparison.OrdinalIgnoreCase) ||
                identifier.Contains("message", StringComparison.OrdinalIgnoreCase) ||
                identifier.Contains("event", StringComparison.OrdinalIgnoreCase));
            if (handlerEvidence is not null)
            {
                hints.Add(new CandidateHint
                {
                    Tag = "attribute-declared-handler",
                    Evidence = $"attribute-identifier:{handlerEvidence}",
                });
            }
        }

        if (category.StartsWith("preprocessor-", StringComparison.Ordinal))
        {
            hints.Add(new CandidateHint
            {
                Tag = "preprocessor",
                Evidence = $"syntax-category:{category}",
            });
        }

        return hints
            .DistinctBy(hint => (hint.Tag, hint.Evidence))
            .OrderBy(hint => hint.Tag, StringComparer.Ordinal)
            .ThenBy(hint => hint.Evidence, StringComparer.Ordinal)
            .ToArray();
    }
}
