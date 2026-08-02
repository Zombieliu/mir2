import Link from "next/link";

export const dynamic = "force-dynamic";

type PublicDailyReport = {
  reportId: string;
  reportDate: string;
  timezone: string;
  publishedAtMs: number;
  playerMarkdown: string;
  highlights: {
    dailyActiveAccounts: number;
    gameplayEventCount: number;
    activeZones: number;
    topMaps: Array<{
      mapFileName: string;
      mapTitle: string;
      characterCount: number;
      percent: number;
    }>;
  };
};

export default async function WorldReportPage() {
  const result = await loadLatestWorldReport();
  return (
    <main className="world-report-page">
      <header className="world-report-header">
        <div>
          <p>OBELISK · MIR2 WORLD INTELLIGENCE</p>
          <h1>玛法世界日报</h1>
        </div>
        <Link href="/">返回游戏</Link>
      </header>
      {result.ok ? (
        <>
          <section className="world-report-hero">
            <div>
              <span>已发布 · {result.report.timezone}</span>
              <h2>{result.report.reportDate}</h2>
              <p>由真实世界事件聚合，经运营审核后发布。</p>
            </div>
            <div className="world-report-metrics">
              <WorldMetric label="冒险者" value={result.report.highlights.dailyActiveAccounts} />
              <WorldMetric label="世界行动" value={result.report.highlights.gameplayEventCount} />
              <WorldMetric label="活跃分区" value={result.report.highlights.activeZones} />
            </div>
          </section>
          <article className="world-report-copy">
            <SafeWorldMarkdown source={result.report.playerMarkdown} />
          </article>
          <section className="world-report-maps">
            <p>昨日冒险者聚集地</p>
            <div>
              {result.report.highlights.topMaps.map((map) => (
                <article key={`${map.mapFileName}:${map.mapTitle}`}>
                  <span>{map.mapFileName}</span>
                  <strong>{map.mapTitle}</strong>
                  <em>{map.characterCount} 人</em>
                </article>
              ))}
              {!result.report.highlights.topMaps.length ? <span>暂无地图人口快照</span> : null}
            </div>
          </section>
          <footer className="world-report-footer">
            <span>REPORT {result.report.reportId}</span>
            <time dateTime={new Date(result.report.publishedAtMs).toISOString()}>
              {new Date(result.report.publishedAtMs).toLocaleString("zh-CN", {
                timeZone: "Asia/Shanghai"
              })}
            </time>
          </footer>
        </>
      ) : (
        <section className="world-report-unavailable">
          <span>WORLD FEED OFFLINE</span>
          <h2>今天的世界报尚未发布</h2>
          <p>{result.error}</p>
          <Link href="/">进入玛法世界</Link>
        </section>
      )}
    </main>
  );
}

function WorldMetric({ label, value }: { label: string; value: number }) {
  return (
    <article>
      <span>{label}</span>
      <strong>{value.toLocaleString("zh-CN")}</strong>
    </article>
  );
}

function SafeWorldMarkdown({ source }: { source: string }) {
  return source.split(/\r?\n/).map((line, index) => {
    const value = line.trim();
    if (!value) return <div className="world-report-copy-gap" key={`gap-${index}`} />;
    if (value.startsWith("### ")) {
      return <h4 key={`h4-${index}`}>{worldInline(value.slice(4))}</h4>;
    }
    if (value.startsWith("## ")) {
      return <h3 key={`h3-${index}`}>{worldInline(value.slice(3))}</h3>;
    }
    if (value.startsWith("# ")) {
      return <h2 key={`h2-${index}`}>{worldInline(value.slice(2))}</h2>;
    }
    if (value.startsWith("- ")) {
      return <p className="world-report-copy-list" key={`li-${index}`}>• {worldInline(value.slice(2))}</p>;
    }
    return <p key={`p-${index}`}>{worldInline(value)}</p>;
  });
}

function worldInline(value: string) {
  return value.split(/(\*\*[^*]+\*\*)/).map((part, index) =>
    part.startsWith("**") && part.endsWith("**") ? (
      <strong key={`${part}-${index}`}>{part.slice(2, -2)}</strong>
    ) : (
      part
    )
  );
}

async function loadLatestWorldReport(): Promise<
  { ok: true; report: PublicDailyReport } | { ok: false; error: string }
> {
  const configured = process.env.MIR2_DAILY_REPORT_PUBLIC_API_URL?.trim();
  if (!configured && process.env.NODE_ENV === "production") {
    return { ok: false, error: "世界日报数据源尚未配置。" };
  }
  const url = configured ?? "http://127.0.0.1:7420/public/daily-report/latest";
  try {
    const response = await fetch(url, {
      cache: "no-store",
      signal: AbortSignal.timeout(8_000)
    });
    if (!response.ok) {
      return {
        ok: false,
        error: response.status === 404 ? "运营团队正在整理今日战报。" : `数据源 HTTP ${response.status}`
      };
    }
    return { ok: true, report: (await response.json()) as PublicDailyReport };
  } catch {
    return { ok: false, error: "世界情报暂时无法连接，请稍后再来。" };
  }
}
