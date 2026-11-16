import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
function evaluate(path, dependencies = {}, globals = {}) {
  const result = ts.transpileModule(read(path), { fileName: path,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true } });
  const module = { exports: {} };
  new Function("require", "module", "exports", ...Object.keys(globals), result.outputText)(id => {
    assert(Object.hasOwn(dependencies, id), `Unexpected runtime dependency ${id}`);
    return dependencies[id];
  }, module, module.exports, ...Object.values(globals));
  return module.exports;
}
const bootstrap = evaluate("../lib/first-crystal-scene-request.ts");
const bootstrapChunk = evaluate("../lib/pinned-bootstrap-chunk.ts", { "./first-crystal-scene-request": bootstrap });
const nativeLocalization = evaluate("../lib/r16-native-localization.ts", {
  "./generated/r16-native-localization-startup.json": JSON.parse(read("../lib/generated/r16-native-localization-startup.json")),
  "./generated/r16-native-localization-delivery.json": JSON.parse(read("../lib/generated/r16-native-localization-delivery.json")),
  "./first-crystal-scene-request": bootstrap,
  "./pinned-bootstrap-chunk": bootstrapChunk,
});
const browserLocalization = evaluate("../lib/r16-browser-localization.ts");
const characterCreate = evaluate("../lib/character-create.ts");
const shellFlow = evaluate("../app/components/original-client-shell-flow.ts");
const layout = evaluate("../lib/r16-auth-layout.ts", {
  "./r16-native-localization": nativeLocalization, "./r16-browser-localization": browserLocalization,
});
const nativeRoot = "../../game-client/client-bevy/src/";
const spec = read(nativeRoot + "crystal_ui/spec.rs");
const nativeLogin = read(nativeRoot + "crystal_ui/login.rs");
const nativeShell = read(nativeRoot + "native_shell_ui.rs");
const nativeShellModel = read(nativeRoot + "native_shell.rs");
const nativeSelect = read(nativeRoot + "crystal_ui/select.rs");
const nativePassword = read(nativeRoot + "crystal_ui/change_password.rs");

function nativeRect(source, name) {
  const declaration = source.match(new RegExp(`(?:pub )?const ${name}:[^;]+;`, "s"))?.[0];
  assert(declaration, `Missing native declaration ${name}`);
  const match = declaration.match(/CrystalRect::new\(([\d.]+),\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)\)/);
  assert(match, `Missing native rectangle ${name}`);
  const [left, top, width, height] = match.slice(1).map(Number);
  return { left, top, width, height };
}

function nativeRectList(source, name) {
  const declaration = source.split(`const ${name}:`)[1].split("];", 1)[0];
  return [...declaration.matchAll(/CrystalRect::new\(([\d.]+),\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)\)/g)]
    .map(([, left, top, width, height]) => ({ left: Number(left), top: Number(top), width: Number(width), height: Number(height) }));
}

test("browser auth hit rectangles match the independent R16 native source", () => {
  const loginSpec = spec.split("pub mod login {")[1].split("pub mod safe_key")[0];
  for (const [webKey, sourceKey] of Object.entries({ panel: "PANEL", account: "ACCOUNT_FIELD", password: "PASSWORD_FIELD",
    ok: "OK", newAccount: "NEW_ACCOUNT", changePassword: "CHANGE_PASSWORD", safeKey: "SAFE_KEY", cancel: "CANCEL" })) {
    assert.deepEqual(layout.R16_AUTH_LAYOUT.login[webKey], nativeRect(loginSpec, sourceKey));
  }
  assert.deepEqual(layout.R16_AUTH_LAYOUT.login.wallet, nativeRect(nativeLogin, "WALLET_LOGIN_BUTTON_RECT"));
  const selectSpec = spec.split("pub mod character_select {")[1].split("pub mod hud")[0];
  for (const [webKey, sourceKey] of Object.entries({ start: "START", new: "NEW_CHARACTER", delete: "DELETE_CHARACTER", credits: "CREDITS", exit: "EXIT" })) {
    const left = Number(selectSpec.match(new RegExp(`const ${sourceKey}:.*?bottom_button\\(\\d+, ([\\d.]+)\\)`))?.[1]);
    assert(Number.isFinite(left), sourceKey);
    assert.deepEqual(layout.R16_AUTH_LAYOUT.select.buttons[webKey], { left, top: 736, width: 100, height: 25 });
  }
  assert.match(selectSpec, /PREVIEW_FRAME_DELAY_SECONDS: f32 = 0\.25/);
  assert.equal(layout.R16_AUTH_LAYOUT.select.previewDelayMs, 250);
  for (const [webKey, sourceKey] of Object.entries({ panel: "NEW_CHARACTER_FRAME", name: "NEW_CHARACTER_NAME_FIELD", submit: "NEW_CHARACTER_CREATE", cancel: "NEW_CHARACTER_CANCEL" })) {
    assert.deepEqual(layout.R16_AUTH_LAYOUT.create[webKey], nativeRect(nativeShell, sourceKey));
  }
  assert.match(nativeShell, new RegExp(`MAX_NAME: usize = ${layout.R16_AUTH_LAYOUT.create.maxNameLength};`));
});

test("login opening follows the native 18-frame clock and keeps login music", () => {
  assert.match(nativeShellModel, /login_opening_elapsed\.as_millis\(\)/);
  assert.equal(shellFlow.LOGIN_TRANSITION_FRAME_MS, 100);
  assert.deepEqual([0, 99, 100, 1799, 1800].map(ms => shellFlow.loginDoorFrameAt(ms, 19)), [1, 1, 2, 18, null]);
  assert.equal(shellFlow.desiredMusicForScreen("openingLogin"), shellFlow.ORIGINAL_AUDIO.loginMusic);
  assert.equal(shellFlow.desiredMusicForScreen("select"), shellFlow.ORIGINAL_AUDIO.selectMusic);
});

