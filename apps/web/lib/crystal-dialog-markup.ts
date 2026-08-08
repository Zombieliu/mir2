export type CrystalDialogSegment = {
  text: string;
  colour?: string;
};

// Mirrors Crystal MirScrollingLabel's `{(.*?/.*?)}` colour directive while
// keeping adjacent directives independent.
const CRYSTAL_COLOUR_SPAN = /\{([^{}]*\/[^{}]*)\}/g;

export function stripCrystalDialogMarkup(text: string): string {
  return text
    .replace(/\{\/?[A-Z]+\}/gi, "")
    .replace(/<\$[^>]+>/g, "")
    .replace(/%[A-Z0-9_()]+/gi, "")
    .replace(/\s{2,}/g, " ")
    .trim();
}

export function parseCrystalColourSpans(line: string): CrystalDialogSegment[] {
  const segments: CrystalDialogSegment[] = [];
  let lastIndex = 0;
  CRYSTAL_COLOUR_SPAN.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = CRYSTAL_COLOUR_SPAN.exec(line)) !== null) {
    if (match.index > lastIndex) {
      segments.push({ text: line.slice(lastIndex, match.index) });
    }
    const parts = match[1].split("/");
    const text = parts[0];
    const colour = (parts[1] ?? "").trim();
    segments.push(colour ? { text, colour } : { text });
    lastIndex = match.index + match[0].length;
  }
  if (lastIndex < line.length) {
    segments.push({ text: line.slice(lastIndex) });
  }
  if (segments.length === 0) {
    segments.push({ text: line });
  }
  return segments;
}
