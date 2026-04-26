export function Bars({
  rows
}: {
  rows: Array<{ label: string; value: number; suffix?: string }>;
}) {
  return (
    <div className="bars">
      {rows.map((row) => (
        <div className="bar-row" key={row.label}>
          <span>{row.label}</span>
          <div className="track">
            <div className="fill" style={{ width: `${row.value}%` }} />
          </div>
          <span>
            {row.value}
            {row.suffix ?? "%"}
          </span>
        </div>
      ))}
    </div>
  );
}
