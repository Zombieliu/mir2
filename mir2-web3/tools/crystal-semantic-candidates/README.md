# Crystal semantic syntax candidate discovery

This isolated tool creates a deterministic, fail-closed **syntax candidate**
inventory from the fixed sibling source roots:

```text
Crystal/Client
Crystal/Server
Crystal/Shared
```

It deliberately does not accept a source-root argument. A successful report
proves that a stable list of C# files was hashed and that the configured Roslyn
syntax categories were over-approximately enumerated. It does **not** prove
which candidates are player-observable semantic leaves.

## Offline build and test

The project has no `PackageReference` and does not download Roslyn from NuGet.
It references `Microsoft.CodeAnalysis.dll` and
`Microsoft.CodeAnalysis.CSharp.dll` from the active .NET SDK's
`Roslyn/bincore` directory.

Run low-load, serial validation from `mir2-web3`:

```powershell
dotnet restore tools/crystal-semantic-candidates/tests/CrystalSemanticCandidates.Tests.csproj --ignore-failed-sources --disable-parallel
dotnet build tools/crystal-semantic-candidates/tests/CrystalSemanticCandidates.Tests.csproj --no-restore --configuration Debug --maxcpucount:1
dotnet run --project tools/crystal-semantic-candidates/tests/CrystalSemanticCandidates.Tests.csproj --no-build --configuration Debug
```

The checked configuration targets `net9.0`, matching the SDK 9 Roslyn
assemblies used to build it. If another machine keeps SDK Roslyn assemblies in
a nonstandard location, pass the location explicitly at build time:

```powershell
dotnet build tools/crystal-semantic-candidates/CrystalSemanticCandidates.csproj --no-restore --maxcpucount:1 -p:RoslynBinariesPath=C:\path\to\sdk\Roslyn\bincore
```

Missing or runtime-incompatible SDK Roslyn assemblies fail the build; the tool
does not fall back to a downloaded package or a text/regex parser.

## Generate

Write deterministic JSON to stdout:

```powershell
dotnet run --project tools/crystal-semantic-candidates/CrystalSemanticCandidates.csproj --no-build --configuration Debug
```

Or atomically replace an explicit output file:

```powershell
dotnet run --project tools/crystal-semantic-candidates/CrystalSemanticCandidates.csproj --no-build --configuration Debug -- --output C:\temp\crystal-semantic-candidates.json
```

Unknown arguments and all source-root overrides are rejected.

## Candidate classes

The Roslyn walk emits candidates for:

- type and delegate declarations;
- methods, constructors, destructors, operators, conversions, accessors, local
  functions, lambdas, and anonymous functions;
- `if`/`else`, switch sections and expression arms, conditional expressions,
  loops, catches, catch filters, and `finally` clauses;
- invocations, explicit/implicit/anonymous object creation, explicit/implicit/
  stackalloc array creation, assignments, returns, throws, and yields;
- field/property/event and enum-member initializers plus property-like
  expression bodies;
- attributes, preprocessor directives, and disabled-text boundaries.

Each candidate carries a stable ID, normalized source path, exact UTF-16 span,
one-based line/column range, syntax kind, syntactic parent symbol, exact-content
SHA-256, and optional explainable hints. Packet handlers, timers/ticks,
persistence, RNG, networking, UI events, rendering, and audio are hints only.
Every candidate remains `UNDISPOSITIONED_CANDIDATE` with `semanticLeaf=null`.
The report also binds the loaded Roslyn assembly version and parse options;
changing parser versions therefore changes the report aggregate instead of
silently reusing evidence from a different syntax engine.

## Fail-closed input rules

The scanner:

- requires all three fixed source roots and at least one `.cs` file;
- explicitly excludes the SDK compiler-output directory names `bin` and `obj`
  (the report records this scope); generated AssemblyInfo/global-using files are
  not Crystal authored compile inputs;
- manually traverses directories and rejects symlinks, junctions, and other
  reparse points observed in existing input path components or entries; this is
  fail-closed checking but is not an atomic strong no-follow handle guarantee;
- rejects control characters, alternate-data-stream separators, trailing dots
  or spaces, DOS device names, non-NFC paths, duplicate paths, and
  case-insensitive path aliases;
- hashes exact source bytes, parses the captured bytes with Roslyn, rejects all
  parse errors, then rescans and rehashes the source tree before reporting;
- emits no timestamps or absolute source paths, so unchanged inputs serialize
  byte-for-byte identically across repeated runs.

## Completion boundary

Successful output intentionally states:

```text
sourceSnapshotCaptured=true
gitCleanBound=false
strongNoFollowBound=false
sourceFileInventoryComplete=false
semanticCandidateDiscoveryComplete=true
semanticDispositionComplete=false
semanticLeafInventoryComplete=false
inventoryComplete=false
```

`semanticCandidateDiscoveryComplete=true` applies only to the exact captured
authored-C# snapshot named by `sourceSnapshotSha256` and
`sourceSnapshotFileCount`, after the disclosed `bin`/`obj` exclusions. It is not
a clean-revision or strong-no-follow source inventory claim.

### Compatibility with the existing Node inventory

The current repository Node inventory reports 403 C# files because it also
counts nine SDK-generated files under `Client/obj/Debug`, `Server/obj/Debug`,
and `Shared/obj/Debug` (AssemblyAttributes, AssemblyInfo, and GlobalUsings). This
Roslyn candidate tool intentionally captures 394 authored compile inputs after
excluding `bin` and `obj`. The 403-file and 394-file aggregates describe
different scopes and must not be compared, substituted, or treated as the same
semantic denominator. Migration to one reviewed denominator remains unfinished.

A separate trusted review must first unify that denominator, then disposition
every candidate as one or more
semantic leaves, supporting code linked to those leaves, or a signed
non-observable reason. That review must also split materially different success,
rejection, timeout, duplicate, persistence, packet-order, and client-consequence
behaviors.

This syntax layer still cannot prove:

- player observability or semantic-leaf identity;
- runtime behavior reached through reflection, generated code, data files,
  scripts, native libraries, or dynamic dispatch;
- semantics inside disabled preprocessor text (it records exact boundaries but
  does not pretend an inactive branch was parsed as active code);
- compilation/type binding, call graph identity, packet order, timing, RNG state,
  persistence after reload, or original-client consequences;
- immunity to every possible filesystem race or hard-link substitution beyond
  the explicit reparse checks plus before/after byte, size, timestamp, and file
  set binding.

Those limits are why neither this tool nor its tests can set
`sourceFileInventoryComplete`, `semanticLeafInventoryComplete`, or
`inventoryComplete` to true.
