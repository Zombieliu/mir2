import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import path from "node:path";
import { gunzipSync } from "node:zlib";

export const R16_PLAYABLE_KIND = "mir2-r16-playable-assets";
export const R16_MONSTER_LIBRARIES = Object.freeze([0, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 139]
  .map(index => `Monster/${String(index).padStart(3, "0")}`));
const UI_RANGES = {
  ChrSel: [[0, 18], [20, 35], [40, 55], [60, 75], [80, 95], [100, 115], [140, 155],
    [300, 315], [320, 335], [340, 355], [360, 375], [600, 615], [880, 895]],
  Prguse: [[44, 44], [65, 65], [114, 120], [238, 238], [360, 360], [389, 390], [660, 660], [940, 940], [1080, 1084]],
  Prguse2: [[351, 351], [360, 362]],
  Title: [[5, 5], [30, 32], [40, 40], [107, 112], [130, 138], [200, 208], [210, 212],
    [290, 295], [320, 334], [340, 354], [360, 362], [520, 522], [660, 669]],
};
export const R16_UI_ASSET_PATHS = Object.freeze(Object.entries(UI_RANGES).flatMap(([library, ranges]) =>
  ranges.flatMap(([first, last]) => Array.from({ length: last - first + 1 }, (_, i) => `original-ui/${library}/${first + i}.png`))).sort());
