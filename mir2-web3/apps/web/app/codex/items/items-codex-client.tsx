"use client";

import { useEffect, useMemo, useState } from "react";

// Shape of /codex/items.json (built by scripts/codex/build-item-codex.mjs).
type Stat = { id: number; label: string; value: number };
type DropSource = {
  monster: string;
  monsterLevel: number;
  isBoss: boolean;
  chanceRaw: string | null;
  chance: number | null;
};
type CodexItem = {
  index: number;
  name: string;
  type: string;
  typeId: number;
  grade: string;
  gradeId: number;
  requirement: { type: string; amount: number } | null;
  classes: string[];
  genders: string[];
  stats: Stat[];
  price: number;
  weight: number;
  durability: number;
  stackSize: number;
  shape: number;
  image: number;
  light: number;
  bind: number;
  unique: number;
  slots: number;
  needIdentify: boolean;
  canMine: boolean;
  canAwakening: boolean;
  randomStatsId: number;
  droppedBy: DropSource[];
  droppedByCount: number;
  raw: Record<string, unknown>;
};
type CodexFile = {
  totalItems: number;
  itemsWithDropSource: number;
  crystalDbVersion: number | null;
  types: string[];
  grades: string[];
  decodeSources: Record<string, string>;
  generatedFrom: Record<string, string>;
  items: CodexItem[];
};

const GRADE_COLOR: Record<string, string> = {
  None: "#cdbf9e",
  Common: "#e8dbc2",
  Rare: "#5aa9e6",
  Legendary: "#c77dff",
  Mythical: "#ffb454",
  Heroic: "#ff6b6b",
};

const MAX_RESULTS = 400;

export function ItemsCodexClient() {
  const [data, setData] = useState<CodexFile | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [typeFilter, setTypeFilter] = useState("");
  const [gradeFilter, setGradeFilter] = useState("");
  const [onlyDroppable, setOnlyDroppable] = useState(false);
  const [selected, setSelected] = useState<CodexItem | null>(null);
  const [devMode, setDevMode] = useState(false);

  useEffect(() => {
    let alive = true;
    fetch("/codex/items.json")
      .then((r) => {
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return r.json();
      })
      .then((d: CodexFile) => {
        if (alive) setData(d);
      })
      .catch((e) => alive && setError(String(e)));
    return () => {
      alive = false;
    };
  }, []);

  const filtered = useMemo(() => {
    if (!data) return [];
    const q = query.trim().toLowerCase();
    return data.items.filter((it) => {
      if (typeFilter && it.type !== typeFilter) return false;
      if (gradeFilter && it.grade !== gradeFilter) return false;
      if (onlyDroppable && it.droppedByCount === 0) return false;
      if (q && !it.name.toLowerCase().includes(q) && String(it.index) !== q) return false;
      return true;
    });
  }, [data, query, typeFilter, gradeFilter, onlyDroppable]);

  if (error) {
    return (
      <Shell>
        <p style={{ color: "var(--danger)" }}>
          载入物品资料库失败：{error}
          <br />
          请先运行 <code>node scripts/codex/build-item-codex.mjs</code> 生成数据。
        </p>
      </Shell>
    );
  }
  if (!data) {
    return (
      <Shell>
        <p style={{ opacity: 0.7 }}>正在载入 Crystal 物品数据…</p>
      </Shell>
    );
  }

  const shown = filtered.slice(0, MAX_RESULTS);

  return (
    <Shell>
      <header style={{ marginBottom: 14 }}>
        <div style={{ display: "flex", alignItems: "baseline", gap: 12, flexWrap: "wrap" }}>
          <h1 style={{ margin: 0, fontSize: 22, color: "var(--gold)" }}>物品资料库</h1>
          <span style={{ opacity: 0.6, fontSize: 13 }}>
            Item Codex · {data.totalItems} 件 · {data.itemsWithDropSource} 件有掉落来源
            {data.crystalDbVersion != null ? ` · DB v${data.crystalDbVersion}` : ""}
          </span>
          <label style={{ marginLeft: "auto", fontSize: 13, cursor: "pointer", userSelect: "none" }}>
            <input
              type="checkbox"
              checked={devMode}
              onChange={(e) => setDevMode(e.target.checked)}
              style={{ marginRight: 6 }}
            />
            开发者视图
          </label>
        </div>
        <p style={{ margin: "6px 0 0", fontSize: 12, opacity: 0.55 }}>
          数据源自 Crystal 提取的 manifest（1:1 权威）。物品名为 Crystal 内部名（暂无中文翻译表）。
        </p>
      </header>

      <div style={{ display: "flex", gap: 8, flexWrap: "wrap", marginBottom: 12 }}>
        <input
          placeholder="搜索物品名 / 编号…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={inputStyle}
        />
        <select value={typeFilter} onChange={(e) => setTypeFilter(e.target.value)} style={inputStyle}>
          <option value="">全部类型</option>
          {data.types.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
        <select value={gradeFilter} onChange={(e) => setGradeFilter(e.target.value)} style={inputStyle}>
          <option value="">全部品质</option>
          {data.grades.map((g) => (
            <option key={g} value={g}>
              {g}
            </option>
          ))}
        </select>
        <label style={{ display: "flex", alignItems: "center", fontSize: 13, gap: 6 }}>
          <input type="checkbox" checked={onlyDroppable} onChange={(e) => setOnlyDroppable(e.target.checked)} />
          仅显示可掉落
        </label>
        <span style={{ alignSelf: "center", opacity: 0.6, fontSize: 12 }}>
          {filtered.length} 条匹配{filtered.length > MAX_RESULTS ? `（显示前 ${MAX_RESULTS}）` : ""}
        </span>
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "minmax(0,1fr) minmax(0,1.1fr)", gap: 16 }}>
        <ul style={{ listStyle: "none", margin: 0, padding: 0, maxHeight: "72vh", overflowY: "auto" }}>
          {shown.map((it) => (
            <li key={it.index}>
              <button
                onClick={() => setSelected(it)}
                style={{
                  ...rowStyle,
                  borderColor: selected?.index === it.index ? "var(--gold)" : "var(--line)",
                  background: selected?.index === it.index ? "var(--panel-strong)" : "var(--panel)",
                }}
              >
                <span style={{ color: GRADE_COLOR[it.grade] ?? "var(--ink)", fontWeight: 600 }}>{it.name}</span>
                <span style={{ opacity: 0.55, fontSize: 12 }}>
                  {it.type}
                  {it.requirement ? ` · ${it.requirement.type} ${it.requirement.amount}` : ""}
                  {it.droppedByCount > 0 ? ` · 掉落×${it.droppedByCount}` : ""}
                </span>
              </button>
            </li>
          ))}
          {shown.length === 0 && <li style={{ opacity: 0.6, padding: 12 }}>无匹配结果</li>}
        </ul>

        <div style={{ maxHeight: "72vh", overflowY: "auto" }}>
          {selected ? (
            <ItemDetail item={selected} devMode={devMode} decodeSources={data.decodeSources} />
          ) : (
            <div style={{ opacity: 0.5, padding: 24, border: "1px dashed var(--line)", borderRadius: 8 }}>
              从左侧选择一件物品查看详情。
            </div>
          )}
        </div>
      </div>
    </Shell>
  );
}

