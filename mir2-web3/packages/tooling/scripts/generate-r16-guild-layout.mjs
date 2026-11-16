import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const SOURCE = "packages/game-data/data/r16-guild-layout.json";
const TS_OUTPUT = "apps/web/lib/r16-guild-layout.ts";
const RUST_OUTPUT = "apps/game-client/client-bevy/src/crystal_ui/r16_guild_layout.rs";
const fail = message => { throw Error(`R16 guild layout: ${message}`); };
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
function exact(value, keys, path) {
  if (!value || typeof value !== "object" || Array.isArray(value)
    || !same(Object.keys(value).sort(), [...keys].sort())) fail(`invalid fields at ${path}`);
}
function integer(value, path, minimum = 0, maximum = 1024) {
  if (!Number.isSafeInteger(value) || value < minimum || value > maximum) fail(`invalid integer at ${path}`);
}
function rect(value, path, bounds) {
  exact(value, ["left", "top", "width", "height"], path);
  for (const field of ["left", "top", "width", "height"]) integer(value[field], `${path}.${field}`, field === "width" || field === "height" ? 1 : 0);
  if (bounds && (value.left + value.width > bounds.width || value.top + value.height > bounds.height)) fail(`rectangle outside ${path}`);
}
function sprite(value, path) {
  if (typeof value !== "string") fail(`invalid asset at ${path}`);
  const match = /^\/original-ui\/(Title|Prguse|Prguse2)\/(0|[1-9][0-9]*)\.png$/.exec(value);
  if (!match || Number(match[2]) > 65535) fail(`invalid asset at ${path}`);
  return {path: value.slice(1), library: match[1], frame: Number(match[2])};
}
function button(value, path) {
  exact(value, ["normal", "hover", "active"], path);
  const frames = Object.entries(value).map(([key, pathValue]) => sprite(pathValue, `${path}.${key}`));
  if (frames.some(frame => frame.library !== frames[0].library)) fail(`mixed button libraries at ${path}`);
}
const STATUS_RECTS = ["rect", "backgroundRect", "titleRect", "headersRect", "nameRect", "levelRect", "membersRect",
  "recruitLabelRect", "recruitNameRect", "recruitButtonRect", "progressRect", "progressTextRect"];
const RANK_RECTS = ["editLabelRect", "selectLabelRect", "nameRect", "dropdownRect", "saveRect", "dropdownRowsRect",
  "dropdownHitRect", "scrollUpRect", "scrollDownRect", "thumbRect"];
const BUTTON_ASSETS = ["close", "save", "recruit", "dropdownArrow", "dropdownUp", "dropdownDown", "dropdownThumb"];
const SPRITE_ASSETS = ["background", "title", "permissionUnchecked", "permissionChecked", "statusBackground", "progressBackground", "progress"];