// Complete physical common-window libraries, including every skin state and
// metadata frame. These are source catalogs, not a list learned from one QA run.
// DNItems is a world sprite library; the common inventory/drop renderer uses Items.
const COMMON_UI_RANGES = {
  Prguse: "1-2,4,6-8,12-13,20,24,44,50,63,65,73,76,100-104,114-120,155-157,160,164-167,170,180,237-238,340-341,360-363,389-390,396-399,430-439,441-449,461-469,481-489,501-509,520-525,530-533,540-541,544-545,550-552,554-574,583,586,600-618,660,710,826-828,917-920,940,950-953,956-966,979,983-989,995,997,1000,1080-1088,1170-1185,1190-1205,1210-1225,1230-1245,1250-1265,1270-1285,1290-1305,1310-1325,1330-1347,1349-1365,1370-1385,1390-1405,1410-1425,1428-1445,1450-1465,1470-1485,1490-1505,1510-1525,1530-1545,1550-1565,1656-1658,1850-1853,1900-1914,1921,1923-1928,1932-1940,1943-1946,1960-1962,1970-1996,2000-2002,2004-2006,2012,2015-2029,2034,2036-2065,2086-2087,2090-2105,2110-2113,2122-2127,2131-2137,2140-2162,2190,2193,2221,2224,2227,2247,2420-2434,2443",
  Prguse2: "0-1,7-9,20-33,190-192,197-199,201-209,240-245,257-259,351,360-362,366-368,420,423-424,427-428,431-433,450-468,470-475,500-514,530-533,535,539-545,550-554,570-573,580-589,600-605,610-619,630-640,650-656,660-665,670-677,690-693,700-705,720-725,730-739,750-755,760-766,780-785,790-799,810-815,820-825,840-845,850-855,870-875,880-888,1202-1205,1260-1282,1290-1324,1332-1343,1350,1360,1365-1366,1400-1411,1430-1446,1550-1557,1560-1575",
  UI_32bit: "470-473",
  ChrSel: "0-18,20-35,40-55,60-75,80-95,100-115,140-155,220-227,300-315,320-335,340-355,360-375,560-567,600-615,660-669,880-895",
  Title: "0,5-7,14-17,20,25-26,30-32,40,45,51-52,57,90-96,99-115,119-122,130-138,142,149,156-158,163-164,166-182,193-218,237,270-278,280-282,284,286-295,300-314,320-334,340-354,356-358,360-362,370-372,382-387,411,468-469,483-485,500-504,506-508,516-517,520-522,530-542,560-565,567,570-595,607-618,633-638,660-672,674-676,680-688,728,737-739,743-746,749-783,785,820-829,848,850-851,853-856",
  "Gate/00": "0-3,8-11,16-19,24-33,43-49,56-61,64-69", "Gate/01": "0-8", "Gate/02": "0-8", "Gate/03": "0-8",
  Help: "0-41",
  Items: "0-261,264,266,270-271,273,277,280-287,296-298,312,315-316,320,328,336,344,360,368,391-392,394,396,398,400,402,404,406,408,412,420-426,428-430,432-434,444,446-448,452,466,481-484,490-491,502-503,505,510-518,520-534,540-548,550-555,557-558,560-565,568,575-597,600-607,610-624,670-684,690,700-705,710-715,720-726,730-736,740-746,750-814,820-828,837,849-855,857,859,870-872,874,878,893-910,912-915,934,941,960-962,970-972,1000-1028,1030-1033,1040-1043,1067-1068,1078,1095,1110-1112,1132,1150-1157,1170,1173,1176-1180,1182,1330-1332,1400-1401,1420-1422,1433,1693-1699,1701,1705,1707,1718-1719,1813,1942,1945,1948,1983-1985,1988,2040,2042,2044,2060-2061,2064-2067,2070,2085,2087-2088,2110-2114,2116-2118,2120,2123,2125,2240-2241,2250-2259,2270-2274,2276-2277,2280-2284,2286-2287,2313,2330-2334,2370,2380,2391,2393-2396,2398-2400,2440,2620,2760-2764,2770,2880-2887,2890,2901-2903,2912,2929-2931,2960-2961,3027,3140-3148,3200-3227,3290,3310,3313,3316,3319,3330-3333,3367-3368,3390-3391,3393-3395,3397-3410,3420-3424,3426,3430-3434,3436,3470-3499,3576,3614-3615,3640-3644,3660-3662,3670-3675,3699-3702,3706-3712,3721-3724,3730,3732-3734,3738-3740,3747-3749,3756-3757,3759-3761,3764-3767,3775-3776,3778-3779,3781-3786,3789-3790,3792,3861,3880-3882,3884-3889,3893-3894,4120-4122,4124-4126,4166-4169,4292,4400-4404,5050-5051,5060-5061,5070-5072,5080-5082,5090-5092,5101-5102,5110-5112,5120-5122,5130-5132,5140-5142,5150-5152",
  StateItem: "30-64,66-77,80-84,86-87,100-111,344,534,590-597,600-607,740-744,827-828,960-962,970-972,1030-1033,1040-1043,1150-1157,1330-1336,2240-2241,2250-2259,2270-2274,2276-2277,2280-2284,2286-2287,2313,2440,3140-3142,3290,3330-3333,3390-3391,3393-3395,3397-3410,3420-3424,3426,3430-3434,3436,3782,3880-3882,3884-3889,4400-4404,5050-5051,5060-5061,5070-5072,5130-5132,5140-5142,5150-5152",
  BuffIcon: "0-264", MagIcon: "0-223", MagIcon2: "0-223", GuildSkill: "0-47", MapLinkIcon: "100-109,111-112,114,116,118-120",
  MMap: "0-31,33-36,42,44-45,47-51,53,55-57,59-60,62-65,67-71,73-75,77,81-87,91-99,101-106,108-109,117-134,141-144,151-154,161-162,165-166,171-177,181-204,211-212,214-222,245,247-249,264-269,273-276,279,286,292-293,296,311-315,324,330-333,358,364-366,370-372,376,406-425,435-436,443-451",
};
const expandRanges = ranges => ranges.split(",").flatMap(range => {
  const [first, last = first] = range.split("-").map(Number);
  return Array.from({ length: last - first + 1 }, (_, i) => first + i);
});
const COMMON_UI_NAMED = {
  Cursors: "Cursor_Compulsion_Atk.CUR Cursor_Compulsion_Atk.png Cursor_Default.CUR Cursor_Default.png Cursor_Normal_Atk.CUR Cursor_Normal_Atk.png Cursor_Npc.cur Cursor_Npc.png Cursor_TextPrompt.CUR Cursor_Trash.CUR Cursor_Upgrade.CUR",
  // All present local clips plus the complete existing semantic event registry
  // and class-select voices. This does not select the full world sound archive.
  Sound: "005-1.wav 005-2.wav 005-3.wav 100.wav 103.wav 104.wav 105.wav 130-6.wav 138.wav 139.wav 144.wav 145.wav 52.wav 60.wav 61.wav 62.wav 63.wav 64.wav 65.wav 70.wav 71.wav 72.wav 73.wav 80.wav 81.wav 82.wav 83.wav 91.wav Login2.wav M100-0.wav M102-0.wav M103-0.wav M103-1.wav M104-0.wav M105-0.wav M105-1.wav M105-2.wav M121-0.wav M121-2.wav M121-5.wav M122-0.wav M122-2.wav M124-0.wav M124-5.wav M125-0.wav M125-5.wav M126-5.wav M126-6.wav M126-7.wav M126-8.wav M127-5.wav M127-6.wav M127-7.wav M128-0.wav M128-2.wav M129-0.wav M130-5.wav M130-6.wav M130-7.wav M131-0.wav M131-5.wav M132-7.wav M133-2.wav M135-2.wav M136-0.wav M136-5.wav M136-6.wav M136-7.wav M31-0.wav M31-1.wav M31-2.wav M34-0.wav M34-1.wav M34-2.wav M39-0.wav M39-1.wav M40-0.wav M41-1.wav M41-2.wav M45-2.wav M61-0.wav M61-1.wav M64-0.wav M64-1.wav M64-2.wav M69-0.wav M76-0.wav M79-1.wav M8-1.wav M91-0.wav M92-0.wav M92-1.wav M93-0.wav M95-0.wav M96-0.wav M97-0.wav M98-0.wav M98-1.wav M99-0.wav NewChar.wav Select2.wav pet_blackdragon.wav pet_chick.wav pet_frog.wav pet_kitty.wav pet_monkey.wav pet_olympicmascot.wav pet_pickup.wav pet_pig.wav pet_pigman.wav pet_skeleton.wav pet_weman.wav tiger_struck_1.wav tiger_struck_2.wav wolf_struck1.wav "
    + "106.wav 107.wav 108.wav 110.wav 111.wav 112.wav 113.wav 114.wav 115.wav 116.wav 117.wav 118.wav levelup.wav 50.wav 51.wav 53.wav 54.wav 55.wav 56.wav ride_walk_l.wav ride_walk_r.wav ride_run.wav fishing_throw.wav fishing_pull.wav fishing.wav "
    + "WarMSel.wav WarFSel.wav WizMSel.wav WizFSel.wav TaoMSel.wav TaoFSel.wav AssMSel.wav AssFSel.wav",
  fonts: "NotoSansCJKsc-Regular.otf OFL-NotoSansCJK.txt README.md "
    + "NotoSans-Regular.ttf NotoSans-Bold.ttf OFL-NotoSans.txt "
    + "NotoSansDevanagari-Regular.ttf NotoSansDevanagari-Bold.ttf OFL-NotoSansDevanagari.txt "
    + "NotoSansThai-Regular.ttf NotoSansThai-Bold.ttf OFL-NotoSansThai.txt "
    + "NotoSansArabic-Regular.ttf NotoSansArabic-Bold.ttf OFL-NotoSansArabic.txt NotoSansTC.ttf OFL-NotoSansTC.txt",
  "gdi-text": "manifest.json images/chat-line-message-net-8.png images/chat-online-players.png images/chat-system-error.png images/entity-assistant-jane.png images/entity-merchant-ruben-multiline.png images/hud-hp.png images/minimap-coordinate.png images/pt-br-accents.png",
};
export const R16_COMMON_UI_ASSET_PATHS = Object.freeze([
  ...Object.entries(COMMON_UI_RANGES).flatMap(([library, ranges]) => [
    ...expandRanges(ranges).map(index => `original-ui/${library}/${index}.png`), `original-ui/${library}/meta.json`]),
  ...Object.entries(COMMON_UI_NAMED).flatMap(([library, names]) => names.split(" ").map(name => `original-ui/${library}/${name}`)),
  "original-ui/sound-index.generated.json",
].sort());
const UI_SET = new Set([...R16_UI_ASSET_PATHS, ...R16_COMMON_UI_ASSET_PATHS]);
const SHA = /^[a-f0-9]{64}$/;
// The production renderer currently uses single-image raw sprites, not the
// profile atlases. Its complete 13-profile closure measures 21,019 files, so
// this engineering count cap changes from 20k to the smallest next thousand.
// The original 360MiB asset and 8MiB manifest byte budgets do not change.
const MAX_FILES = 22_000, MAX_BYTES = 360 * 1024 * 1024, MAX_MANIFEST = 8 * 1024 * 1024;
const CATEGORIES = ["ui", "map0", "map0141", "monsters"];
const COMMON_CATEGORIES = [...CATEGORIES, "firstplay"];
// This append is tied to the verified v13 baseline, not a full-world opt-in.
// Only schema3 gets the 24k engineering file ceiling; both byte budgets and
// schema1/2's 22k ceiling remain unchanged.
export const R16_FOREST_CAVE_BASE_SHA256 = "e7e13ea2aeb9cde489f6bc2a3d23927bcb72e0e4eb0a0ddfdbdc2289a7fdeb18";
export const R16_FOREST_CAVE_SCOPE = Object.freeze({ id: "forest-cave-v17", maps: ["D001"],
  npcLibraries: ["NPC/04", "NPC/06", "NPC/08", "NPC/16", "NPC/25", "NPC/27", "NPC/52", "NPC/83"],
  playerLibraries: ["CWeapon/05"], monsterLibraries: ["Monster/022"] });