test("localized create captions, slots and notice match the Native bounds without overlap", () => {
  const create = layout.R16_AUTH_LAYOUT.create;
  assert.deepEqual(create.classLabel, nativeRect(nativeShell, "LOCALIZED_NEW_CHARACTER_CLASS_LABEL"));
  assert.deepEqual(create.genderLabel, nativeRect(nativeShell, "LOCALIZED_NEW_CHARACTER_GENDER_LABEL"));
  assert.deepEqual(create.classes, nativeRectList(nativeShell, "LOCALIZED_NEW_CHARACTER_CLASS_BUTTONS"));
  assert.deepEqual(create.genders, nativeRectList(nativeShell, "LOCALIZED_NEW_CHARACTER_GENDER_BUTTONS"));
  assert.deepEqual(create.notice, nativeRect(nativeShell, "NEW_CHARACTER_NOTICE"));
  const bottom = box => box.top + box.height;
  assert(bottom(create.name) <= create.classLabel.top);
  assert(create.classes.every(box => bottom(create.classLabel) <= box.top && bottom(box) <= create.genderLabel.top));
  assert(create.genders.every(box => bottom(create.genderLabel) <= box.top && bottom(box) <= create.submit.top));
  assert(bottom(create.submit) <= create.notice.top);
});

test("bounded English/Simplified/Portuguese captions remain exact native catalog projections", () => {
  const shell = JSON.parse(read("../../../packages/game-data/data/native-i18n/shell.json")).entries;
  const simplified = JSON.parse(read("../../../packages/game-data/data/native-i18n/extra/zh-CN.json"));
  for (const [key, values] of Object.entries(layout.R16_AUTH_CAPTIONS)) {
    const native = shell.find(entry => entry.key === `shell.${key}`);
    assert(native, key);
    assert.deepEqual(values, [native.en, simplified[native.key], native["pt-BR"]], key);
    assert.equal(layout.R16_AUTH_TRADITIONAL[key], native["zh-TW"], key);
    assert.equal(layout.r16AuthCaption("zh-TW", key), native["zh-TW"]);
    assert.equal(layout.r16AuthCaption("es", key, "Texto existente"), "Texto existente");
  }
});