export function validateR16GuildLayout(value) {
  exact(value, ["schema", "id", "source", "window", "titleRect", "closeRect", "dragRect", "tabs", "pages", "ranks", "assets"], "root");
  if (value.schema !== 1 || value.id !== "windows-r16-guild-layout") fail("unknown schema or identity");
  exact(value.source, ["native", "crystal", "windowCoordinates", "controlCoordinates"], "source");
  if (value.source.native !== "apps/game-client/client-bevy/src/crystal_ui/overlays.rs"
    || value.source.crystal !== "Crystal/Client/MirScenes/Dialogs/GuildDialog.cs"
    || value.source.windowCoordinates !== "crystal-stage-1024x768" || value.source.controlCoordinates !== "window-local") fail("unknown source/coordinate contract");
  rect(value.window, "window", {width:1024,height:768});
  for (const key of ["titleRect", "closeRect", "dragRect"]) rect(value[key], key, value.window);
  const tabKeys = ["notice", "members", "storage", "ranks", "status", "buff"];
  if (!Array.isArray(value.tabs) || value.tabs.length !== tabKeys.length) fail("six tabs required");
  value.tabs.forEach((tab, index) => {
    exact(tab, ["key", "side", "rect", "assets", "pressed"], `tabs[${index}]`);
    if (tab.key !== tabKeys[index] || tab.side !== (index < 4 ? "left" : "right")
      || tab.pressed !== (tab.key === "ranks" ? "current" : "active")) fail("tab identity/state drift");
    rect(tab.rect, `tabs[${index}].rect`, value.window);
    exact(tab.assets, ["idle", "active"], `tabs[${index}].assets`);
    for (const [key, path] of Object.entries(tab.assets)) if (sprite(path, `tabs[${index}].assets.${key}`).library !== "Title") fail("tab library drift");
  });
  exact(value.pages, ["left", "status"], "pages");
  exact(value.pages.left, ["rect"], "pages.left"); rect(value.pages.left.rect, "pages.left.rect", value.window);
  const status = value.pages.status;
  exact(status, [...STATUS_RECTS, "recruitButtonDrawSize", "progressFillWidth"], "pages.status");
  for (const key of STATUS_RECTS) rect(status[key], `pages.status.${key}`, value.window);
  exact(status.recruitButtonDrawSize, ["width", "height"], "pages.status.recruitButtonDrawSize");
  integer(status.recruitButtonDrawSize.width, "recruit width", 1); integer(status.recruitButtonDrawSize.height, "recruit height", 1);
  integer(status.progressFillWidth, "progressFillWidth", 1, status.progressRect.width);
  exact(value.ranks, [...RANK_RECTS, "permissions", "dropdownRowStep", "dropdownMaxRows", "thumbTravel", "thumbDrag"], "ranks");
  for (const key of RANK_RECTS) rect(value.ranks[key], `ranks.${key}`, value.window);
  if (!Array.isArray(value.ranks.permissions) || value.ranks.permissions.length !== 8) fail("eight permission bits required");
  value.ranks.permissions.forEach((permission, index) => {
    exact(permission, ["bit", "rect", "checkedRect", "labelRect"], `permission[${index}]`);
    if (permission.bit !== index) fail("permission bit order drift");
    for (const key of ["rect", "checkedRect", "labelRect"]) rect(permission[key], `permission[${index}].${key}`, value.window);
  });
  integer(value.ranks.dropdownRowStep, "dropdownRowStep", 1);
  if (value.ranks.dropdownMaxRows !== 5) fail("Native dropdown has five rows");
  integer(value.ranks.thumbTravel, "thumbTravel", 1);
  exact(value.ranks.thumbDrag, ["minY", "maxY", "offsetY", "travel"], "thumbDrag");
  for (const [key, field] of Object.entries(value.ranks.thumbDrag)) integer(field, `thumbDrag.${key}`);
  if (value.ranks.thumbDrag.minY > value.ranks.thumbDrag.maxY || value.ranks.thumbDrag.travel === 0) fail("invalid thumb drag range");
  exact(value.assets, [...BUTTON_ASSETS, ...SPRITE_ASSETS], "assets");
  for (const key of BUTTON_ASSETS) button(value.assets[key], `assets.${key}`);
  for (const key of SPRITE_ASSETS) sprite(value.assets[key], `assets.${key}`);
  return value;
}

