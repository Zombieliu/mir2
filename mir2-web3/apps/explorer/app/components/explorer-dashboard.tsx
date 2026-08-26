"use client";

import { useEffect, useMemo, useRef, useState } from "react";

const searchEntries = [
  { type: "装备", name: "屠龙", meta: "传说武器 · 最近成交 128,000 金币", icon: "刃" },
  { type: "角色", name: "苍月孤狼", meta: "48级战士 · 沙巴克城主", icon: "战" },
  { type: "公会", name: "龙魂殿", meta: "182名成员 · 当前领主", icon: "盟" },
  { type: "地图", name: "赤月峡谷", meta: "推荐等级 40–50 · 极高危险", icon: "图" },
  { type: "怪物", name: "赤月恶魔", meta: "世界首领 · 下次窗口 21:30", icon: "魔" },
];

const activities = [
  { time: "刚刚", kind: "稀有掉落", title: "裁决之杖现世", detail: "苍月孤狼 · 祖玛教主", tone: "gold" },
  { time: "2分钟前", kind: "领地变更", title: "龙魂殿占领沙巴克", detail: "战役 #SBK-0826 · 2,431人参与", tone: "red" },
  { time: "7分钟前", kind: "市场成交", title: "麻痹戒指完成交易", detail: "成交价 96,800 金币 · 已验证", tone: "cyan" },
  { time: "12分钟前", kind: "角色成就", title: "清风道长达到50级", detail: "本世界第 37 位满级角色", tone: "green" },
];

const markets = [
  { name: "黑铁矿石", price: "1,284", change: "+12.8%", trend: "up" },
  { name: "祝福油", price: "8,620", change: "+4.2%", trend: "up" },
  { name: "祖玛头像", price: "21,400", change: "−3.1%", trend: "down" },
  { name: "裁决之杖", price: "68,900", change: "+18.4%", trend: "up" },
];

const mapZones = [
  { name: "沙巴克", players: 2431, risk: "战争中", x: "47%", y: "43%", level: "hot" },
  { name: "比奇省", players: 1286, risk: "安全", x: "28%", y: "31%", level: "safe" },
  { name: "盟重省", players: 864, risk: "活跃", x: "65%", y: "59%", level: "warm" },
  { name: "赤月峡谷", players: 319, risk: "极高", x: "77%", y: "27%", level: "danger" },
];