function ItemDetail({
  item,
  devMode,
  decodeSources,
}: {
  item: CodexItem;
  devMode: boolean;
  decodeSources: Record<string, string>;
}) {
  return (
    <div style={{ border: "1px solid var(--line)", borderRadius: 8, padding: 16, background: "var(--panel)" }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 10, flexWrap: "wrap" }}>
        <h2 style={{ margin: 0, fontSize: 20, color: GRADE_COLOR[item.grade] ?? "var(--gold)" }}>{item.name}</h2>
        <span style={{ opacity: 0.6, fontSize: 12 }}>#{item.index}</span>
        <Badge>{item.type}</Badge>
        <Badge>{item.grade}</Badge>
      </div>

      <dl style={dlStyle}>
        {item.requirement && (
          <Row k="装备需求">
            {item.requirement.type} ≥ {item.requirement.amount}
          </Row>
        )}
        <Row k="职业">{item.classes.length ? item.classes.join(" / ") : "无限制"}</Row>
        {item.genders.length > 0 && <Row k="性别">{item.genders.join(" / ")}</Row>}
        <Row k="价格">{item.price.toLocaleString()} 金</Row>
        <Row k="重量">{item.weight}</Row>
        {item.durability > 0 && <Row k="耐久">{item.durability}</Row>}
        {item.stackSize > 1 && <Row k="堆叠">{item.stackSize}</Row>}
        {item.slots > 0 && <Row k="镶孔">{item.slots}</Row>}
      </dl>

      {item.stats.length > 0 && (
        <section style={{ marginTop: 8 }}>
          <h3 style={h3Style}>属性</h3>
          <ul style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexWrap: "wrap", gap: 6 }}>
            {item.stats.map((s, i) => (
              <li key={i} style={statChip}>
                <span style={{ opacity: 0.7 }}>{s.label}</span>{" "}
                <strong style={{ color: s.value < 0 ? "var(--danger)" : "var(--gold)" }}>
                  {s.value > 0 ? `+${s.value}` : s.value}
                </strong>
              </li>
            ))}
          </ul>
        </section>
      )}

      {item.droppedBy.length > 0 && (
        <section style={{ marginTop: 12 }}>
          <h3 style={h3Style}>掉落来源（{item.droppedByCount}）</h3>
          <ul style={{ margin: 0, padding: 0, listStyle: "none", display: "grid", gap: 4 }}>
            {item.droppedBy.slice(0, 40).map((d, i) => (
              <li key={i} style={dropRow}>
                <span style={{ color: d.isBoss ? "var(--gold)" : "var(--ink)" }}>
                  {d.monster}
                  {d.isBoss ? " 👑" : ""}
                </span>
                <span style={{ opacity: 0.6, fontSize: 12 }}>
                  {d.monsterLevel > 0 ? `Lv${d.monsterLevel} · ` : ""}
                  {d.chanceRaw ?? "?"}
                </span>
              </li>
            ))}
            {item.droppedBy.length > 40 && (
              <li style={{ opacity: 0.5, fontSize: 12 }}>…另有 {item.droppedBy.length - 40} 个来源</li>
            )}
          </ul>
        </section>
      )}

      {devMode && (
        <section style={{ marginTop: 14, borderTop: "1px solid var(--line)", paddingTop: 12 }}>
          <h3 style={h3Style}>开发者 · Crystal 原始字段</h3>
          <div style={{ fontSize: 11, opacity: 0.6, marginBottom: 6 }}>
            typeId={item.typeId} · gradeId={item.gradeId} · shape={item.shape} · image={item.image} · bind=
            {item.bind} · randomStatsId={item.randomStatsId}
          </div>
          <pre style={preStyle}>{JSON.stringify(item.raw, null, 2)}</pre>
          <h3 style={{ ...h3Style, marginTop: 10 }}>解码出处（权威 = Rust 镜像）</h3>
          <ul style={{ margin: 0, paddingLeft: 16, fontSize: 11, opacity: 0.7 }}>
            {Object.entries(decodeSources).map(([k, v]) => (
              <li key={k}>
                <code>{k}</code> → {v}
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <main
      style={{
        minHeight: "100vh",
        background: "var(--bg)",
        color: "var(--ink)",
        padding: "20px clamp(12px, 4vw, 48px)",
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <nav style={{ marginBottom: 12, fontSize: 13 }}>
        <a href="/" style={{ color: "var(--gold-dim)" }}>
          ← 返回游戏
        </a>
        <span style={{ opacity: 0.3, margin: "0 8px" }}>/</span>
        <span style={{ opacity: 0.7 }}>资料库 · 物品</span>
      </nav>
      {children}
    </main>
  );
}

const inputStyle: React.CSSProperties = {
  background: "var(--panel)",
  color: "var(--ink)",
  border: "1px solid var(--line)",
  borderRadius: 6,
  padding: "6px 10px",
  fontSize: 14,
};
const rowStyle: React.CSSProperties = {
  width: "100%",
  textAlign: "left",
  display: "flex",
  justifyContent: "space-between",
  alignItems: "center",
  gap: 10,
  border: "1px solid var(--line)",
  borderRadius: 6,
  padding: "7px 10px",
  marginBottom: 4,
  cursor: "pointer",
  color: "var(--ink)",
};
const dlStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "auto 1fr",
  columnGap: 12,
  rowGap: 4,
  margin: "12px 0 0",
  fontSize: 14,
};
const h3Style: React.CSSProperties = { margin: "0 0 6px", fontSize: 13, color: "var(--gold-dim)", letterSpacing: 1 };
const statChip: React.CSSProperties = {
  border: "1px solid var(--line)",
  borderRadius: 4,
  padding: "3px 8px",
  fontSize: 13,
};
const dropRow: React.CSSProperties = { display: "flex", justifyContent: "space-between", gap: 10, fontSize: 13 };
const preStyle: React.CSSProperties = {
  background: "#0c0a07",
  border: "1px solid var(--line)",
  borderRadius: 6,
  padding: 10,
  fontSize: 11,
  maxHeight: 280,
  overflow: "auto",
  margin: 0,
};

function Badge({ children }: { children: React.ReactNode }) {
  return (
    <span style={{ border: "1px solid var(--line)", borderRadius: 4, padding: "1px 7px", fontSize: 12, opacity: 0.8 }}>
      {children}
    </span>
  );
}
function Row({ k, children }: { k: string; children: React.ReactNode }) {
  return (
    <>
      <dt style={{ opacity: 0.55 }}>{k}</dt>
      <dd style={{ margin: 0 }}>{children}</dd>
    </>
  );
}
