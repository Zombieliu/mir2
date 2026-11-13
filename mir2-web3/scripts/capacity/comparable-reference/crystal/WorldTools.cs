using System.Drawing;
using System.Net;
using System.Net.NetworkInformation;
using System.Security.AccessControl;
using System.Security.Principal;
using System.Security.Cryptography;
using System.Text.Json;
using Server;
using Server.MirDatabase;
using Server.MirEnvir;

namespace ComparableCrystal;

internal sealed record PrivateActor(int Ordinal, string AccountId, string Password, string Name, int CharacterIndex,
    int MapIndex, Cell Position, int GroupIndex, string TargetName);
internal sealed record Endpoint(string Host, int Port);
internal sealed record PrivateFixture(string Schema, Endpoint Endpoint, string VersionFile, string VersionFileSha256,
    string PlanSha256, string NavigationSha256, PrivateActor[] Actors);

internal static class WorldTools
{
    private const string Marker = ".crystal-reference-isolated.json";
    private static string Random(int bytes) => Convert.ToHexString(RandomNumberGenerator.GetBytes(bytes)).ToLowerInvariant();
    private static void SecureDirectory(string root)
    {
        if (!OperatingSystem.IsWindows()) { File.SetUnixFileMode(root, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute); return; }
        var security = new DirectorySecurity(); security.SetAccessRuleProtection(true, false);
        var inherit = InheritanceFlags.ContainerInherit | InheritanceFlags.ObjectInherit;
        security.AddAccessRule(new FileSystemAccessRule(WindowsIdentity.GetCurrent().User!, FileSystemRights.FullControl, inherit, PropagationFlags.None, AccessControlType.Allow));
        security.AddAccessRule(new FileSystemAccessRule(new SecurityIdentifier(WellKnownSidType.LocalSystemSid, null), FileSystemRights.FullControl, inherit, PropagationFlags.None, AccessControlType.Allow));
        new DirectoryInfo(root).SetAccessControl(security);
    }
    private static void CopyAssets(string source, string root, bool allMaps)
    {
        root = Files.StatePath(root); source = Path.GetFullPath(source);
        Guard.Need(!Directory.Exists(root) && !File.Exists(root) && !root.StartsWith(source + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase), "fresh_isolated_world_required");
        Guard.Need(Files.Hash(Path.Combine(source, "Server.MirDB")) == Files.DatabaseSha && Files.Hash(Path.Combine(source, "Maps", "DogYoHyun.map")) == Files.MapSha, "original_data_pin_mismatch");
        Directory.CreateDirectory(root); SecureDirectory(root);
        Files.Write(Path.Combine(root, Marker), new { scope = "isolated-crystal-reference-capacity", comparableFixture = true, originalAccountsCopied = false, sourceDataRoot = source });
        var files = new List<object>();
        foreach (var name in new[] { "Server.MirDB", "Maps", "Envir", "Configs", "Localization", "log4net.config" })
        {
            var input = Path.Combine(source, name);
            if (Directory.Exists(input))
            {
                foreach (var item in Directory.EnumerateFileSystemEntries(input, "*", SearchOption.AllDirectories))
                    Guard.Need((File.GetAttributes(item) & FileAttributes.ReparsePoint) == 0, "original_asset_reparse_forbidden");
                foreach (var file in Directory.EnumerateFiles(input, "*", SearchOption.AllDirectories))
                    if (name != "Maps" || allMaps || Path.GetFileName(file) == "DogYoHyun.map") Copy(file);
            }
            else { Guard.Need(File.Exists(input), "required_original_asset_missing"); Copy(input); }
        }
        Files.Write(Path.Combine(root, "original-assets-manifest.json"), new { schema = "mir2.comparable.original-assets.v1", originalAccountsCopied = false, fullMapAssetSet = allMaps, files });
        void Copy(string file)
        {
            Guard.Need((File.GetAttributes(file) & FileAttributes.ReparsePoint) == 0, "original_asset_reparse_forbidden");
            var relative = Path.GetRelativePath(source, file); var destination = Path.Combine(root, relative);
            Directory.CreateDirectory(Path.GetDirectoryName(destination)!); var before = Files.Hash(file); File.Copy(file, destination, false);
            File.SetAttributes(destination, File.GetAttributes(destination) & ~FileAttributes.ReadOnly);
            Guard.Need(Files.Hash(destination) == before, "isolated_asset_copy_changed"); files.Add(new { path = relative.Replace('\\', '/'), bytes = new FileInfo(destination).Length, sha256 = before });
        }
    }
    private static (Envir Environment, Map Map) Load(string root, bool accounts = false)
    {
        root = Files.StatePath(root);
        Guard.Need(File.Exists(Path.Combine(root, Marker)), "explicit_isolated_marker_required");
        Guard.Need(Files.Read(Path.Combine(root, Marker)).GetProperty("scope").GetString() == "isolated-crystal-reference-capacity", "isolated_marker_scope_mismatch");
        Directory.SetCurrentDirectory(root); Packet.IsServer = true; Settings.Load();
        Guard.Need(Envir.Edit.LoadDB() && Envir.Main.LoadDB(), "native_gui_database_bootstrap_failed");
        var env = Envir.Main;
        Guard.Need(Envir.Edit.GameShopList.Count == env.GameShopList.Count && env.GameShopList.Count == 105, "native_gui_shop_bootstrap_mismatch");
        var info = env.MapInfoList.Single(x => x.Index == 440);
        Guard.Need(info.FileName == "DogYoHyun" && !info.NoFight && !info.NoDrug && !info.Fire && !info.Lightning && !info.NoReconnect && !info.RequiredGroup && info.Respawns.Count == 0 && info.SafeZones.Count == 0 && !env.NPCInfoList.Any(x => x.MapIndex == info.Index), "native_map_rules_changed");
        info.CreateMap(); var map = env.MapList.Single(x => x.Info.Index == info.Index);
        Guard.Need(map.Width == 800 && map.Height == 800 && Files.Hash(Path.Combine(Settings.MapPath, "DogYoHyun.map")) == Files.MapSha, "native_map_bytes_or_geometry_changed");
        if (accounts) env.LoadAccounts();
        return (env, map);
    }
    public static bool Eligible(Map map, Point point)
    {
        if (!map.ValidPoint(point) || map.DoorIndex[point.X, point.Y] is not null) return false;
        return !map.Info.Movements.Any(x => Distance(point, x.Source) <= 3)
            && !map.Info.NPCs.Any(x => Distance(point, x.Location) <= 3)
            && !map.Info.SafeZones.Any(x => Distance(point, x.Location) <= x.Size + 2);
    }
    private static int Distance(Point a, Point b) => Math.Max(Math.Abs(a.X - b.X), Math.Abs(a.Y - b.Y));
    public static int Navigation(Options options)
    {
        var root = options.Required("data-root"); CopyAssets(options.Required("source-data"), root, false);
        var (_, map) = Load(root); var bits = new byte[(map.Width * map.Height + 7) / 8]; var count = 0;
        for (var y = 0; y < map.Height; y++) for (var x = 0; x < map.Width; x++)
            if (Eligible(map, new Point(x, y))) { var index = y * map.Width + x; bits[index / 8] |= (byte)(1 << (index % 8)); count++; }
        var output = options.Required("output");
        Files.Write(output, new { schema = "mir2.comparable.navigation.v1", map = new { index = 440, fileName = "DogYoHyun", width = map.Width, height = map.Height, rawSha256 = Files.MapSha },
            source = "original-library-cell-valid-and-movement-exclusion", gameCodeUnmodified = true, encoding = "row-major-lsb0-bitset", eligibleBitsBase64 = Convert.ToBase64String(bits), eligibleCount = count,
            exclusions = new { movementRadius = 3, npcRadius = 3, allDoorCells = true, safeZones = true } });
        Console.WriteLine(JsonSerializer.Serialize(new { navigation = output, sha256 = Files.Hash(output), eligibleCount = count, gameStarted = false })); return 0;
    }
    internal static void ValidateNavigation(Plan plan, string navigation)
    {
        Guard.Need(Files.Hash(navigation) == plan.NavigationSha256, "actual_navigation_file_sha_mismatch");
        var nav = Files.Read(navigation);
        Guard.Need(nav.GetProperty("schema").GetString() == "mir2.comparable.navigation.v1" && nav.GetProperty("encoding").GetString() == "row-major-lsb0-bitset" && nav.GetProperty("gameCodeUnmodified").GetBoolean(), "navigation_schema_mismatch");
        var m = nav.GetProperty("map"); Guard.Need(m.GetProperty("index").GetInt32() == 440 && m.GetProperty("rawSha256").GetString() == Files.MapSha && m.GetProperty("width").GetInt32() == 800 && m.GetProperty("height").GetInt32() == 800, "navigation_map_pin_mismatch");
        var bits = Convert.FromBase64String(nav.GetProperty("eligibleBitsBase64").GetString()!); Guard.Need(bits.Length == 80000, "navigation_bitset_size_mismatch");
        foreach (var cell in plan.Actors.SelectMany(x => x.Lane).Concat(plan.Actions.Where(x => x.Target is not null).Select(x => x.Target!)))
        { var index = cell.Y * 800 + cell.X; Guard.Need(cell.X >= 0 && cell.X < 800 && cell.Y >= 0 && cell.Y < 800 && (bits[index / 8] & (1 << (index % 8))) != 0, "plan_cell_not_original_eligible"); }
    }
    public static int Prepare(Options options)
    {
        var planFile = Path.GetFullPath(options.Required("plan")); var plan = Files.Read<Plan>(planFile); plan.Validate(false); CombatPolicy.Verify(plan, options);
        var navigation = Path.GetFullPath(options.Required("navigation")); ValidateNavigation(plan, navigation);
        var versionFile = Path.GetFullPath(options.Required("version-file")); Guard.Need(File.Exists(versionFile), "actual_driver_version_file_required");
        var root = options.Required("data-root"); CopyAssets(options.Required("source-data"), root, true);
        var (env, map) = Load(root); Guard.Need(!File.Exists(Envir.AccountPath), "original_accounts_must_not_exist");
        var bind = IPAddress.Parse(options.Value("bind", "127.77.0.1")); Guard.Need(IPAddress.IsLoopback(bind), "loopback_only");
        var port = options.Integer("port", 17655); Guard.Need(port is >= 1024 and <= 65535, "port_invalid");
        Settings.IPAddress = bind.ToString(); Settings.Port = (ushort)port; Settings.MaxUser = checked((ushort)(plan.Population + 16)); Settings.MaxIP = checked((ushort)(plan.Population + 16));
        Settings.IPBlockSeconds = 0; Settings.TestServer = false; Settings.CheckVersion = true; Settings.VersionPath = versionFile; Settings.StartHTTPService = false; Settings.EnforceDBChecks = true; Settings.GMPassword = Random(10); Settings.Save();
        var baseStats = Settings.ClassBaseStats[(int)MirClass.Warrior].Stats;
        var hp = baseStats.Single(x => x.Type == Stat.HP).Calculate(MirClass.Warrior, 40); var mp = baseStats.Single(x => x.Type == Stat.MP).Calculate(MirClass.Warrior, 40);
        var bag = baseStats.Single(x => x.Type == Stat.BagWeight).Calculate(MirClass.Warrior, 40);
        var itemSpecs = new[] { (221, EquipmentSlot.Weapon), (317, EquipmentSlot.Armour), (482, EquipmentSlot.BraceletL), (482, EquipmentSlot.BraceletR) };
        var seedInfo = itemSpecs.Select(x => Functions.GetRealItem(env.GetItemInfo(x.Item1), 40, MirClass.Warrior, env.ItemInfoList)).ToArray();
        var baseAttackSpeedStat = baseStats.Where(x => x.Type == Stat.AttackSpeed).Sum(x => x.Calculate(MirClass.Warrior, 40));
        if (plan.Scenario == "dispersed-melee") Guard.Need(seedInfo.All(x => x.Unique == SpecialItemMode.None && x.Set == ItemSet.None &&
            x.Stats[Stat.HP] == 0 && x.Stats[Stat.HPRatePercent] == 0 && x.Stats[Stat.AttackSpeed] >= 0 &&
            x.Stats[Stat.AttackSpeedRatePercent] >= 0 && x.Stats[Stat.Reflect] == 0 && x.Stats[Stat.HPDrainRatePercent] == 0 && x.Stats[Stat.EnergyShieldPercent] == 0) && baseAttackSpeedStat >= 0,
            "ordinary_melee_seed_has_unreviewed_health_or_special_effect");
        var potion = env.GetItemInfo(662); Guard.Need(potion is not null && potion.Name == "(HP)DrugLarge" && potion.Stats[Stat.HP] == 110 && potion.StackSize == 20, "original_potion_template_mismatch");
        var potionCount = Math.Min(180, bag / potion!.Weight); Guard.Need(potionCount >= 20, "fixture_supply_exceeds_weight");
        var batch = Random(3); var names = plan.Actors.Select(x => "Cm" + batch + x.Ordinal.ToString("D4")).ToArray(); var actors = new List<PrivateActor>();
        foreach (var actor in plan.Actors.OrderBy(x => x.Ordinal))
        {
            Guard.Need(actor.Lane.All(x => Eligible(map, x.Point)), "native_fixture_lane_invalid");
            var id = "cm" + batch + actor.Ordinal.ToString("D4"); var password = Random(6); Guard.Need(Envir.IsPasswordValid(password), "native_password_validation_failed");
            var account = new AccountInfo(new ClientPackets.NewAccount { AccountID = id, Password = password, UserName = "capacity fixture", BirthDate = new DateTime(1990, 1, 1), SecretQuestion = "isolated fixture", SecretAnswer = Random(6), EMailAddress = id + "@example.invalid" })
                { Index = ++env.NextAccountID, CreationIP = bind.ToString(), CreationDate = DateTime.Now, LastDate = DateTime.Now, AdminAccount = false };
            var character = new CharacterInfo { Index = ++env.NextCharacterID, AccountInfo = account, Name = names[actor.Ordinal], Class = MirClass.Warrior, Gender = MirGender.Male, Level = 40, Hair = 0,
                CreationIP = bind.ToString(), CreationDate = DateTime.Now, LastLoginDate = DateTime.Now, CurrentMapIndex = 440, CurrentLocation = actor.Start.Point, BindMapIndex = 440, BindLocation = actor.Start.Point,
                Direction = MirDirection.Down, HP = hp, MP = mp, Experience = 0, AMode = AttackMode.Peace, PMode = PetMode.Both, Heroes = new HeroInfo[1], MaximumHeroCount = 1, CurrentHeroIndex = 0 };
            foreach (var spec in itemSpecs) { var info = env.GetItemInfo(spec.Item1); Guard.Need(info is not null && info.RequiredAmount <= 40, "ordinary_item_template_missing"); character.Equipment[(int)spec.Item2] = env.CreateFreshItem(info); }
            if (plan.Scenario == "dispersed-melee") Guard.Need(character.Magics.Count == 0 && character.Buffs.Count == 0 && character.Heroes.All(x => x is null),
                "native_melee_fixture_skills_buffs_heroes_must_be_empty");
            for (int left = potionCount, slot = 6; left > 0; slot++) { var item = env.CreateFreshItem(potion); item.Count = checked((ushort)Math.Min(left, potion.StackSize)); character.Inventory[slot] = item; left -= item.Count; }
            Guard.Need(character.Inventory.Where(x => x is not null).Sum(x => x.Weight) <= bag, "native_fixture_bag_overweight");
            account.Characters.Add(character); env.AccountList.Add(account); env.CharacterList.Add(character);
            actors.Add(new PrivateActor(actor.Ordinal, id, password, character.Name, character.Index, 440, actor.Start, actor.Group, names[actor.PeerOrdinals[0]]));
        }
        env.SaveAccounts(); Guard.Need(File.Exists(Envir.AccountPath), "native_SaveAccounts_missing");
        var metadata = new { mapIndex = 440, fileName = "DogYoHyun", noFight = false, noDrug = false, respawnDefinitions = 0, npcDefinitions = 0, safeZones = 0, movementDefinitions = map.Info.Movements.Count, originalDatabaseSha256 = Files.DatabaseSha, mapFileSha256 = Files.MapSha, naturalNoRespawnNoNpc = true };
        Files.Write(Path.Combine(root, "actors.private.json"), new { schema = "mir2.crystal-reference-actors.v1", endpoint = new Endpoint(bind.ToString(), port), versionFile, versionFileSha256 = Files.Hash(versionFile),
            planSha256 = Files.Hash(planFile), navigationSha256 = Files.Hash(navigation), count = actors.Count, mapIndex = 440, referencePlayerMap = metadata, actors });
        Files.Write(Path.Combine(root, "comparable-fixture.json"), new { schema = "mir2.comparable.crystal-fixture.v1", planSha256 = Files.Hash(planFile), navigationSha256 = Files.Hash(navigation), originalAccountsCopied = false,
            gameCodeUnmodified = true, originalDatabaseSha256 = Files.Hash(Envir.DatabasePath), originalAccountFileSha256 = Files.Hash(Envir.AccountPath), referencePlayerMap = metadata, actorCount = actors.Count,
            level = 40, maximumHp = hp, maximumMp = mp, potionCount, equipmentIndices = itemSpecs.Select(x => x.Item1), originalGuiBootstrap = new { editLoadedBeforeMainStart = true, editGameShopCount = Envir.Edit.GameShopList.Count, mainGameShopCount = env.GameShopList.Count },
            combatContractSha256 = plan.CombatContractSha256, combatSeedProof = plan.Scenario != "dispersed-melee" ? null : (object)new {
                maximumHpSource = "native class level40 base HP; all selected real equipment has zero HP/HPpercent, no item sets/special modes/reflect/drain/energy shield",
                maximumHp = hp, minimumAttackSpeedStat = baseAttackSpeedStat + seedInfo.Sum(x => x.Stats[Stat.AttackSpeed]), emptyCharacterMagicsBuffsHeroes = true,
                items = seedInfo.Select(x => new { itemIndex = x.Index, unique = (int)x.Unique, set = (int)x.Set, hp = x.Stats[Stat.HP], hpRatePercent = x.Stats[Stat.HPRatePercent], attackSpeed = x.Stats[Stat.AttackSpeed] }) },
            publicActors = actors.Select(a => new { actorOrdinal = a.Ordinal, characterIndex = a.CharacterIndex, nameHash = Files.HashText(a.Name), mapIndex = a.MapIndex, position = a.Position, group = a.GroupIndex }),
            runtimeOnlyRequiredArgument = "--isolated-ip-block-seconds 0", frozenServerSha256 = Files.ServerSha, frozenSharedSha256 = Files.SharedSha });
        Console.WriteLine(JsonSerializer.Serialize(new { preparedWorld = root, actorCount = actors.Count, planSha256 = Files.Hash(planFile), gameStarted = false })); return 0;
    }
    public static int Inspect(Options options)
    {
        var root = Files.StatePath(options.Required("data-root")); var fixture = Files.Read<PrivateFixture>(Path.Combine(root, "actors.private.json"));
        Guard.Need(!IPGlobalProperties.GetIPGlobalProperties().GetActiveTcpListeners().Any(x => x.Port == fixture.Endpoint.Port), "inspect_only_after_host_stopped");
        var (env, _) = Load(root, true); var requested = options.Value("actor-indices", "all"); var ordinals = requested == "all" ? fixture.Actors.Select(x => x.Ordinal).ToArray() : requested.Split(',').Select(int.Parse).ToArray();
        var samples = ordinals.Select(index =>
        {
            var a = fixture.Actors.Single(x => x.Ordinal == index); var account = env.AccountList.Single(x => x.AccountID == a.AccountId); var c = account.Characters.Single(x => x.Index == a.CharacterIndex);
            return new { actorOrdinal = index, characterIndex = c.Index, nameHash = Files.HashText(c.Name), identityMatchesFixture = c.Name == a.Name, mapIndex = c.CurrentMapIndex, position = Cell.Of(c.CurrentLocation), hp = c.HP, mp = c.MP, level = c.Level, adminAccount = account.AdminAccount,
                potionStock = c.Inventory.Where(x => x?.ItemIndex == 662).Sum(x => x.Count), itemState = ItemState.Describe(c.Inventory, c.Equipment, c.QuestInventory, account.Gold, account.Credit) };
        }).ToArray();
        Files.Write(options.Required("output"), new { schema = "mir2.comparable.crystal-disk-readback.v1", originalLoaderUsed = true, accountFileSha256 = Files.Hash(Envir.AccountPath), sampleCount = samples.Length, samples });
        Console.WriteLine(JsonSerializer.Serialize(new { output = options.Required("output"), sampleCount = samples.Length, originalLoaderUsed = true })); return 0;
    }
}
