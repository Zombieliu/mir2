using System.Text.Json;
using System.Text.Json.Nodes;

namespace ComparableCrystal;

internal static class CombatPolicy
{
    public const string FileSha256 = "8b373b5d03a71c468934d3591c6d06ad65ce6fe2eeb3208acca39ee0ae25a25b";
    public const string CanonicalSha256 = "8cd44de50283077d0cbc9991abb9af531894000d0e624146c64eb2b34e6dfd32";
    public static void RequireEmbedded(Plan plan)
    {
        Guard.Need(plan.CombatContract is { ValueKind: JsonValueKind.Object }, "common_combat_contract_required_no_legacy_melee_v2");
        Guard.Need(plan.CombatContractSha256 == FileSha256 && plan.CombatContractCanonicalSha256 == CanonicalSha256,
            "common_combat_contract_hash_pin_required");
    }
    public static string? Verify(Plan plan, Options options)
    {
        if (plan.Scenario != "dispersed-melee") return null;
        RequireEmbedded(plan);
        var path = Path.GetFullPath(options.Required("combat-contract"));
        Guard.Need(new FileInfo(path).Length <= 16384 && Files.Hash(path) == FileSha256, "actual_common_combat_contract_file_mismatch");
        var actual = Files.Read(path, 16384);
        Guard.Need(JsonNode.DeepEquals(JsonNode.Parse(plan.CombatContract!.Value.GetRawText()), JsonNode.Parse(actual.GetRawText())),
            "embedded_common_combat_contract_differs_from_pinned_actual_file");
        return FileSha256;
    }
}