const snake = text => text.replace(/[A-Z]/g, letter => `_${letter}`).toUpperCase();
const float = value => `${value}.0`;
const rustRect = value => `CrystalRect::new(${[value.left,value.top,value.width,value.height].map(float).join(", ")})`;
const rustAsset = path => {
  const asset = sprite(path, "projection");
  return `GuildAsset { path: ${JSON.stringify(asset.path)}, library: ${JSON.stringify(asset.library)}, frame: ${asset.frame} }`;
};
function rectBindings(layout) {
  return [["WINDOW", ["window"]], ...["titleRect","closeRect","dragRect"].map(key => [snake(key),[key]]),
    ["LEFT_PAGE_RECT",["pages","left","rect"]], ...STATUS_RECTS.map(key => [`STATUS_${snake(key === "rect" ? "pageRect" : key)}`,["pages","status",key]]),
    ...RANK_RECTS.map(key => [`RANK_${snake(key)}`,["ranks",key]])]
    .map(([name,path]) => ({name,path,value:path.reduce((value,key) => value[key],layout)}));
}
export function projectR16GuildLayout(value) {
  const layout = validateR16GuildLayout(value);
  const header = `Generated from ${SOURCE}; run packages/tooling/scripts/generate-r16-guild-layout.mjs.`;
  const typeScript = `// ${header}\nexport type R16GuildRect = Readonly<{left: number; top: number; width: number; height: number}>;\nexport const R16_GUILD_LAYOUT = ${JSON.stringify(layout,null,2)} as const;\n`;
  const rectangles = rectBindings(layout);
  const rust = [`// ${header}`, "use super::spec::CrystalRect;", "",
    "#[derive(Debug, Clone, Copy, PartialEq, Eq)]", "pub struct GuildAsset { pub path: &'static str, pub library: &'static str, pub frame: u16 }",
    "#[derive(Debug, Clone, Copy)]", "pub struct GuildButtonAssets { pub normal: GuildAsset, pub hover: GuildAsset, pub active: GuildAsset }",
    "#[derive(Debug, Clone, Copy)]", "pub struct GuildTabLayout { pub key: &'static str, pub side: &'static str, pub rect: CrystalRect, pub idle: GuildAsset, pub active: GuildAsset, pub pressed_current: bool }",
    "#[derive(Debug, Clone, Copy)]", "pub struct GuildPermissionLayout { pub bit: u8, pub rect: CrystalRect, pub checked_rect: CrystalRect, pub label_rect: CrystalRect }", "",
    ...rectangles.map(({name,value}) => `pub const ${name}: CrystalRect = ${rustRect(value)};`), "",
    `pub const TABS: [GuildTabLayout; 6] = [`, ...layout.tabs.map(tab => `    GuildTabLayout { key: ${JSON.stringify(tab.key)}, side: ${JSON.stringify(tab.side)}, rect: ${rustRect(tab.rect)}, idle: ${rustAsset(tab.assets.idle)}, active: ${rustAsset(tab.assets.active)}, pressed_current: ${tab.pressed === "current"} },`), "];",
    ...layout.tabs.map((tab,index) => `pub const TAB_${tab.key.toUpperCase()}: GuildTabLayout = TABS[${index}];`), "",
    "pub const RANK_PERMISSIONS: [GuildPermissionLayout; 8] = [", ...layout.ranks.permissions.map(permission => `    GuildPermissionLayout { bit: ${permission.bit}, rect: ${rustRect(permission.rect)}, checked_rect: ${rustRect(permission.checkedRect)}, label_rect: ${rustRect(permission.labelRect)} },`), "];", "",
    ...SPRITE_ASSETS.map(key => `pub const ASSET_${snake(key)}: GuildAsset = ${rustAsset(layout.assets[key])};`),
    ...BUTTON_ASSETS.map(key => `pub const ASSET_${snake(key)}: GuildButtonAssets = GuildButtonAssets { ${["normal","hover","active"].map(field => `${field}: ${rustAsset(layout.assets[key][field])}`).join(", ")} };`), "",
    `pub const STATUS_RECRUIT_BUTTON_DRAW_SIZE: (f32, f32) = (${float(layout.pages.status.recruitButtonDrawSize.width)}, ${float(layout.pages.status.recruitButtonDrawSize.height)});`,
    `pub const STATUS_PROGRESS_FILL_WIDTH: f64 = ${float(layout.pages.status.progressFillWidth)};`,
    `pub const RANK_DROPDOWN_ROW_STEP: f32 = ${float(layout.ranks.dropdownRowStep)};`,
    `pub const RANK_DROPDOWN_MAX_ROWS: usize = ${layout.ranks.dropdownMaxRows};`,
    `pub const RANK_THUMB_TRAVEL: f32 = ${float(layout.ranks.thumbTravel)};`,
    ...Object.entries(layout.ranks.thumbDrag).map(([key,value]) => `pub const RANK_THUMB_DRAG_${snake(key)}: f32 = ${float(value)};`), "",
    "pub fn at(window: CrystalRect, local: CrystalRect) -> CrystalRect { CrystalRect::new(window.left + local.left, window.top + local.top, local.width, local.height) }", "",
    "#[cfg(test)]", "mod tests {", "    use super::*;", "    use serde_json::Value;",
    "    fn rect(actual: CrystalRect, source: &Value) { for (key,value) in [(\"left\",actual.left),(\"top\",actual.top),(\"width\",actual.width),(\"height\",actual.height)] { assert_eq!(f64::from(value),source[key].as_f64().unwrap(),\"{key}\"); } }",
    "    #[test]", "    fn r16_guild_layout_matches_canonical_source() {",
    `        let source: Value = serde_json::from_str(include_str!("../../../../../${SOURCE}")).unwrap();`,
    ...rectangles.map(({name,path}) => `        rect(${name}, &source${path.map(key => `[${JSON.stringify(key)}]`).join("")});`),
    "        for (index, tab) in TABS.iter().enumerate() { let json = &source[\"tabs\"][index]; rect(tab.rect,&json[\"rect\"]); assert_eq!(tab.key,json[\"key\"].as_str().unwrap()); assert_eq!(tab.idle.path,json[\"assets\"][\"idle\"].as_str().unwrap().trim_start_matches('/')); assert_eq!(tab.active.path,json[\"assets\"][\"active\"].as_str().unwrap().trim_start_matches('/')); }",
    "        for (index, permission) in RANK_PERMISSIONS.iter().enumerate() { let json = &source[\"ranks\"][\"permissions\"][index]; assert_eq!(usize::from(permission.bit),index); rect(permission.rect,&json[\"rect\"]); rect(permission.checked_rect,&json[\"checkedRect\"]); rect(permission.label_rect,&json[\"labelRect\"]); }",
    "        assert_eq!((WINDOW.left,WINDOW.top,WINDOW.width,WINDOW.height),(217.0,168.0,590.0,432.0));", "    }", "}", ""].join("\n");
  return {typeScript,rust};
}