export const R16_FOREST_CAVE_PATHS = Object.freeze({
  forestCityNpcs: R16_FOREST_CAVE_SCOPE.npcLibraries.flatMap((library, i) =>
    Array.from({ length: i < 4 ? 42 : 12 }, (_, frame) => `original-ui/${library}/${frame}.png`)).sort(),
  forestPlayer: Array.from({ length: 832 }, (_, frame) => `original-ui/CWeapon/05/${frame}.png`).sort(),
  caveMonster: Array.from({ length: 244 }, (_, frame) => `original-ui/Monster/022/${frame}.png`).sort(),
});
const FOREST_SET = new Set(Object.values(R16_FOREST_CAVE_PATHS).flat());
const FOREST_CATEGORIES = [...COMMON_CATEGORIES, ...Object.keys(R16_FOREST_CAVE_PATHS), "caveMap"];
const FOREST_MAP_PATHS_SHA256 = "c4c3ab024690c5867fda7d7ef2cecd9fa5bd0ca05a85f409e63b272854f8d890";
const FOREST_MAP_SOURCE_SHA256 = "b0c4ebf0c0999cdd1273fffb06ca31a8bc5171eeb68de532cdde2d7bd2a6b383";
// Schema4 closes the active map-0/D001 raw monster route and the temporary
// background's finite alternatives. It preserves the complete schema3 pins.
export const R16_ROUTE_BASE_SHA256 = "4c6837f63275261d78340213c51ebd6e9a03f3b020bdabd4108bb55e7953adb3";
const ROUTE_MONSTER_COUNTS = { "015": 22, "019": 224, "020": 221, "021": 272, "022": 0, "023": 224 };
export const R16_ROUTE_SCOPE = Object.freeze({ ...R16_FOREST_CAVE_SCOPE, id: "forest-cave-v19",
  monsterLibraries: Object.keys(ROUTE_MONSTER_COUNTS).map(id => `Monster/${id}`) });
export const R16_ROUTE_ASSET_PATHS = Object.freeze({
  mapRouteMonsters: Object.entries(ROUTE_MONSTER_COUNTS).flatMap(([id, count]) => [
    ...Array.from({ length: count }, (_, frame) => `original-ui/Monster/${id}/${frame}.png`),
    `original-ui/Monster/${id}/meta.json`,
  ]).sort(),
  transitionSamples: ["smtile-32", "smtile-52", "smtile-72", "smtile-80", "smtile-104", "tiles-0", "tiles-1"]
    .map(name => `debug/map-samples/${name}.png`).sort(),
});
const ROUTE_SET = new Set(Object.values(R16_ROUTE_ASSET_PATHS).flat());
const ROUTE_CATEGORIES = [...FOREST_CATEGORIES, ...Object.keys(R16_ROUTE_ASSET_PATHS)];
export const R16_NEWCOMER_BASE_SHA256 = "454f30224cdec496b0a5f9d69e1e4a51d29e80a8897b00c5c158baad9ece370a";
export const R16_NEWCOMER_SCOPE = Object.freeze({ id: "newcomer-web-30-v21", maps: ["1", "D401", "D021", "D022"],
  classes: ["Warrior", "Wizard", "Taoist"] });
const NEWCOMER_APPEND = {
  newcomerEquipment: { count: 13133,
    paths: "6e71ff1876d33e6c82ad80e38da5f34db0abcbf1c358cdeae6c747f2cd20bd10",
    files: "dedf8c65c6ca706a4cdae31bf903e697dcc8513f5bc5a16d46d2a5c8cd8acf05",
    path: /^original-ui\/(?:CArmour\/0[2-5]|CWeapon\/(?:0[5679]|1[01678]))\/(?:\d+\.png|meta\.json)$/ },
  newcomerMapAssets: { count: 2580,
    paths: "fbac98eeb62712dd318de621c8853bcd17d25a62d602a184683c7056797b6fc3",
    files: "59da1ba6569ee05009a0645b2777dca7b00cd0b09a83da651bfee0dec7cc6e83",
    path: /^original-map\/(?:WemadeMir2|ShandaMir2|WemadeMir3)(?:\/[A-Za-z0-9_-]+)+\/\d+\.png$/ },
  // Includes the summoned skeleton used in the Taoist quest line.
  newcomerMapMonsters: { count: 2749,
    paths: "8ec776915251546812b89c7b72867440a6d36d7430d3d2e805f84e4f3ad9639d",
    files: "d5514c3c23cc5dd52a7e760a3e286719f06fe6afa7efdb20237b543714c9680f",
    path: /^original-ui\/Monster\/(?:01[78]|02[79]|03[0-3]|069|07[08])\/(?:\d+\.png|meta\.json)$/ },
};
const NEWCOMER_MAP_SOURCE_PINS = {
  map1MapSha256: "e058d1006dc56830850b3d72b301383a4f10aea6477da412e5cf6efd3d43cd53",
  mapD401MapSha256: "734f2c5ce06defd46ebb3fc9a7d02a38c00ac7b070be03b317b0e6fa9aa78a71",
  mapD021MapSha256: "5e283984b31ee993c8d1157402f50d7e9d4984d8be92db49388c1e024e257ca2",
  mapD022MapSha256: "2738312fa1136b1880174e5e9fdc455ed42f707d1b210d18325b2e919d527333",
};
const FIRSTPLAY_RANGES = {
  "NPC/00": "0-41", "NPC/01": "0-41", "NPC/03": "0-41", "NPC/05": "0-31", "NPC/07": "0-41",
  "NPC/11": "0-41", "NPC/15": "0-41", "NPC/45": "0-5", "NPC/94": "0-11",
  "CArmour/00": "0-1615", "CArmour/01": "0-1615", "CHair/00": "0-631,808-1439", "CWeapon/00": "0-831",
  // Ordinary newcomer death/revive can observe this additional world entity.
  // Keep its whole physical animation without changing the respawn whitelist.
  "Monster/016": "0-21",
};
export const R16_FIRSTPLAY_LIBRARIES = Object.freeze(Object.keys(FIRSTPLAY_RANGES).sort());
export const R16_FIRSTPLAY_ASSET_PATHS = Object.freeze([
  ...Object.entries(FIRSTPLAY_RANGES).flatMap(([library, ranges]) => [
    ...expandRanges(ranges).map(index => `original-ui/${library}/${index}.png`), `original-ui/${library}/meta.json`]),
  "debug/map-samples/smtile-0.png",
].sort());
const FIRSTPLAY_SET = new Set(R16_FIRSTPLAY_ASSET_PATHS);
// Complete physical PNG sets from these exact R16 profiles' pinned meta.frames
// and pngSha256 witnesses. Every action is required by the active raw renderer;
// this does not select other monster libraries or the full world archive.
const COMMON_MONSTER_RANGES = {
  "Monster/000": "0-127", "Monster/003": "0-231", "Monster/004": "0-231", "Monster/005": "0-233",
  "Monster/006": "0-223", "Monster/007": "0-447", "Monster/009": "0-223", "Monster/010": "0-163",
  "Monster/011": "0-175", "Monster/012": "0-223", "Monster/013": "0-21", "Monster/014": "0-21", "Monster/139": "0-271",
};
export const R16_FIRSTPLAY_MONSTER_RAW_PATHS = Object.freeze(Object.entries(COMMON_MONSTER_RANGES).flatMap(([library, ranges]) =>
  expandRanges(ranges).map(index => `original-ui/${library}/${index}.png`)).sort());
