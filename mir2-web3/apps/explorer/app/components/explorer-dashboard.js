"use client";
// The deployed public presentation was reconstructed from its public runtime module.
// Source API truncation and preservation limits are documented in RECOVERY-PROVENANCE.json.
import * as a from "react/jsx-runtime";
import * as i from "react";
import { AtlasLink as Link } from "./atlas-link";
import { AtlasFooter } from "./atlas-footer";
import { displayCopy, getDisplayMetrics } from "../../lib/atlas-display.mjs";
import {
  locales as n,
  copy as r,
  archiveHref as o,
  searchRecords as c,
  ledgerRecords as u,
  trendRecords as m,
  marketRows as h,
  mapPoints as g,
  requireArchiveRecord as v,
} from "../../lib/atlas-data.mjs";
const l = { default: Link };
export const ExplorerDashboard = function ({ locale: e }) {
  let s = r[e],
    [t, d] = (0, i.useState)(""),
    [p, b] = (0, i.useState)(0),
    [j, x] = (0, i.useState)(0),
    k = (0, i.useRef)(null),
    y = (0, i.useMemo)(
      () =>
        c.map(([a, l]) => {
          let i = v(e, a, l);
          return {
            type: i.kindLabel,
            name: i.name,
            meta: i.summary,
            icon: i.emblem,
            href: o(i, e),
          };
        }),
      [e],
    ),
    w = (0, i.useMemo)(() => {
      let a = t.trim().toLocaleLowerCase(e);
      return a
        ? y.filter((l) =>
            `${l.type}${l.name}${l.meta}`.toLocaleLowerCase(e).includes(a),
          )
        : y;
    }, [e, t, y]),
    N = (0, i.useMemo)(
      () =>
        m.map((a) =>
          a.map(([a, l], i) => {
            let n = v(e, a, l);
            return {
              rank: String(i + 1).padStart(2, "0"),
              name: n.name,
              meta: n.summary,
              value: 0 === i ? "\u2014" : 1 === i ? "\u2014" : "\u2014",
              href: o(n, e),
            };
          }),
        ),
      [e],
    );
  (0, i.useEffect)(() => {
    let e = (e) => {
      (e.metaKey || e.ctrlKey) &&
        "k" === e.key.toLowerCase() &&
        (e.preventDefault(), k.current?.focus()),
        "Escape" === e.key && (d(""), k.current?.blur());
    };
    return (
      window.addEventListener("keydown", e),
      () => window.removeEventListener("keydown", e)
    );
  }, []);
  let f = v(e, "world", "world-01"),
    R = v(e, "monster", "red-moon-demon"),
    E = v(e, "event", "sabuk-0826");
  return (0, a.jsxs)("main", {
    className: "atlas-shell",
    lang: e,
    dir: s.dir,
    children: [
      (0, a.jsxs)("header", {
        className: "topbar",
        children: [
          (0, a.jsxs)(l.default, {
            className: "atlas-brand",
            href: `/${e}`,
            "aria-label": `NUMERON ATLAS \xb7 ${s.nav[0]}`,
            children: [
              (0, a.jsxs)("span", {
                className: "brand-mark",
                children: [(0, a.jsx)("i", {}), (0, a.jsx)("i", {})],
              }),
              (0, a.jsxs)("span", {
                children: [
                  (0, a.jsx)("strong", { children: "NUMERON" }),
                  (0, a.jsx)("small", { children: "ATLAS" }),
                ],
              }),
            ],
          }),
          (0, a.jsx)("nav", {
            "aria-label": s.nav[0],
            children: s.nav.map((e, l) =>
              (0, a.jsx)(
                "button",
                {
                  type: "button",
                  className: p === l ? "active" : "",
                  "aria-pressed": p === l,
                  onClick: () => {
                    let e;
                    return (
                      b(l),
                      2 === l && x(2),
                      3 === l && x(1),
                      4 === l && x(0),
                      (e = [
                        "atlas-search-stage",
                        "world-activity",
                        "world-trends",
                        "world-trends",
                        "world-trends",
                        "market-pulse",
                      ]),
                      void window.requestAnimationFrame(() =>
                        document
                          .getElementById(e[l])
                          ?.scrollIntoView({
                            behavior: "smooth",
                            block: "start",
                          }),
                      )
                    );
                  },
                  children: e,
                },
                e,
              ),
            ),
          }),
          (0, a.jsxs)("details", {
            className: "atlas-locale",
            children: [
              (0, a.jsx)("summary", { children: s.short }),
              (0, a.jsx)("div", {
                children: n.map((i) =>
                  (0, a.jsx)(
                    l.default,
                    {
                      href: `/${i}`,
                      lang: i,
                      "aria-current": i === e ? "page" : void 0,
                      children: r[i].name,
                    },
                    i,
                  ),
                ),
              }),
            ],
          }),
          (0, a.jsxs)("div", {
            className: "realm-switch",
            children: [
              (0, a.jsx)("span", { className: "prototype-dot" }),
              "NUMERON",
              (0, a.jsx)("small", { children: "WORLD 01" }),
              (0, a.jsx)("i", { children: "⌄" }),
            ],
          }),
        ],
      }),
      (0, a.jsxs)("section", {
        className: "explore-hero",
        children: [
          (0, a.jsxs)("div", {
            className: "hero-copy",
            children: [
              (0, a.jsxs)("div", {
                className: "overline",
                children: [(0, a.jsx)("span", {}), s.hero[0]],
              }),
              (0, a.jsxs)("h1", {
                children: [
                  s.hero[1],
                  (0, a.jsx)("br", {}),
                  (0, a.jsx)("em", { children: s.hero[2] }),
                ],
              }),
              (0, a.jsx)("p", { children: s.hero[3] }),
              (0, a.jsxs)("span", {
                className: "active-view",
                children: [s.hero[4], ": ", s.nav[p], " ", s.hero[5]],
              }),
            ],
          }),
          (0, a.jsxs)("div", {
            className: "world-clock",
            "aria-label": s.ui[12],
            children: [
              (0, a.jsxs)("div", {
                className: "clock-rings",
                children: [
                  (0, a.jsx)("i", {}),
                  (0, a.jsx)("i", {}),
                  (0, a.jsx)("i", {}),
                  (0, a.jsx)("b", { children: "\u2014" }),
                ],
              }),
              (0, a.jsx)("span", { children: "WORLD PULSE" }),
              (0, a.jsx)("strong", { children: s.record[1] }),
            ],
          }),
        ],
      }),
      (0, a.jsxs)("section", {
        className: "search-stage",
        id: "atlas-search-stage",
        "aria-label": s.ui[0],
        children: [
          (0, a.jsxs)("div", {
            className: "search-box",
            children: [
              (0, a.jsx)("span", { className: "search-glyph", children: "⌕" }),
              (0, a.jsx)("label", {
                htmlFor: "atlas-search",
                children: s.ui[0],
              }),
              (0, a.jsx)("input", {
                id: "atlas-search",
                ref: k,
                value: t,
                onChange: (e) => d(e.target.value),
                placeholder: s.ui[1],
                autoComplete: "off",
              }),
              (0, a.jsx)("kbd", { children: "⌘ K" }),
            ],
          }),
          (0, a.jsxs)("div", {
            className: `search-results ${t ? "open" : ""}`,
            children: [
              (0, a.jsxs)("div", {
                className: "result-head",
                children: [
                  (0, a.jsx)("span", { children: s.ui[2] }),
                  (0, a.jsx)("small", { children: w.length }),
                ],
              }),
              w.length
                ? w.map((e) =>
                    (0, a.jsxs)(
                      l.default,
                      {
                        className: "search-result",
                        href: e.href,
                        onClick: () => d(""),
                        children: [
                          (0, a.jsx)("i", { children: e.icon }),
                          (0, a.jsxs)("span", {
                            children: [
                              (0, a.jsx)("small", { children: e.type }),
                              (0, a.jsx)("strong", { children: e.name }),
                              (0, a.jsx)("em", { children: e.meta }),
                            ],
                          }),
                          (0, a.jsx)("b", { children: "↗" }),
                        ],
                      },
                      `${e.type}-${e.name}`,
                    ),
                  )
                : (0, a.jsx)("p", {
                    className: "empty-result",
                    children: s.ui[3],
                  }),
            ],
          }),
          (0, a.jsxs)("div", {
            className: "quick-search",
            children: [
              (0, a.jsx)("span", { children: s.ui[4] }),
              y
                .slice(0, 4)
                .map((e) =>
                  (0, a.jsx)(
                    l.default,
                    { href: e.href, children: e.name },
                    e.name,
                  ),
                ),
            ],
          }),
        ],
      }),
      (0, a.jsx)("section", {
        className: "pulse-strip",
        "aria-label": s.ui[13],
        "data-data-state": "prototype",
        children: getDisplayMetrics(e, s.ui[20]).map(metric => (0, a.jsxs)("article", {
          "data-metric-id": metric.id,
          children: [(0,a.jsx)("small", {children:metric.label}), (0,a.jsx)("strong", {children:metric.value}), (0,a.jsx)("span", {children:metric.note})],
        }, metric.id)),
      }),
      (0, a.jsxs)("section", {
        className: "dashboard-grid",
        children: [
          (0, a.jsxs)("article", {
            className: "panel era-panel",
            "aria-label": s.ui[9],
            children: [
              (0, a.jsxs)("div", {
                className: "era-identity",
                children: [
                  (0, a.jsxs)("span", {
                    className: "era-index",
                    children: [
                      "ERA",
                      (0, a.jsx)("br", {}),
                      (0, a.jsx)("strong", { children: "\u2014" }),
                    ],
                  }),
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("small", { children: "WORLD PREVIEW" }),
                      (0, a.jsxs)("h2", { children: [s.ui[9], ""] }),
                      (0, a.jsx)("p", { children: s.record[3] }),
                    ],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "era-progress",
                children: [
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("span", { children: s.ui[10] }),
                      (0, a.jsx)("strong", { children: "—" }),
                    ],
                  }),
                  (0, a.jsx)("i", { children: (0, a.jsx)("b", { style: { width: 0 } }) }),
                  (0, a.jsxs)("small", { children: [s.ui[11], "\u2014"] }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "era-facts",
                children: [
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("small", { children: s.ui[10] }),
                      (0, a.jsx)("strong", { children: "\u2014" }),
                    ],
                  }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("small", { children: s.ui[11] }),
                      (0, a.jsx)("strong", { children: "—" }),
                    ],
                  }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("small", { children: s.ui[12] }),
                      (0, a.jsx)("strong", { children: "\u2014" }),
                    ],
                  }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("small", { children: s.ui[12] }),
                      (0, a.jsxs)("strong", {
                        className: "online-label",
                        children: [(0, a.jsx)("i", {}), s.record[1]],
                      }),
                    ],
                  }),
                ],
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel world-map-panel",
            id: "world-activity",
            children: [
              (0, a.jsxs)("div", {
                className: "panel-heading",
                children: [
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("small", { children: "WORLD ACTIVITY" }),
                      (0, a.jsx)("h2", { children: s.ui[13] }),
                    ],
                  }),
                  (0, a.jsxs)(l.default, {
                    href: o(f, e),
                    children: [s.ui[14], " ↗"],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "atlas-map",
                children: [
                  (0, a.jsxs)("div", {
                    className: "map-contours",
                    children: [
                      (0, a.jsx)("i", {}),
                      (0, a.jsx)("i", {}),
                      (0, a.jsx)("i", {}),
                      (0, a.jsx)("i", {}),
                    ],
                  }),
                  (0, a.jsx)("div", { className: "map-route route-a" }),
                  (0, a.jsx)("div", { className: "map-route route-b" }),
                  g.map(([i, n, r, t, d, c]) => {
                    let u = v(e, i, n);
                    return (0, a.jsxs)(
                      l.default,
                      {
                        href: o(u, e),
                        "aria-label": `${s.ui[14]} ${u.name}`,
                        className: `zone-point ${c}`,
                        style: { left: t, top: d },
                        children: [
                          (0, a.jsx)("i", {}),
                          (0, a.jsxs)("span", {
                            children: [
                              (0, a.jsx)("strong", { children: u.name }),
                              (0, a.jsxs)("small", {
                                children: [
                                  r.toLocaleString(e),
                                  " · ",
                                  s.record[2],
                                ],
                              }),
                            ],
                          }),
                        ],
                      },
                      n,
                    );
                  }),
                  (0, a.jsxs)("div", {
                    className: "map-legend",
                    children: [
                      (0, a.jsxs)("span", {
                        children: [
                          (0, a.jsx)("i", { className: "safe" }),
                          s.record[1],
                        ],
                      }),
                      (0, a.jsxs)("span", {
                        children: [
                          (0, a.jsx)("i", { className: "warm" }),
                          s.ui[13],
                        ],
                      }),
                      (0, a.jsxs)("span", {
                        children: [
                          (0, a.jsx)("i", { className: "hot" }),
                          s.record[2],
                        ],
                      }),
                    ],
                  }),
                ],
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel event-panel",
            children: [
              (0, a.jsxs)("div", {
                className: "panel-heading",
                children: [
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("small", { children: "NEXT WORLD EVENT" }),
                      (0, a.jsx)("h2", { children: R.name }),
                    ],
                  }),
                  (0, a.jsx)("span", {
                    className: "event-status",
                    children: s.ui[15],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "event-sigil",
                children: [
                  (0, a.jsx)("i", {}),
                  (0, a.jsx)("i", {}),
                  (0, a.jsx)("b", { children: R.emblem }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "countdown",
                children: [
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("strong", { children: "\u2014" }),
                      (0, a.jsx)("small", { children: "H" }),
                    ],
                  }),
                  (0, a.jsx)("em", { children: ":" }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("strong", { children: "\u2014" }),
                      (0, a.jsx)("small", { children: "M" }),
                    ],
                  }),
                  (0, a.jsx)("em", { children: ":" }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("strong", { children: "\u2014" }),
                      (0, a.jsx)("small", { children: "S" }),
                    ],
                  }),
                ],
              }),
              (0, a.jsxs)("p", {
                children: [R.summary, (0, a.jsx)("br", {}), s.record[4]],
              }),
              (0, a.jsxs)(l.default, {
                className: "primary-action",
                href: o(R, e),
                children: [
                  s.ui[14],
                  " ",
                  (0, a.jsx)("span", { children: "↗" }),
                ],
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel ledger-panel",
            id: "world-ledger",
            children: [
              (0, a.jsxs)("div", {
                className: "panel-heading",
                children: [
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("small", { children: "WORLD LEDGER" }),
                      (0, a.jsx)("h2", { children: s.ui[16] }),
                    ],
                  }),
                  (0, a.jsxs)(l.default, {
                    href: o(f, e),
                    children: [s.ui[14], " ↗"],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "ledger-summary",
                children: [
                  (0, a.jsxs)("span", {
                    children: [(0, a.jsx)("i", {}), s.record[1]],
                  }),
                  (0, a.jsx)("p", { children: s.record[4] }),
                  (0, a.jsx)("strong", { children: "\u2014" }),
                ],
              }),
              (0, a.jsx)("div", {
                className: "ledger-scroll",
                children: (0, a.jsxs)("table", {
                  children: [
                    (0, a.jsx)("thead", {
                      children: (0, a.jsxs)("tr", {
                        children: [
                          (0, a.jsx)("th", { children: "ID" }),
                          (0, a.jsx)("th", { children: "#" }),
                          (0, a.jsx)("th", { children: s.nav[4] }),
                          (0, a.jsx)("th", { children: s.nav[2] }),
                          (0, a.jsx)("th", { children: s.ui[21] }),
                          (0, a.jsx)("th", { children: s.ui[12] }),
                        ],
                      }),
                    }),
                    (0, a.jsx)("tbody", {
                      children: u.map(([i, n, r], t) => {
                        let d = v(e, i, n);
                        return (0, a.jsxs)(
                          "tr",
                          {
                            children: [
                              (0, a.jsx)("td", {
                                children: (0, a.jsxs)(l.default, {
                                  href: o(d, e),
                                  children: "—",
                                }),
                              }),
                              (0, a.jsx)("td", { children: t + 1 }),
                              (0, a.jsx)("td", {
                                children: (0, a.jsx)("span", {
                                  className: `record-kind ${r}`,
                                  children: d.kindLabel,
                                }),
                              }),
                              (0, a.jsx)("td", { children: d.name }),
                              (0, a.jsx)("td", {
                                children: (0, a.jsxs)(l.default, {
                                  className: "record-event",
                                  href: o(d, e),
                                  children: [
                                    d.summary,
                                    (0, a.jsx)("i", { children: "↗" }),
                                  ],
                                }),
                              }),
                              (0, a.jsx)("td", {
                                children: (0, a.jsxs)("span", {
                                  className: "record-status",
                                  children: [
                                    (0, a.jsx)("i", {}),
                                    s.record[4 === t ? 2 : 1],
                                  ],
                                }),
                              }),
                            ],
                          },
                          n,
                        );
                      }),
                    }),
                  ],
                }),
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel trends-panel",
            id: "world-trends",
            children: [
              (0, a.jsxs)("div", {
                className: "panel-heading",
                children: [
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("small", { children: "DISCOVERY TRENDS" }),
                      (0, a.jsx)("h2", { children: s.ui[17] }),
                    ],
                  }),
                  (0, a.jsx)("span", {
                    className: "trend-window",
                    children: "24H",
                  }),
                ],
              }),
              (0, a.jsx)("div", {
                className: "trend-tabs",
                role: "tablist",
                "aria-label": s.ui[17],
                children: [2, 1, 0, 4].map((e, l) =>
                  (0, a.jsx)(
                    "button",
                    {
                      type: "button",
                      role: "tab",
                      "aria-selected": j === l,
                      className: j === l ? "active" : "",
                      onClick: () => x(l),
                      children: s.kinds[e],
                    },
                    e,
                  ),
                ),
              }),
              (0, a.jsx)("div", {
                className: "trend-list",
                children: N[j].map((e) =>
                  (0, a.jsxs)(
                    l.default,
                    {
                      href: e.href,
                      className: "trend-row",
                      children: [
                        (0, a.jsx)("b", { children: e.rank }),
                        (0, a.jsxs)("span", {
                          children: [
                            (0, a.jsx)("strong", { children: e.name }),
                            (0, a.jsx)("small", { children: e.meta }),
                          ],
                        }),
                        (0, a.jsx)("em", { children: e.value }),
                        (0, a.jsx)("i", { children: "↗" }),
                      ],
                    },
                    e.rank,
                  ),
                ),
              }),
              (0, a.jsxs)("p", {
                className: "trend-note",
                children: [s.record[3], " ", s.record[4]],
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel market-panel",
            id: "market-pulse",
            children: [
              (0, a.jsxs)("div", {
                className: "panel-heading",
                children: [
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsx)("small", { children: "MARKET PULSE" }),
                      (0, a.jsx)("h2", { children: s.ui[18] }),
                    ],
                  }),
                  (0, a.jsxs)(l.default, {
                    href: o(v(e, "market", "world-01"), e),
                    children: [s.ui[14], " ↗"],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "market-chart",
                "aria-hidden": "true",
                children: "\u2014",
              }),
              (0, a.jsxs)("div", {
                className: "market-table",
                children: [
                  (0, a.jsxs)("div", {
                    className: "table-head",
                    children: [
                      (0, a.jsx)("span", { children: s.kinds[3] }),
                      (0, a.jsx)("span", { children: displayCopy[e].metrics.marketVolume }),
                      (0, a.jsx)("span", { children: "24H" }),
                    ],
                  }),
                  h.map(([i, n, r, s]) => {
                    let t = v(e, i, n);
                    return (0, a.jsxs)(
                      l.default,
                      {
                        href: o(t, e),
                        children: [
                          (0, a.jsx)("strong", { children: t.name }),
                          (0, a.jsxs)("span", { children: ["₲ ", r] }),
                          (0, a.jsx)("em", {
                            className: s.startsWith("+") ? "up" : "down",
                            children: s,
                          }),
                        ],
                      },
                      n,
                    );
                  }),
                ],
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel chronicle-panel",
            children: [
              (0, a.jsxs)("div", {
                className: "battle-art",
                children: [
                  (0, a.jsx)("span", { children: s.ui[20] }),
                  (0, a.jsx)("strong", { children: E.name }),
                  (0, a.jsx)("small", { children: "INTERFACE PROTOTYPE" }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "battle-copy",
                children: [
                  (0, a.jsxs)("span", {
                    className: "prototype-state",
                    children: ["\u25C7 ", s.record[1]],
                  }),
                  (0, a.jsx)("h2", { children: E.name }),
                  (0, a.jsx)("p", { children: E.summary }),
                  (0, a.jsxs)("div", {
                    children: [
                      (0, a.jsxs)("span", {
                        children: [
                          (0, a.jsx)("small", { children: s.kinds[1] }),
                          (0, a.jsx)("strong", { children: "\u2014" }),
                        ],
                      }),
                      (0, a.jsxs)("span", {
                        children: [
                          (0, a.jsx)("small", { children: s.ui[7] }),
                          (0, a.jsx)("strong", { children: "\u2014" }),
                        ],
                      }),
                      (0, a.jsxs)("span", {
                        children: [
                          (0, a.jsx)("small", { children: s.kinds[0] }),
                          (0, a.jsx)("strong", { children: "\u2014" }),
                        ],
                      }),
                    ],
                  }),
                  (0, a.jsxs)(l.default, {
                    href: o(E, e),
                    children: [
                      s.ui[14],
                      " ",
                      (0, a.jsx)("i", { children: "↗" }),
                    ],
                  }),
                ],
              }),
            ],
          }),
          (0, a.jsxs)("article", {
            className: "panel integrity-panel",
            "aria-label": s.ui[19],
            children: [
              (0, a.jsxs)("div", {
                className: "integrity-copy",
                children: [
                  (0, a.jsx)("small", { children: "WORLD INTEGRITY" }),
                  (0, a.jsx)("h2", { children: s.ui[19] }),
                  (0, a.jsxs)("p", {
                    children: [s.record[3], " ", s.record[4]],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "integrity-signals",
                children: [
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("i", { className: "pending" }),
                      (0, a.jsx)("small", { children: s.ui[12] }),
                      (0, a.jsx)("strong", { children: s.record[1] }),
                      (0, a.jsx)("em", { children: "\u2014" }),
                    ],
                  }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("i", { className: "pending" }),
                      (0, a.jsx)("small", { children: s.ui[7] }),
                      (0, a.jsx)("strong", { children: s.record[1] }),
                      (0, a.jsx)("em", { children: "\u2014" }),
                    ],
                  }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("i", { className: "pending" }),
                      (0, a.jsx)("small", { children: "SUI" }),
                      (0, a.jsx)("strong", { children: s.ui[20] }),
                      (0, a.jsx)("em", { children: s.record[2] }),
                    ],
                  }),
                  (0, a.jsxs)("span", {
                    children: [
                      (0, a.jsx)("i", { className: "private" }),
                      (0, a.jsx)("small", { children: s.ui[19] }),
                      (0, a.jsx)("strong", { children: s.record[2] }),
                      (0, a.jsx)("em", { children: s.record[4] }),
                    ],
                  }),
                ],
              }),
              (0, a.jsxs)("div", {
                className: "integrity-stamp",
                children: [
                  (0, a.jsx)("span", { children: "ATLAS" }),
                  (0, a.jsxs)("strong", {
                    children: ["PUBLIC", (0, a.jsx)("br", {}), "WORLD DATA"],
                  }),
                  (0, a.jsx)("small", { children: "PROTOTYPE · WORLD 01" }),
                ],
              }),
            ],
          }),
        ],
      }),
      (0, a.jsx)(AtlasFooter, { locale: e }),
    ],
  });
};
