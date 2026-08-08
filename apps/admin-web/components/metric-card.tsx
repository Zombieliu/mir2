export function MetricCard({
  title,
  value,
  delta,
  negative
}: {
  title: string;
  value: string;
  delta: string;
  negative?: boolean;
}) {
  return (
    <section className="card">
      <div className="metric-title">{title}</div>
      <div className="metric-value">{value}</div>
      <div className={negative ? "delta negative" : "delta"}>{delta}</div>
    </section>
  );
}