const FULL_INDEX = "generated/crystal-packs/full/index.json";
const FULL_PREFIX = "generated/crystal-packs/full/";
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const sorted = values => [...new Set(values)].sort();
const pathsHash = values => sha(JSON.stringify(values));
const record = value => value !== null && typeof value === "object" && !Array.isArray(value);
const assert = (condition, message) => { if (!condition) throw new Error(`R16 playable assets: ${message}`); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const samePath = (a, b) => process.platform === "win32" ? a.toLowerCase() === b.toLowerCase() : a === b;
const outside = relative => relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative);

export function assertR16PlayableAssetPath(value, { forestCave = false, mapRoute = false } = {}) {
  assert(typeof value === "string" && value.length > 0 && value.length <= 240
    && !path.posix.isAbsolute(value) && !path.win32.isAbsolute(value)
    && !value.includes("\\") && !value.includes(":") && !value.includes("\0")
    && value.split("/").every(part => part && part !== "." && part !== ".."
      && !part.startsWith(".") && !/^(?:node_modules|private|secrets?)$/i.test(part)), "unsafe asset path");
  const monster = /^original-ui\/(Monster\/\d{3})\/(?:\d+\.png|meta\.json)$/.exec(value);
  const allowed = UI_SET.has(value) || FIRSTPLAY_SET.has(value) || forestCave && FOREST_SET.has(value)
    || mapRoute && ROUTE_SET.has(value)
    || monster && R16_MONSTER_LIBRARIES.includes(monster[1])
    || /^original-map\/(?:WemadeMir2|ShandaMir2|WemadeMir3)(?:\/[A-Za-z0-9_-]+)+\/\d+\.png$/.test(value)
    || /^bevy-entity-atlases\/(?:manifest\.json|[A-Za-z0-9_-]+\.png)$/.test(value)
    || value === FULL_INDEX
    || /^generated\/crystal-packs\/full\/libraries\/maps\/[a-f0-9-]+\.json$/.test(value)
    || /^generated\/crystal-packs\/full\/pages\/[a-f0-9]{2}\/[a-f0-9]{64}\.png$/.test(value);
  assert(allowed, `path outside the finite R16 asset families: ${value}`);
  return value;
}

/** Check each path component and the resolved file; never follow a selected link. */
export async function readR16RegularFile(root, relative, { directoryCache = new Map(), maxBytes = MAX_BYTES } = {}) {
  assert(path.isAbsolute(root), "public root must be absolute");
  assert(typeof relative === "string" && relative && !path.isAbsolute(relative) && !path.win32.isAbsolute(relative)
    && !relative.includes("\\") && !relative.includes(":") && relative.split("/").every(p => p && p !== "." && p !== ".."), "unsafe regular-file path");
  const absoluteRoot = path.resolve(root), canonicalRoot = await fs.realpath(absoluteRoot);
  const rootStat = await fs.lstat(absoluteRoot);
  assert(rootStat.isDirectory() && !rootStat.isSymbolicLink(), "public root must be a real directory");
  const segments = relative.split("/"); let current = absoluteRoot;
  for (const component of segments.slice(0, -1)) {
    current = path.join(current, component);
    if (!directoryCache.has(current)) directoryCache.set(current, fs.lstat(current).then(stat => {
      assert(stat.isDirectory() && !stat.isSymbolicLink(), `non-regular directory: ${relative}`);
    }));
    await directoryCache.get(current);
  }
  const file = path.join(absoluteRoot, ...segments), before = await fs.lstat(file);
  assert(before.isFile() && !before.isSymbolicLink() && before.size > 0 && before.size <= maxBytes, `non-regular/empty/oversize file: ${relative}`);
  const canonical = await fs.realpath(file), expected = path.join(canonicalRoot, ...segments);
  assert(samePath(canonical, expected), `resolved path changed: ${relative}`);
  const handle = await fs.open(file, "r");
  try {
    const opened = await handle.stat();
    assert(opened.isFile() && opened.dev === before.dev && opened.ino === before.ino && opened.size === before.size, `file changed before read: ${relative}`);
    const bytes = await handle.readFile(), after = await handle.stat();
    assert(bytes.length === before.size && after.size === before.size && after.mtimeMs === before.mtimeMs, `file changed during read: ${relative}`);
    assert(samePath(await fs.realpath(file), expected) && !(await fs.lstat(file)).isSymbolicLink(), `file changed after read: ${relative}`);
    return bytes;
  } finally { await handle.close(); }
}

