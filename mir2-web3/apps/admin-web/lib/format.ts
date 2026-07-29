export function formatNumber(value: number | undefined | null) {
  return (value ?? 0).toLocaleString();
}

export function formatCompactNumber(value: number | undefined | null) {
  return new Intl.NumberFormat(undefined, {
    notation: "compact",
    maximumFractionDigits: 1
  }).format(value ?? 0);
}

export function formatDateTime(value: number | undefined | null) {
  if (!value) return "-";
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "short",
    timeStyle: "medium"
  }).format(new Date(value));
}

export function formatVersion(value: number | undefined | null) {
  return value === undefined || value === null ? "-" : value.toLocaleString();
}

export function formatMap(mapTitle: string, mapFileName: string) {
  if (mapTitle.trim()) {
    return mapTitle;
  }
  if (mapFileName.trim()) {
    return mapFileName;
  }
  return "-";
}

export function statusTone(status: string): "default" | "success" | "warn" | "danger" {
  const normalized = status.toLowerCase();
  if (normalized === "healthy" || normalized === "normal" || normalized === "tracked" || normalized === "online") {
    return "success";
  }
  if (normalized === "banned" || normalized === "critical" || normalized === "unavailable") {
    return "danger";
  }
  if (normalized === "warn" || normalized === "high" || normalized === "notconfigured" || normalized === "chatbanned") {
    return "warn";
  }
  return "default";
}

export function serviceConfigStatusKey(configured: boolean, status: string) {
  if (configured) {
    return "common.connected";
  }
  if (status.toLowerCase() === "healthy") {
    return "common.devDefault";
  }
  return "common.unconfigured";
}
