import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "资料库 · mir2 Codex",
  description: "传奇2(Crystal) 游戏资料库 — 玩家与开发者通用。",
};

const DOMAINS = [
  { href: "/codex/items", title: "物品", desc: "1628 件 · 属性 / 职业需求 / 掉落来源", ready: true },
  { href: "/codex/monsters", title: "怪物", desc: "555 只 · 属性 / 掉落表（规划中）", ready: false },
  { href: "/codex/magic", title: "技能 / 魔法", desc: "110 个 · 效果 / 等级（规划中）", ready: false },
  { href: "/codex/npc", title: "NPC / 商店", desc: "375 NPC · 105 商店（规划中）", ready: false },
];

export default function CodexHome() {
  return (
    <main
      style={{
        minHeight: "100vh",
        background: "var(--bg)",
        color: "var(--ink)",
        padding: "32px clamp(12px, 4vw, 48px)",
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <nav style={{ marginBottom: 16, fontSize: 13 }}>
        <a href="/" style={{ color: "var(--gold-dim)" }}>
          ← 返回游戏
        </a>
      </nav>
      <h1 style={{ margin: "0 0 6px", fontSize: 26, color: "var(--gold)" }}>资料库 · Codex</h1>
      <p style={{ margin: "0 0 24px", opacity: 0.6, fontSize: 14, maxWidth: 640 }}>
        传奇2(Crystal) 游戏数据库，玩家与开发者通用。数据 1:1 提取自 Crystal，开发者视图附带原始字段与解码出处。
      </p>
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))", gap: 14 }}>
        {DOMAINS.map((d) => {
          const card = (
            <div
              style={{
                border: "1px solid var(--line)",
                borderRadius: 10,
                padding: 18,
                background: "var(--panel)",
                opacity: d.ready ? 1 : 0.5,
                height: "100%",
              }}
            >
              <div style={{ display: "flex", alignItems: "baseline", gap: 8 }}>
                <h2 style={{ margin: 0, fontSize: 18, color: "var(--gold)" }}>{d.title}</h2>
                {!d.ready && <span style={{ fontSize: 11, opacity: 0.7 }}>规划中</span>}
              </div>
              <p style={{ margin: "8px 0 0", fontSize: 13, opacity: 0.7 }}>{d.desc}</p>
            </div>
          );
          return d.ready ? (
            <a key={d.href} href={d.href} style={{ textDecoration: "none", color: "inherit" }}>
              {card}
            </a>
          ) : (
            <div key={d.href}>{card}</div>
          );
        })}
      </div>
    </main>
  );
}