export function ExplorerDashboard() {
  const [query, setQuery] = useState("");
  const [activeView, setActiveView] = useState("总览");
  const searchInput = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchInput.current?.focus();
      }
      if (event.key === "Escape") {
        setQuery("");
        searchInput.current?.blur();
      }
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, []);

  const results = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return searchEntries;
    return searchEntries.filter((entry) => `${entry.type}${entry.name}${entry.meta}`.toLowerCase().includes(normalized));
  }, [query]);

  return (
    <main className="atlas-shell">
      <header className="topbar">
        <a className="atlas-brand" href="/zh-CN" aria-label="NUMERON ATLAS 首页">
          <span className="brand-mark"><i /><i /></span>
          <span><strong>NUMERON</strong><small>ATLAS · 天机阁</small></span>
        </a>
        <nav aria-label="主要导航">
          {["总览", "世界", "角色", "公会", "图鉴", "市场"].map((item) => (
            <button key={item} className={activeView === item ? "active" : ""} onClick={() => setActiveView(item)}>{item}</button>
          ))}
        </nav>
        <div className="realm-switch"><span className="live-dot" />玛法一号<small>WORLD 01</small><i>⌄</i></div>
      </header>

      <section className="explore-hero">
        <div className="hero-copy">
          <div className="overline"><span />THE WORLD REMEMBERS EVERYTHING</div>
          <h1>看见世界<br /><em>正在发生什么。</em></h1>
          <p>搜索角色、公会、装备、怪物、地图或公开事件。所有数据均为原型演示，不代表真实游戏状态。</p>
          <span className="active-view">当前：{activeView}数据视图</span>
        </div>
        <div className="world-clock" aria-label="世界状态在线">
          <div className="clock-rings"><i /><i /><i /><b>01</b></div>
          <span>WORLD PULSE</span>
          <strong>ONLINE</strong>
        </div>
      </section>

      <section className="search-stage" aria-label="全局搜索">
        <div className="search-box">
          <span className="search-glyph">⌕</span>
          <label htmlFor="atlas-search">探索传奇重生世界</label>
          <input
            id="atlas-search"
            ref={searchInput}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="输入角色、公会、装备、地图、事件编号…"
            autoComplete="off"
          />
          <kbd>⌘ K</kbd>
        </div>
        <div className={`search-results ${query ? "open" : ""}`}>
          <div className="result-head"><span>匹配结果</span><small>{results.length} RESULT{results.length === 1 ? "" : "S"}</small></div>
          {results.length > 0 ? results.map((entry) => (
            <button key={`${entry.type}-${entry.name}`} className="search-result" onClick={() => setQuery(entry.name)}>
              <i>{entry.icon}</i><span><small>{entry.type}</small><strong>{entry.name}</strong><em>{entry.meta}</em></span><b>↗</b>
            </button>
          )) : <p className="empty-result">世界档案中暂时没有找到相关记录。</p>}
        </div>
        <div className="quick-search"><span>热门搜索</span>{["屠龙", "沙巴克", "赤月恶魔", "龙魂殿"].map((item) => <button key={item} onClick={() => setQuery(item)}>{item}</button>)}</div>
      </section>

      <section className="pulse-strip" aria-label="世界实时指标">
        <article><small>在线冒险者</small><strong>12,684</strong><span className="positive">+8.4%</span></article>
        <article><small>活跃区域</small><strong>428</strong><span>/ 463</span></article>
        <article><small>今日公开事件</small><strong>8,912</strong><span className="positive">LIVE</span></article>
        <article><small>24H 市场成交</small><strong>₲ 48.6M</strong><span className="positive">+12.1%</span></article>
      </section>

      <section className="dashboard-grid">
        <article className="panel world-map-panel">
          <div className="panel-heading"><div><small>WORLD ACTIVITY</small><h2>玛法世界热度</h2></div><button>查看完整地图 ↗</button></div>
          <div className="atlas-map">
            <div className="map-contours"><i /><i /><i /><i /></div>
            <div className="map-route route-a" /><div className="map-route route-b" />
            {mapZones.map((zone) => (
              <button key={zone.name} className={`zone-point ${zone.level}`} style={{ left: zone.x, top: zone.y }}>
                <i /><span><strong>{zone.name}</strong><small>{zone.players.toLocaleString()}人 · {zone.risk}</small></span>
              </button>
            ))}
            <div className="map-legend"><span><i className="safe" />平稳</span><span><i className="warm" />活跃</span><span><i className="hot" />冲突</span></div>
          </div>
        </article>

        <article className="panel event-panel">
          <div className="panel-heading"><div><small>NEXT WORLD EVENT</small><h2>赤月恶魔</h2></div><span className="event-status">预测窗口</span></div>
          <div className="event-sigil"><i /><i /><b>魔</b></div>
          <div className="countdown"><span><strong>01</strong><small>小时</small></span><em>:</em><span><strong>24</strong><small>分钟</small></span><em>:</em><span><strong>08</strong><small>秒</small></span></div>
          <p>赤月峡谷 · 区域级提示<br />精确位置仅在游戏内向符合条件的玩家显示。</p>
          <button className="primary-action">查看事件档案 <span>↗</span></button>
        </article>

        <article className="panel activity-panel">
          <div className="panel-heading"><div><small>VERIFIED ACTIVITY</small><h2>世界正在发生</h2></div><button>全部记录</button></div>
          <div className="activity-list">
            {activities.map((activity) => (
              <button key={activity.title} className="activity-row">
                <i className={activity.tone}>{activity.kind.slice(0, 1)}</i>
                <span><small>{activity.time} · {activity.kind}</small><strong>{activity.title}</strong><em>{activity.detail}</em></span><b>↗</b>
              </button>
            ))}
          </div>
        </article>

        <article className="panel market-panel">
          <div className="panel-heading"><div><small>MARKET PULSE</small><h2>市场热度</h2></div><button>市场总览 ↗</button></div>
          <div className="market-chart" aria-hidden="true"><i /><i /><i /><i /><i /><i /><i /><i /><i /><i /><span>24H VOLUME</span><strong>₲ 48,619,204</strong></div>
          <div className="market-table">
            <div className="table-head"><span>物品</span><span>地板价</span><span>24H</span></div>
            {markets.map((item) => <button key={item.name}><strong>{item.name}</strong><span>₲ {item.price}</span><em className={item.trend}>{item.change}</em></button>)}
          </div>
        </article>

        <article className="panel chronicle-panel">
          <div className="battle-art"><span>SABUK</span><strong>沙巴克之夜</strong><small>WORLD EVENT · #SBK-0826</small></div>
          <div className="battle-copy"><span className="verified">✓ 已验证战报</span><h2>龙魂殿首次夺取沙巴克</h2><p>2,431 名玩家参与，战役持续 01:46:32。最后一道城门于 22:47 被攻破。</p><div><span><small>参战公会</small><strong>18</strong></span><span><small>公开击杀</small><strong>7,294</strong></span><span><small>领主</small><strong>苍月孤狼</strong></span></div><button>回放世界事件 <i>▶</i></button></div>
        </article>
      </section>

      <footer><div><strong>NUMERON ATLAS</strong><span>传奇重生世界公开档案</span></div><p>原型数据 · 非实时服务 · 敏感位置与私人资产不会公开</p><span>WORLD 01 / 2026</span></footer>
    </main>
  );
}