export function validateR16PlayableManifest(manifest) {
  if (manifest?.schemaVersion === 5) return validateNewcomerAppend(manifest);
  const mapRoute = manifest?.schemaVersion === 4 && manifest?.uiScope === "common";
  const forestCave = (manifest?.schemaVersion === 3 || mapRoute) && manifest?.uiScope === "common";
  const common = (manifest?.schemaVersion === 2 || forestCave) && manifest?.uiScope === "common";
  const categories = mapRoute ? ROUTE_CATEGORIES : forestCave ? FOREST_CATEGORIES : common ? COMMON_CATEGORIES : CATEGORIES;
  const maxFiles = mapRoute ? 25_000 : forestCave ? 24_000 : MAX_FILES;
  assert(record(manifest) && same(Object.keys(manifest).sort(), ["schemaVersion", "kind", "profileId", "maps", "monsterLibraries", "requiredPaths", "sourcePins", "files", "totalBytes", ...(common ? ["uiScope"] : []), ...(forestCave ? ["baseManifestSha256", "appendScope"] : [])].sort()), "invalid manifest fields");
  assert((manifest.schemaVersion === 1 || common) && manifest.kind === R16_PLAYABLE_KIND && manifest.profileId === "platinum_fiveclass"
    && same(manifest.maps, forestCave ? ["0", "0141", "D001"] : ["0", "0141"]) && same(manifest.monsterLibraries, R16_MONSTER_LIBRARIES), "wrong R16 scope");
  assert(record(manifest.requiredPaths) && same(Object.keys(manifest.requiredPaths).sort(), [...categories].sort()), "missing required categories");
  assert(record(manifest.sourcePins) && Object.keys(manifest.sourcePins).length >= 6
    && Object.entries(manifest.sourcePins).every(([key, value]) => /^[A-Za-z][A-Za-z0-9]*Sha256$/.test(key) && typeof value === "string" && SHA.test(value)), "invalid source pins");
  for (const category of categories) {
    const values = manifest.requiredPaths[category];
    assert(Array.isArray(values) && values.length > 0 && values.length <= maxFiles && same(values, sorted(values)), `unsorted/empty required ${category}`);
    for (const value of values) assertR16PlayableAssetPath(value, { forestCave, mapRoute });
    assert(manifest.sourcePins[`${category}PathsSha256`] === pathsHash(values), `required ${category} path pin mismatch`);
  }
  assert(same(manifest.requiredPaths.ui, common ? R16_COMMON_UI_ASSET_PATHS : R16_UI_ASSET_PATHS), "missing/extra required R16 UI asset");
  if (common) {
    assert(same(manifest.requiredPaths.firstplay, R16_FIRSTPLAY_ASSET_PATHS), "missing/extra required first-play asset");
    const monsterPaths = new Set(manifest.requiredPaths.monsters);
    assert(R16_FIRSTPLAY_MONSTER_RAW_PATHS.every(file => monsterPaths.has(file)), "incomplete first-play monster raw fallback");
    for (const key of ["presentSoundsSha256", "soundEventsSha256", "soundIndexSha256"])
      assert(SHA.test(manifest.sourcePins[key]), `missing common source pin ${key}`);
    assert(manifest.sourcePins.soundIndexSha256 === manifest.files?.find(f => f.path === "original-ui/sound-index.generated.json")?.sha256, "sound index pin mismatch");
  }
  assert(manifest.requiredPaths.map0.every(p => p.startsWith("original-map/")), "non-map asset in map0 closure");
  const interior = manifest.requiredPaths.map0141;
  assert(interior.includes(FULL_INDEX) && interior.filter(p => p.includes("/libraries/maps/")).length === 1
    && interior.filter(p => p.includes("/pages/")).length === 6 && interior.length === 8, "incomplete 0141 full-pack closure");
  assert(R16_MONSTER_LIBRARIES.every(lib => manifest.requiredPaths.monsters.includes(`original-ui/${lib}/meta.json`)), "missing monster metadata");
  assert(manifest.requiredPaths.monsters.every(p => p.startsWith("original-ui/Monster/") || p.startsWith("bevy-entity-atlases/")), "non-monster asset in entity closure");
  assert(Array.isArray(manifest.files) && manifest.files.length > 0 && manifest.files.length <= maxFiles, "invalid finite file count");
  const seen = new Set(); let total = 0;
  for (const file of manifest.files) {
    assert(record(file) && same(Object.keys(file).sort(), ["path", "bytes", "sha256"].sort()), "invalid file fields");
    assertR16PlayableAssetPath(file.path, { forestCave, mapRoute });
    const folded = file.path.toLowerCase(); assert(!seen.has(folded), "duplicate/case-alias asset path"); seen.add(folded);
    assert(Number.isSafeInteger(file.bytes) && file.bytes > 0 && file.bytes <= MAX_BYTES && typeof file.sha256 === "string" && SHA.test(file.sha256), "invalid asset size/hash"); total += file.bytes;
  }
  assert(same(manifest.files.map(f => f.path), sorted(manifest.files.map(f => f.path))), "file entries must be sorted");
  assert(same(manifest.files.map(f => f.path), sorted(categories.flatMap(key => manifest.requiredPaths[key]))), "missing/extra required file entry");
  assert(total === manifest.totalBytes && total <= MAX_BYTES, "manifest byte total/budget mismatch");
  assert(manifest.sourcePins.fullPackIndexSha256 === manifest.files.find(f => f.path === FULL_INDEX)?.sha256, "full-pack index pin mismatch");
  if (forestCave) validateForestCaveAppend(manifest, mapRoute);
  return manifest;
}

function validateNewcomerAppend(manifest) {
  const baseMaps = ["0", "0141", "D001"];
  assert(manifest.baseManifestSha256 === R16_NEWCOMER_BASE_SHA256
    && same(manifest.maps, [...baseMaps, ...R16_NEWCOMER_SCOPE.maps])
    && record(manifest.appendScope)
    && same(Object.keys(manifest.appendScope).sort(), Object.keys(R16_NEWCOMER_SCOPE).sort())
    && Object.entries(R16_NEWCOMER_SCOPE).every(([key, value]) => same(manifest.appendScope[key], value)),
  "wrong newcomer append scope");
  assert(record(manifest.requiredPaths) && record(manifest.sourcePins) && Array.isArray(manifest.files)
    && manifest.files.length === 42724, "wrong newcomer append file count or groups");
  assert(Object.entries(NEWCOMER_MAP_SOURCE_PINS).every(([key, value]) => manifest.sourcePins[key] === value),
    "wrong newcomer map source pin");

  const added = new Set();
  for (const [group, spec] of Object.entries(NEWCOMER_APPEND)) {
    const paths = manifest.requiredPaths[group];
    assert(Array.isArray(paths) && paths.length === spec.count && same(paths, sorted(paths))
      && paths.every(value => typeof value === "string" && spec.path.test(value))
      && pathsHash(paths) === spec.paths && manifest.sourcePins[group + "PathsSha256"] === spec.paths,
    `wrong ${group} path pin`);
    const selected = new Set(paths);
    assert(pathsHash(manifest.files.filter(file => selected.has(file?.path))) === spec.files, `wrong ${group} file pins`);
    for (const item of selected) added.add(item);
  }

  const oldFiles = manifest.files.filter(file => !added.has(file?.path));
  const oldRequired = Object.fromEntries(Object.entries(manifest.requiredPaths).filter(([key]) => !Object.hasOwn(NEWCOMER_APPEND, key)));
  const extraPins = new Set([...Object.keys(NEWCOMER_MAP_SOURCE_PINS),
    ...Object.keys(NEWCOMER_APPEND).map(key => key + "PathsSha256")]);
  const oldPins = Object.fromEntries(Object.entries(manifest.sourcePins).filter(([key]) => !extraPins.has(key)));
  const ordered = object => Object.entries(object).sort(([a], [b]) => a.localeCompare(b, "en"));
  assert(pathsHash(oldFiles) === "5648487844ee99817f53a07ca7fdc4b1c9e9b5e7402665d13d30c49dd0302fe8"
    && pathsHash(ordered(oldPins)) === "15ffce01a4938976fedb0c8c361cdd6e122eeb63aa23fed6a4b3464d1627d39a"
    && pathsHash(ordered(oldRequired)) === "060483b722e8f9e2633b3315b85c4c4cb66e29d8be1331551c7311b9a2501d8d",
  "newcomer append changed the frozen v19 assets");
  validateR16PlayableManifest({ ...manifest, schemaVersion: 4, maps: baseMaps,
    baseManifestSha256: R16_ROUTE_BASE_SHA256, appendScope: R16_ROUTE_SCOPE,
    requiredPaths: oldRequired, sourcePins: oldPins, files: oldFiles, totalBytes: 288862544 });

  const paths = manifest.files.map(file => file.path);
  const expected = sorted([...oldFiles.map(file => file.path), ...added]);
  const total = manifest.files.reduce((bytes, file) => bytes + file.bytes, 0);
  assert(same(paths, expected) && new Set(paths.map(value => value.toLowerCase())).size === paths.length,
    "newcomer append contains missing, duplicate or aliased files");
  assert(total === manifest.totalBytes && total <= MAX_BYTES, "newcomer append byte total/budget mismatch");
  return manifest;
}

