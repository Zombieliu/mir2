export function StatusBadge({
  children,
  tone = "default"
}: {
  children: React.ReactNode;
  tone?: "default" | "success" | "warn" | "danger";
}) {
  return <span className={`badge ${tone}`}>{children}</span>;
}