export function assertR16GuildLayoutProjections(layout, actual) {
  const expected = projectR16GuildLayout(layout);
  if (!actual || actual.typeScript !== expected.typeScript || actual.rust !== expected.rust) fail("generated projections are stale or edited");
  return expected;
}
export function buildR16GuildLayout({readSource = relative => readFileSync(resolve(ROOT,relative),"utf8")} = {}) {
  const layout = validateR16GuildLayout(JSON.parse(readSource(SOURCE)));
  return {layout,...projectR16GuildLayout(layout)};
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.slice(2).some(arg => arg !== "--check") || process.argv.length > 3) fail("only --check is supported");
  const generated = buildR16GuildLayout(), checking = process.argv.includes("--check");
  if (checking) assertR16GuildLayoutProjections(generated.layout, {
    typeScript:readFileSync(resolve(ROOT,TS_OUTPUT),"utf8"), rust:readFileSync(resolve(ROOT,RUST_OUTPUT),"utf8"),
  });
  else for (const [path,contents] of [[TS_OUTPUT,generated.typeScript],[RUST_OUTPUT,generated.rust]]) {
    mkdirSync(dirname(resolve(ROOT,path)),{recursive:true}); writeFileSync(resolve(ROOT,path),contents,"utf8");
  }
  console.log(JSON.stringify({checked:checking,source:SOURCE,projections:[TS_OUTPUT,RUST_OUTPUT],tabs:generated.layout.tabs.length,permissions:generated.layout.ranks.permissions.length}));
}