function validateForestCaveAppend(manifest, mapRoute) {
  const scope = mapRoute ? R16_ROUTE_SCOPE : R16_FOREST_CAVE_SCOPE;
  assert(manifest.baseManifestSha256 === (mapRoute ? R16_ROUTE_BASE_SHA256 : R16_FOREST_CAVE_BASE_SHA256)
    && record(manifest.appendScope)
    && same(Object.keys(manifest.appendScope).sort(), Object.keys(scope).sort())
    && Object.entries(scope).every(([key, value]) => same(manifest.appendScope[key], value)), "wrong forest/cave append scope");
  for (const [category, paths] of Object.entries(R16_FOREST_CAVE_PATHS))
    assert(same(manifest.requiredPaths[category], paths), `incomplete forest/cave ${category}`);
  assert(manifest.requiredPaths.caveMap.length === 938
    && pathsHash(manifest.requiredPaths.caveMap) === FOREST_MAP_PATHS_SHA256
    && manifest.sourcePins.mapD001MapSha256 === FOREST_MAP_SOURCE_SHA256, "wrong D001 map closure/source pin");
  const baseCategories = mapRoute ? FOREST_CATEGORIES : COMMON_CATEGORIES;
  const newCategories = (mapRoute ? ROUTE_CATEGORIES : FOREST_CATEGORIES).filter(key => !baseCategories.includes(key));
  if (mapRoute) {
    for (const [category, paths] of Object.entries(R16_ROUTE_ASSET_PATHS)) {
      assert(same(manifest.requiredPaths[category], paths), `incomplete map route ${category}`);
      const selected = new Set(paths);
      const digest = category === "mapRouteMonsters"
        ? "ed094b1d81d3cdb848c17f33ae7d7509846c70b8678dc58d2635a215536aad78"
        : "5a07ba4be42434231d9065265596afed55b1a45363ecd07103e3d1b370115ce4";
      assert(pathsHash(manifest.files.filter(file => selected.has(file.path))) === digest, `wrong map route ${category} file pins`);
    }
  }
  const appended = new Set(newCategories.flatMap(key => manifest.requiredPaths[key]));
  assert(appended.size === (mapRoute ? 976 : 2230), "wrong forest/cave append file count");
  const oldFiles = manifest.files.filter(file => !appended.has(file.path));
  const orderedEntries = object => Object.entries(object).sort(([a], [b]) => a.localeCompare(b, "en"));
  const oldPins = Object.fromEntries(Object.entries(manifest.sourcePins).filter(([key]) =>
    (mapRoute || key !== "mapD001MapSha256") && !newCategories.some(category => key === `${category}PathsSha256`)));
  const oldRequired = Object.fromEntries(baseCategories.map(key => [key, manifest.requiredPaths[key]]));
  const [count, filePin, sourcePin, requiredPin] = mapRoute
    ? [23286, "3d3effa317bfcd09714a3b6b65012f7df9c2e4034d29921eedbd1fb8adf730a6", "61c2fd299907ad9f63bb0b79acbd890abe5266fc26ae959bf19b5d35b3df74a4", "cf910777b260d9d6226d775cba20e341f910e1d45929d961245372d21503ce00"]
    : [21056, "121c7af6d83819572bf6edabca5226d4474f889421ec736924aff4c217ba37b7", "7bbe860b2e053aba40268a892f52c39683f146780ebd1f19653495b10c2a17a3", "7d1af92f70e56679460862b599af3e420feec1cf5bdcfb8053315d4895e68b0c"];
  assert(oldFiles.length === count && pathsHash(oldFiles) === filePin
    && pathsHash(orderedEntries(oldPins)) === sourcePin && pathsHash(orderedEntries(oldRequired)) === requiredPin,
  "forest/cave append changed the frozen baseline");
}

async function eachBounded(values, callback) {
  let index = 0;
  await Promise.all(Array.from({ length: Math.min(16, values.length) }, async () => {
    while (index < values.length) { const current = index++; await callback(values[current], current); }
  }));
}

export async function verifyR16PlayableAssetFiles(publicRoot, manifest) {
  validateR16PlayableManifest(manifest);
  const directoryCache = new Map();
  await eachBounded(manifest.files, async file => {
    const bytes = await readR16RegularFile(publicRoot, file.path, { directoryCache });
    assert(bytes.length === file.bytes && sha(bytes) === file.sha256, `asset size/SHA256 mismatch: ${file.path}`);
  });
  return { files: manifest.files.length, bytes: manifest.totalBytes, verified: true };
}

