using System.Text.Json;
using System.Text.RegularExpressions;

namespace ComparableCrystal;

internal static class Program
{
    public static async Task<int> Main(string[] args)
    {
        try
        {
            var options = new Options(args); Files.VerifyRuntime(); Files.VerifySourceLock(options.Required("source-lock"));
            return options.Mode switch
            {
                "navigation" => WorldTools.Navigation(options),
                "prepare" => WorldTools.Prepare(options),
                "inspect-saves" => WorldTools.Inspect(options),
                "validate-plan" => ValidatePlan(options),
                "self-test" => OfflineTests.Run(),
                "schema-examples" => OfflineTests.Examples(options),
                "run" => await Runner.Run(options),
                _ => throw new InvalidDataException("mode_must_be_navigation_prepare_inspect_saves_run_or_self_test")
            };
        }
        catch (Exception error)
        {
            var code = Regex.IsMatch(error.Message, "^[A-Za-z0-9_.-]{1,160}$") ? error.Message : "operation_failed";
            Console.Error.WriteLine(JsonSerializer.Serialize(new { schema = "mir2.comparable.crystal-error.v1", errorType = error.GetType().Name, code }, Files.Compact)); return 1;
        }
    }
    private static int ValidatePlan(Options options)
    {
        var path = Path.GetFullPath(options.Required("plan")); var plan = Files.Read<Plan>(path); plan.Validate(false); CombatPolicy.Verify(plan, options);
        WorldTools.ValidateNavigation(plan, options.Required("navigation"));
        Console.WriteLine(JsonSerializer.Serialize(new { schema = "mir2.comparable.crystal-plan-validation.v1", planSha256 = Files.Hash(path), navigationSha256 = plan.NavigationSha256,
            plan.Population, plan.DurationMs, actions = plan.Actions.Length, passed = true, gameInstancesStarted = 0 }, Files.Compact)); return 0;
    }
}
