"use client";

import type { CrystalTooltipColour, CrystalTooltipDocument } from "../../lib/shared-item-tooltip";
import type { ItemTooltipAlign } from "./original-client-item-tooltip";

const colours: Record<CrystalTooltipColour, string> = {
  white: "#ffffff", yellow: "#ffff00", deepSkyBlue: "#00bfff", darkOrange: "#ff8c00", plum: "#dda0dd",
  red: "#ff0000", cyan: "#00ffff", darkKhaki: "#bdb76b", khaki: "#f0e68c", orchid: "#da70d6",
};

/** Presentation only; every line, section and colour comes from common Rust. */
export function OriginalCrystalItemTooltip({document, align = "right"}: {
  document: CrystalTooltipDocument; align?: ItemTooltipAlign;
}) {
  return <div className={`original-item-tooltip align-${align}`} role="tooltip"
    data-tooltip-source-complete={document.sourceComplete} data-tooltip-broken={document.broken}>
    {document.sections.map((section) => <div key={section.kind} data-tooltip-section={section.kind}
      style={{borderTop: section.kind === "name" ? undefined : "1px solid rgba(190,157,99,.35)", padding: "4px 0"}}>
      {section.lines.map((line, index) => <div key={index} style={{color: colours[line.colour], whiteSpace: "pre-wrap"}}>{line.text}</div>)}
    </div>)}
  </div>;
}