export async function selectR16PlayableAssetsClosure(publicRoot, { manifestPath = "", manifestSha256 = "" } = {}) {
  if (manifestPath === "" && manifestSha256 === "") return { files: new Set(), entries: [], bytes: 0, manifest: null, manifestSha256: null };
  assert(typeof manifestPath === "string" && path.isAbsolute(manifestPath) && typeof manifestSha256 === "string" && SHA.test(manifestSha256), "manifest requires absolute path and SHA256 together");
  const bytes = await readR16RegularFile(path.dirname(manifestPath), path.basename(manifestPath), { maxBytes: MAX_MANIFEST });
  assert(sha(bytes) === manifestSha256, "manifest SHA256 pin mismatch");
  const manifest = validateR16PlayableManifest(JSON.parse(bytes.toString("utf8")));
  // The generator materializes only the verified closure beside its manifest.
  // Never traverse a development public junction while packaging a release.
  const sourceRoot = `${manifestPath}.public`;
  assert(path.isAbsolute(publicRoot), "public root must be absolute");
  await verifyR16PlayableAssetFiles(sourceRoot, manifest);
  return { files: new Set(manifest.files.map(file => file.path)), entries: manifest.files, bytes: manifest.totalBytes,
    manifest, manifestBytes: bytes, manifestSha256, sourceRoot };
}

async function assetSourceReader(publicRoot, { soundRoot = "" } = {}) {
  // This one explicitly named source tree is a pre-existing development junction.
  // Resolve it once, then require regular descendants of its physical root.
  const fullRoot = await fs.realpath(path.join(publicRoot, ...FULL_PREFIX.split("/").filter(Boolean)));
  const physical = await fs.lstat(fullRoot);
  assert(physical.isDirectory() && !physical.isSymbolicLink(), "invalid physical full-pack source root");
  const publicCache = new Map(), fullCache = new Map();
  assert(soundRoot === "" || typeof soundRoot === "string" && path.isAbsolute(soundRoot), "sound root must be absolute");
  const soundCache = new Map();
  return async relative => {
    if (relative.startsWith(FULL_PREFIX)) return readR16RegularFile(fullRoot, relative.slice(FULL_PREFIX.length), { directoryCache: fullCache });
    try { return await readR16RegularFile(publicRoot, relative, { directoryCache: publicCache }); }
    catch (error) {
      // Only an explicitly selected, finite common clip may use the named raw
      // Sound source. Invalid links/paths/bytes never fall back to another tree.
      if (!soundRoot || error.code !== "ENOENT" || !relative.startsWith("original-ui/Sound/") || !UI_SET.has(relative)) throw error;
      return readR16RegularFile(soundRoot, relative.slice("original-ui/Sound/".length), { directoryCache: soundCache });
    }
  };
}

export async function materializeR16PlayableAssetFiles(publicRoot, destinationRoot, manifest, sourceOptions = {}) {
  validateR16PlayableManifest(manifest);
  assert(path.isAbsolute(destinationRoot), "materialized root must be absolute");
  const delta = path.relative(publicRoot, destinationRoot), reverse = path.relative(destinationRoot, publicRoot);
  assert(outside(delta), "materialized root overlaps source");
  assert(outside(reverse), "materialized source overlaps output");
  const read = await assetSourceReader(publicRoot, sourceOptions);
  await fs.mkdir(destinationRoot); // An existing/linked/partial sidecar is never reused.
  await eachBounded(manifest.files, async file => {
    const bytes = await read(file.path);
    assert(bytes.length === file.bytes && sha(bytes) === file.sha256, `source changed before materialization: ${file.path}`);
    const destination = path.join(destinationRoot, ...file.path.split("/"));
    await fs.mkdir(path.dirname(destination), { recursive: true });
    await fs.writeFile(destination, bytes, { flag: "wx" });
  });
  return verifyR16PlayableAssetFiles(destinationRoot, manifest);
}

/** Build from canonical UI contracts, actual map bytes, source frame metadata,
 * and the active profile. It reads assets and never exports or modifies them. */
