// Isolated UI fixture. It exercises the real component, without a game login or gateway.
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import ts from "typescript";

const require = createRequire(import.meta.url);
const root = packageName => dirname(require.resolve(`${packageName}/package.json`));
const modules = new Map([
  ["react", readFileSync(join(root("react"), "cjs/react.production.js"), "utf8")],
  ["react/jsx-runtime", readFileSync(join(root("react"), "cjs/react-jsx-runtime.production.js"), "utf8")],
  ["react-dom", readFileSync(join(root("react-dom"), "cjs/react-dom.production.js"), "utf8")],
  ["react-dom/client", readFileSync(join(root("react-dom"), "cjs/react-dom-client.production.js"), "utf8")],
  ["scheduler", readFileSync(join(root("scheduler"), "cjs/scheduler.production.js"), "utf8")],
  ["controls", ts.transpileModule(readFileSync(new URL("../app/components/original-client-combat-mode-controls.tsx", import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
  }).outputText],
]);
const bundle = `const modules={${[...modules].map(([key, source]) => `${JSON.stringify(key)}:(require,module,exports)=>{${source}\n}`).join(",")}};
const cache={}; function require(key){if(cache[key])return cache[key].exports;
const module=cache[key]={exports:{}};modules[key](require,module,module.exports);return module.exports;}
const React=require("react"),h=React.createElement;
const Controls=require("controls").OriginalClientCombatModeControls;
function Fixture(){
 const [mode,setMode]=React.useState(0),[requested,setRequested]=React.useState(null),[safe,setSafe]=React.useState(true);
 const [enabled,setEnabled]=React.useState(true),[worldEvents,setWorldEvents]=React.useState(0),[requests,setRequests]=React.useState(0);
 return h(React.Fragment,null,
  h("div",{className:"fixture-tools"},
   h("strong",null,"攻击模式 · 独立组件验收"),
   h("button",{onClick:()=>setSafe(value=>!value)},safe?"切到战斗区":"切到安全区"),
   h("button",{onClick:()=>setEnabled(value=>!value)},enabled?"禁用控件":"启用控件"),
   h("button",{onClick:()=>{setMode(null);setRequested(null)}},"模拟同步中"),
   h("button",{onClick:()=>{if(requested!==null){setMode(requested);setRequested(null)}}},"确认服务器模式"),
   h("output",null,"请求数："+requests+" · 待确认："+(requested??"无")+" · 场景事件："+worldEvents)),
  h("div",{className:"client-stage-frame",onPointerDownCapture:event=>{
   if(!event.target.closest("[data-ui-interactive='true']"))setWorldEvents(value=>value+1);
  },onMouseDown:()=>setWorldEvents(value=>value+1)},
   h("div",{className:"fixture-map"},"本地 UI 验证场景"),
   h(Controls,{mode,shortcut:"Ctrl + H",enabled,inSafeZone:safe,onSelectMode:value=>{setRequested(value);setRequests(n=>n+1)}}),
   h("div",{className:"fixture-objective"},"任务追踪区域"),
   h("div",{className:"fixture-minimap"},"小地图区域"),
   h("div",{className:"fixture-hud"},"HUD 与技能栏区域")));
}
require("react-dom/client").createRoot(document.getElementById("root")).render(h(Fixture));`;
const html = `<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>Combat mode controls fixture</title>
<style>body{margin:0;background:#080807;color:#d9c49e;font:14px Arial,sans-serif}.fixture-tools{display:flex;align-items:center;gap:10px;min-height:54px;padding:0 8px}.fixture-tools button{padding:5px 8px}.fixture-tools output{font-size:12px}.client-stage-frame{position:relative;width:1024px;height:768px;overflow:hidden;background:#252922;box-shadow:inset 0 0 150px #080b06}.fixture-map{position:absolute;inset:0;display:grid;place-items:center;color:#6e7869;background:repeating-linear-gradient(20deg,transparent 0 40px,#0001 41px 43px)}.fixture-objective{position:absolute;left:40%;top:12px;color:#777e68;font-size:12px}.fixture-minimap{position:absolute;right:0;top:0;width:126px;height:152px;border:1px solid #655139;display:grid;place-items:center;color:#8d927d}.fixture-hud{position:absolute;bottom:0;height:135px;left:0;right:0;border-top:4px solid #655139;background:#1a1612;display:grid;place-items:center;color:#a59676}</style>
<div id="root"></div><script>${bundle.replaceAll("</script", "<\\/script")}</script></html>`;
const port = Number(process.env.MIR2_COMBAT_FIXTURE_PORT ?? 4263);
createServer((_request, response) => {
  response.writeHead(200, { "Content-Type": "text/html; charset=utf-8", "Cache-Control": "no-store" });
  response.end(html);
}).listen(port, "127.0.0.1", () => process.stdout.write(`Combat mode UI fixture: http://127.0.0.1:${port}\n`));