test("registration and change-password helpers preserve the actual native fields, bounds and Title art", () => {
  for (const [key, name] of Object.entries({ panel: "NEW_ACCOUNT_FRAME", description: "NEW_ACCOUNT_DESCRIPTION", submit: "NEW_ACCOUNT_OK", cancel: "NEW_ACCOUNT_CANCEL" })) {
    assert.deepEqual(layout.R16_AUTH_LAYOUT.registration[key], nativeRect(nativeShell, name));
  }
  const fields = [...nativeShell.split("const NEW_ACCOUNT_FIELDS:")[1].split("];", 1)[0]
    .matchAll(/RegistrationFocus::(\w+),\s*spec::CrystalRect::new\(([\d.]+),\s*([\d.]+),\s*([\d.]+),\s*([\d.]+)\)/g)];
  assert.equal(fields.length, 8);
  for (const [, name, ...coordinates] of fields) {
    const [left, top, width, height] = coordinates.map(Number);
    assert.deepEqual(layout.R16_AUTH_LAYOUT.registration.fields[name[0].toLowerCase() + name.slice(1)], { left, top, width, height });
  }
  const centre = name => { const match = nativePassword.match(new RegExp(`const ${name}: f32 = \\(([\\d.]+) - ([\\d.]+)\\) / 2\\.0;`)); assert(match, name); return (Number(match[1]) - Number(match[2])) / 2; };
  const left = centre("LEFT"), top = centre("TOP");
  assert.match(nativePassword, /node\(0\.0, 0\.0, 348\.0, 268\.0\)/);
  assert.deepEqual(layout.R16_AUTH_LAYOUT.changePassword.panel, { left, top, width: 348, height: 268 });
  const passwordFields = [...nativePassword.split("let fields = [")[1].split("];", 1)[0].matchAll(/ChangePasswordFocus::(\w+),\s*([\d.]+),/g)];
  assert.equal(passwordFields.length, 4); assert.match(nativePassword, /node\(178\.0, y, 136\.0, 18\.0\)/);
  for (const [, name, y] of passwordFields) assert.deepEqual(layout.R16_AUTH_LAYOUT.changePassword.fields[name[0].toLowerCase() + name.slice(1)],
    { left: left + 178, top: top + Number(y), width: 136, height: 18 });
  const actions = [...nativePassword.matchAll(/\(\s*(107|110),\s*([\d.]+),\s*([\d.]+),\s*NativeShellButton::(Submit|Cancel)ChangePassword/g)];
  assert.equal(actions.length, 2);
  for (const [, index, x, width, action] of actions) {
    assert.deepEqual(layout.R16_AUTH_LAYOUT.changePassword[action === "Submit" ? "submit" : "cancel"], { left: left + Number(x), top: top + 236, width: Number(width), height: 25 });
    const art = layout.R16_AUTH_ASSETS[action === "Submit" ? "passwordSubmit" : "passwordCancel"];
    assert.deepEqual(Object.values(art), [0, 1, 2].map(offset => `/original-ui/Title/${Number(index) + offset}.png`));
    for (const path of Object.values(art)) assert(readFileSync(new URL(`../public${path}`, import.meta.url)).length > 0);
  }
});

test("SafeKey uses the native 10-then-26 split and all 36 hit boxes clear its tools", () => {
  const safeSpec = spec.split("pub mod safe_key {")[1].split("pub mod character_select")[0];
  assert.match(safeSpec, /KEY_COUNT: usize = 36/); assert.match(safeSpec, /2 \+ slot \/ KEY_COLUMNS/);
  const panel = layout.R16_AUTH_LAYOUT.safeKey.panel;
  assert.deepEqual(panel, { left: 689, top: 395, width: 216, height: 278 });
  assert.equal(layout.R16_AUTH_ASSETS.safePanel, "/original-ui/Prguse/1080.png");
  for (const path of [layout.R16_AUTH_ASSETS.safePanel, ...Object.values(layout.R16_AUTH_ASSETS.safeKey)]) assert(readFileSync(new URL(`../public${path}`, import.meta.url)).length > 0);
  const overlap = (a, b) => a.left < b.left + b.width && b.left < a.left + a.width && a.top < b.top + b.height && b.top < a.top + a.height;
  const keys = Array.from({ length: 36 }, (_, index) => layout.r16AuthSafeKeyRect(index));
  assert.deepEqual(keys[9], { left: 797, top: 471, width: 32, height: 30 });
  assert.deepEqual(keys[10], { left: 701, top: 503, width: 32, height: 30 });
  assert.deepEqual(keys[35], { left: 733, top: 631, width: 32, height: 30 });
  for (const [index, box] of keys.entries()) {
    assert(box.left >= panel.left && box.top >= panel.top && box.left + box.width <= panel.left + panel.width && box.top + box.height <= panel.top + panel.height);
    for (const other of keys.slice(index + 1)) assert.equal(overlap(box, other), false);
    for (const name of ["esc", "delete", "enter", "random"]) assert.equal(overlap(box, layout.R16_AUTH_LAYOUT.safeKey[name]), false);
  }
  for (const invalid of [-1, 36, .5, NaN]) assert.equal(layout.r16AuthSafeKeyRect(invalid), null);
});

test("four-slot availability never starts synthetic characters or exceeds the native cap", () => {
  assert.deepEqual(layout.r16SelectAvailability([], 0), { hasSelection: false, canCreate: true });
  assert.deepEqual(layout.r16SelectAvailability([{ synthetic: true }], 0), { hasSelection: false, canCreate: true });
  const characters = Array.from({ length: 4 }, () => ({}));
  assert.deepEqual(layout.r16SelectAvailability(characters, 3), { hasSelection: true, canCreate: false });
  for (const invalid of [-1, 4, 0.5, NaN]) assert.equal(layout.r16SelectAvailability(characters, invalid).hasSelection, false);
});

test("Wizard additive frames use the canonical R16 offsets and local source art", () => {
  const preview = read(nativeRoot + "crystal_ui/preview_data.rs");
  for (const [gender, base] of [["male", 600], ["female", 880]]) {
    const source = preview.match(new RegExp(`const F${base}:[^=]+=(.*?);`, "s"))?.[1];
    assert(source);
    const frames = [...source.matchAll(/PreviewFrame::new\([\d.]+,\s*[\d.]+,\s*(-?[\d.]+),\s*(-?[\d.]+)\)/g)];
    assert.equal(frames.length, 16);
    for (const [index, frame] of frames.entries()) {
      const layer = layout.r16WizardPreviewOverlay("wizard", gender, index);
      assert.deepEqual([layer.x, layer.y], frame.slice(1).map(Number));
      assert.equal(layer.path, `/original-ui/ChrSel/${base + index}.png`);
      assert(readFileSync(new URL(`../public${layer.path}`, import.meta.url)).length > 0);
    }
    assert.deepEqual(layout.r16WizardPreviewOverlay("wizard", gender, 16), layout.r16WizardPreviewOverlay("wizard", gender, 0));
  }
  assert.equal(layout.r16WizardPreviewOverlay("warrior", "male", 0), null);
});

function hookHost() {
  const state = [], effects = [], frames = new Map(); let cursor = 0, current = state;
  return { state, effects, reset(key = "default") { cursor = 0; effects.length = 0; if (!frames.has(key)) frames.set(key, []); current = frames.get(key); },
    react: {
      useState(initial) { const index = cursor++, owned = current; if (!(index in owned)) owned[index] = typeof initial === "function" ? initial() : initial;
        return [owned[index], value => { owned[index] = typeof value === "function" ? value(owned[index]) : value; }]; },
      useRef(initial) { const index = cursor++; if (!(index in current)) current[index] = { current: initial }; return current[index]; },
      useEffect(effect) { effects.push(effect); },
    } };
}
const jsx = (type, props) => ({ type, props: props ?? {} });
const originalUi = evaluate("../lib/original-ui.ts");
const portraits = evaluate("../lib/select-portraits.ts");
function overlayHost() {
  const hooks = hookHost();
  const noop = () => {};
  const listeners = [];
  const document = { activeElement: null, addEventListener(type, callback, capture) { listeners.push({ type, callback, capture }); },
    removeEventListener(type, callback, capture) { const index = listeners.findIndex(row => row.type === type && row.callback === callback && row.capture === capture); if (index >= 0) listeners.splice(index, 1); } };
  const exports = evaluate("../app/components/original-client-overlays.tsx", {
    react: hooks.react, "react/jsx-runtime": { jsx, jsxs: jsx, Fragment: "fragment" },
    "../../lib/original-ui": originalUi, "../../lib/r16-auth-layout": layout, "../../lib/select-portraits": portraits,
    "../../lib/localization": { SUPPORTED_LANGUAGES: nativeLocalization.R16_NATIVE_LANGUAGES, languageNativeName: code => code,
      languageFontFamily: code => nativeLocalization.nativeLanguageInfo(code)?.fontFamily ?? "sans-serif" },
    "../../lib/crystal-hud-metrics": {}, "../../lib/extended-server-packets": {},
    "../../lib/original-audio": { playOriginalSoundId: noop }, "../../lib/original-sound-events": { ORIGINAL_SOUND_IDS: {} },
    "./original-client-audio-settings": { OriginalAudioSettingsControls() { return null; } },
    "./crystal-gdi-text": {}, "./dubhe-wallet-qr-login": { DubheWalletQrLogin: noop },
    "./original-client-scene-map-rendering": {},
  }, { document });
  return { exports, hooks, listeners, document, render(name, props) { hooks.reset(name); return exports[name](props); },
    renderComponent(component, props) { hooks.reset(component); return component(props); } };
}
function all(node) {
  if (Array.isArray(node)) return node.flatMap(all);
  if (!node || typeof node !== "object") return [];
  return [node, ...all(node.props?.children)];
}
function byClass(tree, name) { return all(tree).find(node => node.props.className?.split(" ").includes(name)); }
function keyEvent(key, extra = {}) {
  return { key, nativeEvent: { isComposing: false }, prevented: false, stopped: false,
    preventDefault() { this.prevented = true; this.defaultPrevented = true; }, stopPropagation() { this.stopped = true; }, ...extra };
}
function loginProps() {
  return { language: "zh-CN", t: (_key, _params, fallback) => fallback ?? _key,
    runtimePhase: "ready", runtimeMessage: "", wsState: "connected", accountId: "Existing", password: "opaque", loginBusy: false, loginError: null,
    loginAuth: { state: { surface: "login", epoch: 1, focusField: null, safeFocus: "account" }, ready: true, pending: false, safeFocus() {} },
    suiWallets: [], walletPickerOpen: false, dubheWalletUrl: "https://example.invalid", dubheQr: null };
}

function auxView(host, surface, overrides = {}) {
  const calls = [], props = loginProps();
  const state = { ...props.loginAuth.state, surface, notice: null, safeKeys: "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ",
    registration: { accountId: "Account", password: "StrongPass10", confirmPassword: "StrongPass10", userName: "", birthDate: "", secretQuestion: "", secretAnswer: "", emailAddress: "" },
    changePassword: { accountId: "Account", oldPassword: "OldPass", newPassword: "NewPass", confirmPassword: "NewPass" }, ...overrides.state };
  const controls = { ...props.loginAuth, state, ...Object.fromEntries(["close", "registrationChange", "passwordChange", "submitRegistration", "submitChangePassword", "safeFocus", "safePress", "safeDelete", "safeRandom", "safeEnter"]
    .map(name => [name, (...args) => { calls.push([name, ...args]); }])), ...overrides, state };
  const overlay = host.render("LoginOverlay", { ...props, loginAuth: controls });
  const entry = all(overlay).find(node => node.type?.name === "LoginAuthDialog"); assert(entry);
  const tree = host.renderComponent(entry.type, entry.props), nodes = all(tree);
  for (const node of nodes) node.focus = () => { host.document.activeElement = node; node.props.onFocus?.(); };
  const form = nodes.find(node => node.type === "form"), fields = nodes.filter(node => node.type === "input");
  const submit = nodes.find(node => node.props["data-login-auth-submit"]), close = nodes.find(node => node.props["data-login-auth-close"]);
  form.querySelectorAll = selector => (selector.startsWith("[data-login-auth-field]") ? fields : nodes.filter(node => ["input", "button"].includes(node.type))).filter(node => !node.props.disabled);
  form.querySelector = selector => {
    if (selector.startsWith("[data-login-auth-field=")) return fields.find(node => selector.includes(`"${node.props["data-login-auth-field"]}"`) && !node.props.disabled);
    return submit.props.disabled ? null : submit;
  };
  return { tree, form, fields, submit, close, calls, state };
}

test("actual auxiliary forms place all 8/4 fields and action hit boxes at Native stage coordinates", () => {
  for (const surface of ["registration", "changePassword"]) {
    const host = overlayHost(), view = auxView(host, surface), target = layout.R16_AUTH_LAYOUT[surface];
    assert.deepEqual(view.form.props.style, target.panel);
    assert.equal(view.fields.length, surface === "registration" ? 8 : 4);
    assert.equal(view.form.props["data-login-auth-surface"], surface);
    for (const field of view.fields) {
      assert.deepEqual(field.props.style, layout.r16AuthRelativeRect(target.fields[field.props.name], target.panel));
      assert.equal(field.props.maxLength, ({ userName: 20, birthDate: 10, secretQuestion: 30, secretAnswer: 30, emailAddress: 50 })[field.props.name] ?? 15);
      const changed = "value " + field.props.name; field.props.onChange({ target: { value: changed } });
      assert.deepEqual(view.calls.at(-1), [surface === "registration" ? "registrationChange" : "passwordChange", field.props.name, changed]);
    }
    assert.deepEqual(view.submit.props.style, layout.r16AuthRelativeRect(target.submit, target.panel));
    assert.deepEqual(view.close.props.style, layout.r16AuthRelativeRect(target.cancel, target.panel));
    if (surface === "changePassword") {
      assert.deepEqual(all(view.submit).filter(node => node.type === "img").map(node => node.props.src), Object.values(layout.R16_AUTH_ASSETS.passwordSubmit));
      assert.deepEqual(all(view.close).filter(node => node.type === "img").map(node => node.props.src), Object.values(layout.R16_AUTH_ASSETS.passwordCancel));
    } else {
      const hints = all(view.tree).filter(node => node.props["data-r16-registration-help"]);
      assert.equal(hints.length, 10);
      for (const field of view.fields) assert(hints.some(node => node.props.id === field.props["aria-describedby"]));
      assert.deepEqual(byClass(view.tree, "login-auth-description").props.style, layout.r16AuthRelativeRect(target.description, target.panel));
    }
  }
});

test("actual registration Enter advances, change-password Enter waits, and both Tab boundaries wrap", () => {
  for (const surface of ["registration", "changePassword"]) {
    const host = overlayHost(), view = auxView(host, surface);
    for (const [index, field] of view.fields.entries()) {
      field.focus();
      const event = keyEvent("Enter", { currentTarget: view.form, target: field }); view.form.props.onKeyDown(event);
      assert.equal(event.prevented, true); assert.equal(event.stopped, true);
      assert.equal(host.document.activeElement, surface === "registration" ? view.fields[index + 1] ?? view.submit : field);
      assert.equal(view.calls.length, 0);
    }
    view.fields[0].focus(); view.form.props.onKeyDown(keyEvent("Tab", { shiftKey: true, currentTarget: view.form }));
    assert.equal(host.document.activeElement, view.close);
    view.form.props.onKeyDown(keyEvent("Tab", { currentTarget: view.form })); assert.equal(host.document.activeElement, view.fields[0]);
    view.form.props.onSubmit(keyEvent("Enter")); assert.deepEqual(view.calls, [[surface === "registration" ? "submitRegistration" : "submitChangePassword"]]);
    view.close.props.onClick(); assert.deepEqual(view.calls.at(-1), ["close"]);
  }
});

test("actual SafeKey stays beside the original login, preserves shared key order and routes physical keys", () => {
  const host = overlayHost(), view = auxView(host, "safeKey"), panel = layout.R16_AUTH_LAYOUT.safeKey.panel;
  assert.deepEqual(view.form.props.style, panel);
  assert.equal(byClass(view.tree, "r16-safe-panel").props.src, layout.R16_AUTH_ASSETS.safePanel);
  const keys = all(view.tree).filter(node => node.props["data-login-safe-key"]);
  assert.equal(keys.length, 36); assert.deepEqual(keys.map(node => node.props["data-login-safe-key"]).join(""), view.state.safeKeys);
  for (const [index, key] of keys.entries()) {
    assert.deepEqual(key.props.style, layout.r16AuthRelativeRect(layout.r16AuthSafeKeyRect(index), panel));
    key.props.onMouseDown(keyEvent("mousedown")); key.props.onClick();
    assert.deepEqual(view.calls.at(-1), ["safePress", view.state.safeKeys[index]]);
  }
  assert.equal(view.fields.length, 2);
  for (const [index, field] of view.fields.entries()) {
    assert.equal(field.props.readOnly, true);
    assert.deepEqual(field.props.style, layout.r16AuthRelativeRect(layout.R16_AUTH_LAYOUT.login[index ? "password" : "account"], panel));
    assert.equal(field.props.maxLength, index ? 32 : 24);
  }
  view.fields[0].focus();
  view.form.props.onKeyDown(keyEvent("Tab", { shiftKey: true, currentTarget: view.form }));
  assert.equal(host.document.activeElement, view.fields[1]); assert.deepEqual(view.calls.at(-1), ["safeFocus", "password"]);
  for (const [key, name] of [["Backspace", "safeDelete"], ["Delete", "safeDelete"], ["Enter", "safeEnter"], ["Escape", "close"]]) {
    view.form.props.onKeyDown(keyEvent(key, { currentTarget: view.form })); assert.deepEqual(view.calls.at(-1), [name]);
  }
  const tools = all(view.tree).filter(node => node.props.className === "r16-safe-tool");
  tools.find(node => node.props.children === "重排").props.onClick(); assert.deepEqual(view.calls.at(-1), ["safeRandom"]);
  tools.find(node => node.props.children === "删除").props.onClick(); assert.deepEqual(view.calls.at(-1), ["safeDelete"]);
});

test("auxiliary pending gates and IME cannot dispatch while Escape can retire an auth surface", () => {
  for (const surface of ["registration", "changePassword", "safeKey"]) {
    const host = overlayHost(), view = auxView(host, surface, { pending: true });
    assert(view.fields.every(field => field.props.disabled)); assert.equal(view.submit.props.disabled, true);
    view.form.props.onSubmit(keyEvent("Enter"));
    for (const extra of [{ nativeEvent: { isComposing: true } }, { keyCode: 229 }]) {
      view.form.props.onKeyDown(keyEvent("Enter", { currentTarget: view.form, target: view.fields[0], ...extra }));
      view.form.props.onKeyDown(keyEvent("Escape", extra));
    }
    if (surface === "safeKey") for (const key of ["Enter", "Delete", "Backspace"]) view.form.props.onKeyDown(keyEvent(key));
    assert.deepEqual(view.calls, []);
    view.form.props.onKeyDown(keyEvent("Escape")); assert.deepEqual(view.calls, [["close"]]);
  }
});

test("actual login fields use R16 geometry and Enter advances focus without authenticating", () => {
  const host = overlayHost(); let sent = 0;
  const props = { ...loginProps(), onSubmitLogin() { sent++; } };
  const tree = host.render("LoginOverlay", props);
  const form = byClass(tree, "login-dialog");
  assert.equal(form.props.style.left, 348); assert.equal(form.props.style.top, 274);
  const fields = all(tree).filter(node => node.type === "input");
  const focused = [];
  const fakeForm = { querySelector(selector) { return { focus() { focused.push(selector); } }; } };
  for (const field of fields) {
    assert.equal(field.props.style.width, 136); assert.equal(field.props.style.height, 15);
    field.props.onKeyDown(keyEvent("Enter", { currentTarget: { form: fakeForm } }));
  }
  assert.deepEqual(focused, ['[data-r16-login-focus="password"]', '[data-r16-login-focus="login"]']);
  assert.equal(sent, 0);
  const composing = keyEvent("Enter", { nativeEvent: { isComposing: true }, currentTarget: { form: fakeForm } });
  fields[0].props.onKeyDown(composing); assert.equal(composing.prevented, false); assert.equal(focused.length, 2);
  form.props.onSubmit(keyEvent("Enter")); assert.equal(sent, 1);
  assert.equal(byClass(tree, "login-button").props.style.width, 42);
});

test("actual R16 login Tab order, F8 and Escape obey pending and modal gates", () => {
  const host = overlayHost(); let qr = 0;
  const props = { ...loginProps(), onDubheQrLogin() { qr++; } };
  let tree = host.render("LoginOverlay", props);
  const focused = [];
  tree.props.ref.current = { querySelector(selector) { return { focus() { focused.push(selector); } }; } };
  for (const [current, next] of [["accountId", "password"], ["password", "login"], ["login", "newAccount"], ["newAccount", "accountId"]]) {
    tree.props.onKeyDown(keyEvent("Tab", { target: { closest() { return { dataset: { r16LoginFocus: current } }; } } }));
    assert.equal(focused.at(-1), `[data-r16-login-focus="${next}"]`);
  }
  tree.props.onKeyDown(keyEvent("Tab", { shiftKey: true, target: { closest() { return { dataset: { r16LoginFocus: "accountId" } }; } } }));
  assert.equal(focused.at(-1), '[data-r16-login-focus="newAccount"]');
  tree.props.onKeyDown(keyEvent("F8")); assert.equal(qr, 1);
  tree.props.onKeyDown(keyEvent("Escape")); assert.equal(focused.at(-1), '[data-r16-login-focus="accountId"]');
  for (const changed of [{ loginBusy: true }, { loginAuth: { ...props.loginAuth, pending: true } },
    { loginAuth: { ...props.loginAuth, state: { ...props.loginAuth.state, surface: "registration" } } }, { dubheQr: { status: "pending" } }]) {
    tree = host.render("LoginOverlay", { ...props, ...changed }); tree.props.onKeyDown(keyEvent("F8"));
  }
  assert.equal(qr, 1);
});

function selectProps(characters = []) {
  return { language: "zh-CN", t: (_key, _params, fallback) => fallback ?? _key, characters, selectedCharacterIndex: 0,
    accountId: "Existing", identityProvider: null, identityLinkBusy: false, identityLinkStatus: null,
    suiWallets: [], selectedPortraitFrames: portraits.SELECT_PORTRAIT_ANIMATIONS.warriorMale,
    characterCreate: { status: "idle", classes: ["warrior", "wizard", "taoist", "assassin", "archer"], submit() {}, clearNotice() {} },
    onLanguageChange() {}, onExit() {}, onSelectCharacter() {} };
}
function authButton(tree, label) { return all(tree).find(node => node.type?.name === "SpriteButton" && node.props.label === label); }

test("actual select buttons and localized slot preserve native availability and opaque player names", () => {
  const host = overlayHost(); let started = 0;
  let tree = host.render("SelectOverlay", { ...selectProps([{ synthetic: true }]), onEnterWorld() { started++; } });
  assert.equal(authButton(tree, "开始").props.disabled, true);
  assert.equal(authButton(tree, "删除角色").props.disabled, true);
  tree.props.onKeyDown(keyEvent("Enter", { target: { closest() { return null; } } })); assert.equal(started, 0);
  const characters = Array.from({ length: 4 }, (_, index) => ({ index: index + 7, name: "Login 战士 {level}", classKey: "warrior", gender: "male", level: 18 }));
  tree = host.render("SelectOverlay", { ...selectProps(characters), onEnterWorld() { started++; } });
  assert.equal(authButton(tree, "开始").props.disabled, false);
  assert.equal(authButton(tree, "创建").props.disabled, true);
  assert.equal(byClass(tree, "name").props.children, characters[0].name);
  assert.equal(byClass(tree, "level").props.children, "等级 18");
  assert.equal(byClass(tree, "job").props.children, "战士");
  tree.props.onKeyDown(keyEvent("Enter", { target: { closest() { return null; } } })); assert.equal(started, 1);
  tree.props.onKeyDown(keyEvent("Enter", { target: { closest() { return {}; } } })); assert.equal(started, 1, "browser button click owns Enter once");
  assert.match(nativeSelect, /has_selection/);
});

test("actual create dialog uses R16 modal, 18-character field and distinct class/gender choices", () => {
  const host = overlayHost(); let created = null;
  const select = selectProps();
  const props = { ...select, onEnterWorld() {}, characterCreate: { ...select.characterCreate, submit(draft) { created = draft; } } };
  let tree = host.render("SelectOverlay", props); authButton(tree, "创建").props.onClick();
  tree = host.render("SelectOverlay", props);
  const form = byClass(tree, "select-create-panel");
  assert.equal(form.type, "form"); assert.equal(form.props.role, "dialog"); assert.equal(form.props["aria-modal"], "true");
  assert.deepEqual(form.props.style, layout.R16_AUTH_LAYOUT.create.panel);
  const input = all(form).find(node => node.type === "input");
  assert.equal(input.props.maxLength, 18); assert.equal(input.props.value, "");
  const classes = all(form).filter(node => node.props["data-r16-create-focus"] === "class");
  const genders = all(form).filter(node => node.props["data-r16-create-focus"] === "gender");
  assert.equal(classes.length, 5); assert.equal(genders.length, 2);
  for (const [index, button] of classes.entries()) assert.deepEqual(button.props.style, layout.r16AuthRelativeRect(layout.R16_AUTH_LAYOUT.create.classes[index], layout.R16_AUTH_LAYOUT.create.panel));
  input.props.onChange({ target: { value: "勇者 一二三四五六七八九十" } }); classes[4].props.onClick(); genders[1].props.onClick();
  tree = host.render("SelectOverlay", props);
  byClass(tree, "select-create-panel").props.onSubmit(keyEvent("Enter"));
  assert.deepEqual(created, { name: "勇者一二三四五六七八九十", classKey: "archer", gender: "female" });
});

test("character creation policy and success receipts match the R16 and classic Gateway contracts", () => {
  const realm = { schema: "mir2-realm-handshake/1", realmId: "test" };
  for (const id of ["platinum_176", "platinum_fiveclass"]) {
    const profile = JSON.parse(read(`../../../packages/game-data/data/content_profiles/${id}.json`));
    assert.deepEqual(characterCreate.characterCreateClasses({ ...realm, profileId: id, profileVersion: profile.version }),
      profile.allowedClasses.map(job => job.toLowerCase()));
  }
  assert.deepEqual(characterCreate.characterCreateClasses({ ...realm, profileId: "platinum_176", profileVersion: 24 }), ["warrior", "wizard", "taoist"]);
  assert.deepEqual(characterCreate.characterCreateClasses({ ...realm, profileId: "unknown", profileVersion: 1, allowedClasses: ["assassin"] }), []);
  assert.deepEqual(characterCreate.characterCreateClasses(null), []);
  const draft = { name: "PlayerOne", classKey: "wizard", gender: "female" };
  const reply = { index: 0, level: 0, name: draft.name, class: "Wizard", gender: "Female" };
  assert.equal(characterCreate.matchesCreatedCharacter(reply, draft), true);
  assert.equal(characterCreate.matchesCreatedCharacter({ ...reply, index: undefined }, draft), false);
  assert.equal(characterCreate.matchesCreatedCharacter({ ...reply, name: "Other" }, draft), false);
  assert.equal(characterCreate.matchesCreatedCharacter({ ...reply, class: "SomeWizard" }, draft), false);
  assert.equal(characterCreate.matchesCreatedCharacter({ ...reply, gender: "Male" }, draft), false);
});

test("actual create form holds the draft while pending and permits editing after a rejection", () => {
  const host = overlayHost(), props = selectProps();
  const submitted = [];
  props.characterCreate.submit = draft => submitted.push(draft);
  let tree = host.render("SelectOverlay", props);
  authButton(tree, "创建").props.onClick();
  const panel = () => byClass(host.render("SelectOverlay", props), "select-create-panel");
  const field = form => all(form).find(node => node.props["data-r16-create-focus"] === "name");
  let form = panel();
  field(form).props.onChange({ target: { value: "RetryOne" } });
  panel().props.onSubmit(keyEvent("Enter"));
  props.characterCreate.status = "pending";
  form = panel();
  assert.equal(form.props["aria-busy"], true);
  assert.equal(field(form).props.readOnly, true);
  assert.equal(field(form).props.value, "RetryOne");
  form.props.onSubmit(keyEvent("Enter"));
  assert.equal(submitted.length, 1);
  props.characterCreate.status = "failed";
  form = panel();
  assert.equal(field(form).props.readOnly, false);
  assert.equal(field(form).props.value, "RetryOne");
  assert(byClass(form, "select-create-error"));
  field(form).props.onChange({ target: { value: "RetryTwo" } });
  panel().props.onSubmit(keyEvent("Enter"));
  assert.deepEqual(submitted.map(draft => draft.name), ["RetryOne", "RetryTwo"]);
});

test("character creation shows shared name and slot notices in all ten languages", () => {
  for (const language of nativeLocalization.R16_NATIVE_LANGUAGES) {
    const host = overlayHost(), props = selectProps();
    props.language = language;
    props.t = (key, args, fallback) => {
      const message = nativeLocalization.nativeText(language, key, fallback);
      return (args ?? []).reduce((value, arg, index) => value.replaceAll(`{${index}}`, String(arg)), message);
    };
    let tree = host.render("SelectOverlay", props);
    byClass(tree, "new").props.children.props.onClick();
    const render = () => host.render("SelectOverlay", props);
    let form = byClass(render(), "select-create-panel");
    const name = all(form).find(node => node.props["data-r16-create-focus"] === "name");
    name.props.onChange({ target: { value: "Draft" } });
    props.characterCreate.status = "nameTaken";
    form = byClass(render(), "select-create-panel");
    const error = byClass(form, "select-create-error");
    assert.equal(error.props.children, nativeLocalization.nativeText(language, "client.CharacterNameExists"));
    assert.equal(all(form).find(node => node.props["data-r16-create-focus"] === "name").props.value, "Draft");

    props.characterCreate.status = "slotsFull";
    tree = render();
    // This renderer does not run effects. Execute the create-result effect,
    // then verify the same notice appears outside the dismissed form.
    host.hooks.effects.find(effect => effect.toString().includes('"slotsFull"'))();
    tree = render();
    assert.equal(byClass(tree, "select-create-panel"), undefined);
    const full = byClass(tree, "select-create-error").props.children;
    assert.equal(full, nativeLocalization.nativeText(language, "client.YouCannotMakeMoreCharacters").replace("{0}", "4"));
    assert(full.includes("4"));
  }
});

test("native caption rendering is opt-in and disabled controls cannot activate", () => {
  const host = overlayHost(); let activated = 0;
  const props = { sprite: originalUi.ORIGINAL_UI.login.buttons.ok, label: "确定", onClick() { activated++; } };
  const plain = host.render("SpriteButton", props);
  assert.equal(plain.props["data-r16-auth-button"], undefined);
  const disabled = host.render("SpriteButton", { ...props, nativeCaption: true, disabled: true });
  assert.equal(disabled.props.disabled, true); assert.equal(disabled.props["data-r16-auth-button"], true);
  disabled.props.onClick({ detail: 0 }); assert.equal(activated, 0);
  const enabled = host.render("SpriteButton", { ...props, nativeCaption: true });
  assert.equal(all(enabled).find(node => node.type === "span").props.children, "确定");
  enabled.props.onClick({ detail: 0 }); assert.equal(activated, 1);
});

test("auth language footer uses native geometry and all ten Windows choices, leaving the game default inline", () => {
  const nativeI18n = read(nativeRoot + "native_i18n.rs");
  const selector = nativeI18n.split("fn spawn_language_selector(")[1].split("fn show_language_selector(")[0];
  assert.match(selector, /bottom: px\(12\.0\)/); assert.match(selector, /right: px\(12\.0\)/);
  assert.match(selector, /width: px\(240\.0\)/);
  assert.match(nativeI18n, /bottom: px\(33\.0\)/);
  const host = overlayHost(), selected = [];
  const props = { language: "zh-CN", t: (_k, _p, f) => f, compact: true, onLanguageChange(value) { selected.push(value); } };
  let tree = host.render("LanguageSelector", props);
  assert.equal(all(tree).filter(node => node.type === "button").length, 10);
  assert.equal(byClass(tree, "r16-language-toggle"), undefined);
  tree = host.render("LanguageSelector", { ...props, nativeFooter: true });
  assert.deepEqual(tree.props.style, { ...layout.R16_AUTH_LAYOUT.footer.language, top: "auto" });
  assert.equal(all(tree).filter(node => node.type === "button").length, 1);
  byClass(tree, "r16-language-toggle").props.onClick();
  tree = host.render("LanguageSelector", { ...props, nativeFooter: true });
  const menu = byClass(tree, "r16-language-menu"); assert.equal(menu.props.style.bottom, 33);
  const choices = all(menu).filter(node => node.type === "button"); assert.equal(choices.length, 10);
  assert.deepEqual(choices.map(choice => choice.props.lang), nativeLocalization.R16_NATIVE_LANGUAGES);
  choices.find(choice => choice.props.lang === "zh-TW").props.onClick(); assert.deepEqual(selected, ["zh-TW"]);
  tree = host.render("LanguageSelector", { ...props, nativeFooter: true }); assert.equal(byClass(tree, "r16-language-menu"), undefined);
  byClass(tree, "r16-language-toggle").props.onClick(); tree = host.render("LanguageSelector", { ...props, nativeFooter: true });
  let focused = 0; byClass(tree, "r16-language-toggle").props.ref.current = { focus() { focused++; } };
  const event = keyEvent("Escape"); tree.props.onKeyDown(event); assert.equal(event.stopped, true); assert.equal(focused, 1);
});

test("browser-only login actions and diagnostics mount only inside the collapsed footer without losing callbacks", () => {
  const host = overlayHost(); let passkeys = 0, wallets = 0, quick = 0, security = 0;
  const props = { ...loginProps(), runtimePhase: "deferred", runtimeMessage: "Bevy runtime", wsState: "closed",
    onPasskeyLogin() { passkeys++; }, onWalletPickerToggle() { wallets++; }, onQuickEnter() { quick++; }, onOpenAccountSecurity() { security++; } };
  const tree = host.render("LoginOverlay", props);
  const footer = all(tree).find(node => node.type?.name === "AuthBrowserUtilities"); assert(footer);
  assert.equal(all(tree).filter(node => node.type?.name === "OriginalAudioSettingsControls").length, 0, "no always-mounted top-right audio");
  let view = host.renderComponent(footer.type, footer.props);
  assert.equal(byClass(view, "r16-auth-browser-panel"), undefined);
  assert.equal(byClass(view, "login-runtime-stamp"), undefined);
  byClass(view, "r16-auth-settings-trigger").props.onClick(); view = host.renderComponent(footer.type, footer.props);
  const audio = all(view).find(node => node.type?.name === "OriginalAudioSettingsControls"); assert.equal(audio.props.stacked, true);
  assert.equal(audio.props.language, "zh-CN");
  const controls = all(view).filter(node => node.type === "button");
  controls.find(node => node.props.onClick === props.onPasskeyLogin).props.onClick();
  controls.find(node => node.props.onClick === props.onWalletPickerToggle).props.onClick();
  controls.find(node => node.props.onClick === props.onQuickEnter).props.onClick();
  assert.deepEqual([passkeys, wallets, quick], [1, 1, 1]);
  const diagnostics = byClass(view, "r16-auth-diagnostics"); assert.equal(diagnostics.type, "details"); assert.equal(diagnostics.props.open, undefined);
  assert.equal(byClass(diagnostics, "login-runtime-stamp").props.children, "deferred / closed / Bevy runtime");
  controls.find(node => node.props.children === browserLocalization.browserText("zh-CN", "browser.accountSecurity")).props.onClick(); assert.equal(security, 1);
  view = host.renderComponent(footer.type, footer.props); assert.equal(byClass(view, "r16-auth-browser-panel"), undefined);
  byClass(view, "r16-auth-settings-trigger").props.onClick();
  view = host.renderComponent(footer.type, { ...footer.props, blocked: true });
  assert.equal(byClass(view, "r16-auth-browser-panel"), undefined); assert.equal(byClass(view, "r16-auth-settings-trigger").props.disabled, true);
});

test("select footer clears Start/Exit hit targets and Escape closes browser settings before logout", () => {
  const host = overlayHost(); let loggedOut = 0;
  const tree = host.render("SelectOverlay", { ...selectProps(), onExit() { loggedOut++; } });
  const group = byClass(tree, "r16-select-footer"); assert.equal(group.props.style.bottom, 36);
  for (const item of [layout.R16_AUTH_LAYOUT.footer.browser, layout.R16_AUTH_LAYOUT.footer.language]) {
    const bottom = 768 - group.props.style.bottom - item.bottom;
    assert(bottom < layout.R16_AUTH_LAYOUT.select.buttons.start.top, "footer ends above native select actions");
  }
  const footer = all(group).find(node => node.type?.name === "AuthBrowserUtilities");
  let view = host.renderComponent(footer.type, footer.props); byClass(view, "r16-auth-settings-trigger").props.onClick();
  view = host.renderComponent(footer.type, footer.props);
  const escape = keyEvent("Escape"); view.props.onKeyDown(escape); tree.props.onKeyDown(escape);
  assert.equal(loggedOut, 0); assert.equal(escape.stopped, true);
});

test("language popup closes on an outside capture event and releases its listener", () => {
  const host = overlayHost(), props = { language: "zh-CN", t: (_k, _p, f) => f, nativeFooter: true, onLanguageChange() {} };
  let tree = host.render("LanguageSelector", props); byClass(tree, "r16-language-toggle").props.onClick();
  tree = host.render("LanguageSelector", props);
  const inside = {};
  tree.props.ref.current = { contains(target) { return target === inside; } };
  const cleanup = host.hooks.effects[0]();
  assert.equal(host.listeners.length, 1); assert.equal(host.listeners[0].capture, true);
  host.listeners[0].callback({ target: inside });
  tree = host.render("LanguageSelector", props); assert(byClass(tree, "r16-language-menu"));
  host.listeners[0].callback({ target: {} });
  tree = host.render("LanguageSelector", props); assert.equal(byClass(tree, "r16-language-menu"), undefined);
  cleanup(); assert.equal(host.listeners.length, 0);
});

test("stacked auth audio bounds sliders and supplies missing Chinese labels without changing channel updates", () => {
  const hooks = hookHost(); let settings = { musicEnabled: true, effectsEnabled: true, musicVolume: .8, effectsVolume: .8 }, last;
  const audio = evaluate("../app/components/original-client-audio-settings.tsx", {
    react: hooks.react, "react/jsx-runtime": { jsx, jsxs: jsx, Fragment: "fragment" },
    "../../lib/original-audio": { getOriginalAudioSettings() { return settings; }, setOriginalAudioSettings(next) { last = next; return settings = { ...settings, ...next }; }, subscribeOriginalAudioSettings() {} },
  });
  hooks.reset("audio");
  const tree = audio.OriginalAudioSettingsControls({ t: (_k, _p, f) => f, language: "zh-CN", stacked: true });
  assert.equal(tree.props.style.gridTemplateColumns, "minmax(0, 1fr)");
  const sliders = all(tree).filter(node => node.type?.name === "AudioVolumeSlider");
  assert.deepEqual(sliders.map(node => node.props.label), ["总音量", "音乐音量", "音效音量"]);
  for (const slider of sliders) {
    const rendered = slider.type(slider.props); assert.equal(rendered.props.style.gridTemplateColumns, "90px minmax(0, 1fr) 34px");
    assert.equal(all(rendered).find(node => node.type === "input").props.style.maxWidth, "100%");
  }
  sliders[1].props.onChange(.3); assert.deepEqual(last, { musicVolume: .3 });
  sliders[0].props.onChange(.5); assert.deepEqual(last, { musicVolume: .5, effectsVolume: .5 });
});