export async function collectR16PlayableAssets({ webRoot, publicRoot = path.join(webRoot, "public"), uiScope = "r16", soundRoot = "" }) {
  assert(["r16", "common"].includes(uiScope), "unsupported UI scope");
  const common = uiScope === "common", categories = common ? COMMON_CATEGORIES : CATEGORIES;
  const mapTools = await import("../build-native-keyed-map-pack.mjs");
  const read = await assetSourceReader(publicRoot, { soundRoot });
  const indexBytes = await read(FULL_INDEX), index = JSON.parse(indexBytes);
  assert(index.kind === "mir2-crystal-full-pack-index" && Array.isArray(index.libraries), "invalid source full-pack index");
  const libraryCache = new Map();
  const library = async key => {
    if (!libraryCache.has(key)) libraryCache.set(key, (async () => {
      const entry = index.libraries.find(item => item.key === key); assert(entry && SHA.test(entry.manifestSha256), `missing source library ${key}`);
      const relative = assertR16PlayableAssetPath(entry.manifestUrl?.replace(/^\//, "")), bytes = await read(relative);
      assert(sha(bytes) === entry.manifestSha256, `source library pin mismatch ${key}`);
      const value = JSON.parse(bytes); assert(value.libraryKey === key && Array.isArray(value.frames), `invalid source frame metadata ${key}`);
      return { entry, value, relative };
    })());
    return libraryCache.get(key);
  };
  // Monster shards are source witnesses, not selected package assets.
  const sourceLibrary = async key => {
    if (key.startsWith("Map/")) return library(key);
    const entry = index.libraries.find(item => item.key === key); assert(entry && SHA.test(entry.manifestSha256), `missing source library ${key}`);
    assert(/^\/generated\/crystal-packs\/full\/libraries\/entities\/[a-f0-9-]+\.json$/.test(entry.manifestUrl), "unsafe source monster shard");
    const bytes = await read(entry.manifestUrl.slice(1)); assert(sha(bytes) === entry.manifestSha256, `source monster pin mismatch ${key}`);
    const value = JSON.parse(bytes); assert(value.libraryKey === key && Array.isArray(value.frames), "invalid source monster metadata"); return { entry, value };
  };
  const requiredPaths = { ui: [...(common ? R16_COMMON_UI_ASSET_PATHS : R16_UI_ASSET_PATHS)], map0: [], map0141: [FULL_INDEX], monsters: [],
    ...(common ? { firstplay: [...R16_FIRSTPLAY_ASSET_PATHS] } : {}) }, sourcePins = { fullPackIndexSha256: sha(indexBytes) };
  if (common) {
    const present = await readR16RegularFile(webRoot, "lib/generated/crystal-present-sounds.generated.json");
    const sounds = JSON.parse(present), events = await readR16RegularFile(webRoot, "lib/original-sound-events.ts");
    assert(sounds.schemaVersion === 1 && sounds.kind === "mir2-present-sound-manifest" && Array.isArray(sounds.files), "invalid present sound witness");
    for (const file of R16_COMMON_UI_ASSET_PATHS.filter(p => p.startsWith("original-ui/Sound/")))
      assert(sounds.files.includes(file.slice("original-ui/Sound/".length)), `common sound is not advertised by the current resolver: ${file}`);
    sourcePins.presentSoundsSha256 = sha(present); sourcePins.soundEventsSha256 = sha(events);
    sourcePins.soundIndexSha256 = sha(await read("original-ui/sound-index.generated.json"));
  }
  const diagnostics = { maps: {}, monsterRawFrames: 0, monsterAtlasFrames: 0 };
  for (const name of ["0", "0141"]) {
    const compressed = await readR16RegularFile(webRoot, `lib/generated/crystal-map-pack/${name}.map.gz`);
    sourcePins[`map${name}MapSha256`] = sha(compressed);
    const map = mapTools.parsePackagedMap(gunzipSync(compressed)), references = new Map();
    assert(map && map.width * map.height <= 1_500_000, "invalid bounded source map");
    const add = (index, base, count = 1, step = 1) => {
      if (index < 0 || base < 0) return;
      const key = mapTools.mapLibraryKeyForIndex(index); assert(key, "unknown source map library");
      for (let i = 0; i < Math.max(1, count); i++) references.set(`${key}#${base + i * step}`, { key, frame: base + i * step });
    };
    const raw = gunzipSync(compressed);
    for (let x = 0; x < map.width; x++) for (let y = 0; y < map.height; y++) {
      const cell = map.cells[x * map.height + y];
      if (!(x % 2) && !(y % 2)) add(cell.backIndex, (cell.backImage & 0x1fffffff) - 1);
      add(cell.middleIndex, cell.middleImage - 1, mapTools.decodeCrystalMiddleAnimationCount(cell.middleAnimationFrame));
      add(cell.frontIndex, (cell.frontImage & 32767) - 1, mapTools.decodeCrystalFrontAnimationCount(cell.frontAnimationFrame));
      if (raw[2] === 0x43 && raw[3] === 0x23) { const offset = 8 + (x * map.height + y) * 26, base = raw.readInt16LE(offset + 20) - 1, count = raw[offset + 24];
        if (base >= 0 && count > 0) add(190, base, count, raw.readInt16LE(offset + 22) ^ 0x2000); }
    }
    let noDraw = 0, invalidSlots = 0, drawable = 0;
    for (const reference of references.values()) {
      const source = await library(`Map/${reference.key}`), frame = source.value.frames[reference.frame];
      if (!frame) { invalidSlots++; continue; }
      if (frame.noDraw || frame.width <= 0 || frame.height <= 0) { noDraw++; continue; }
      drawable++;
      if (name === "0") requiredPaths.map0.push(`original-map/${reference.key}/${reference.frame}.png`);
      else { requiredPaths.map0141.push(source.relative);
        for (const url of [frame.imageUrl, frame.maskImageUrl].filter(Boolean)) requiredPaths.map0141.push(assertR16PlayableAssetPath(url.replace(/^\//, ""))); }
    }
    diagnostics.maps[name] = { drawable, noDraw, invalidSlots };
  }
  const profileBytes = await readR16RegularFile(path.resolve(webRoot, "../.."), "packages/game-data/data/content_profiles/platinum_fiveclass.json");
  const profile = JSON.parse(profileBytes), worldBytes = await readR16RegularFile(webRoot, "lib/generated/crystal_respawn_manifest.json");
  const world = JSON.parse(worldBytes), allowed = new Set(profile.monsterWhitelist), active = sorted(world.maps.find(map => map.map_file_name === "0").respawns
    .filter(item => allowed.has(item.monster_name)).map(item => `Monster/${String(item.monster_image).padStart(3, "0")}`));
  assert(same(active, sorted(R16_MONSTER_LIBRARIES)), "active R16 monster library set changed");
  sourcePins.profileSha256 = sha(profileBytes); sourcePins.respawnsSha256 = sha(worldBytes);
  const atlasBytes = await read("bevy-entity-atlases/manifest.json"), atlas = JSON.parse(atlasBytes);
  assert(atlas.schemaVersion === 2 && Array.isArray(atlas.atlases), "invalid local entity atlas");
  sourcePins.entityAtlasSha256 = sha(atlasBytes);
  const atlasFrames = new Set(atlas.atlases.flatMap(item => item.rects.map(rect => rect.key.split("|")[0])));
  const atlasPages = new Set();
  const visit = value => { if (Array.isArray(value)) for (const x of value) visit(x); else if (record(value)) for (const [key, x] of Object.entries(value)) {
    if (key === "imageUrl" && typeof x === "string" && x.startsWith("/bevy-entity-atlases/")) atlasPages.add(assertR16PlayableAssetPath(x.slice(1))); else if (record(x) || Array.isArray(x)) visit(x);
  } }; visit(atlas);
  for (const key of R16_MONSTER_LIBRARIES) {
    const source = await sourceLibrary(key); requiredPaths.monsters.push(`original-ui/${key}/meta.json`);
    for (const frame of source.value.frames.filter(f => !f.noDraw && f.width > 0 && f.height > 0)) {
      const file = `original-ui/${key}/${frame.index}.png`;
      if (atlasFrames.has(`/${file}`)) diagnostics.monsterAtlasFrames++;
      else { requiredPaths.monsters.push(file); diagnostics.monsterRawFrames++; }
    }
  }
  if (common) {
    const selected = new Set(requiredPaths.monsters);
    diagnostics.firstplayMonsterRawAdditions = R16_FIRSTPLAY_MONSTER_RAW_PATHS.filter(file => !selected.has(file)).length;
    requiredPaths.monsters.push(...R16_FIRSTPLAY_MONSTER_RAW_PATHS);
  }
  if (diagnostics.monsterAtlasFrames) requiredPaths.monsters.push("bevy-entity-atlases/manifest.json", ...atlasPages);
  for (const category of categories) { requiredPaths[category] = sorted(requiredPaths[category]); sourcePins[`${category}PathsSha256`] = pathsHash(requiredPaths[category]); }
  const paths = sorted(Object.values(requiredPaths).flat()), files = new Array(paths.length);
  await eachBounded(paths, async (relative, index) => { assertR16PlayableAssetPath(relative); const bytes = await read(relative);
    files[index] = { path: relative, bytes: bytes.length, sha256: sha(bytes) }; });
  const manifest = { schemaVersion: common ? 2 : 1, kind: R16_PLAYABLE_KIND, ...(common ? { uiScope } : {}), profileId: "platinum_fiveclass", maps: ["0", "0141"],
    monsterLibraries: [...R16_MONSTER_LIBRARIES], requiredPaths, sourcePins, files, totalBytes: files.reduce((n, file) => n + file.bytes, 0) };
  validateR16PlayableManifest(manifest);
  return { manifest, diagnostics };
}
