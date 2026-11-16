import type { DocumentationArea } from "@/lib/documentation-types";

type DocumentationGroup = { id: string; title: string; slugs: readonly string[] };

// One reading order for the directory, sidebar and previous/next links.
export const documentationGroups = {
  guide: [
    { id: "start", title: "开始游玩", slugs: ["getting-started"] },
    { id: "play", title: "边玩边查", slugs: ["controls", "classes-equipment", "exploration", "world"] },
    { id: "help", title: "规则与帮助", slugs: ["death", "payments", "playtest", "faq"] },
  ],
  developers: [
    { id: "understand", title: "认识开放边界", slugs: ["overview", "capabilities"] },
    { id: "integrate", title: "动手接入", slugs: ["quickstart", "environment", "identity", "interfaces"] },
    { id: "maintain", title: "测试与维护", slugs: ["testing", "security", "contributing", "roadmap"] },
  ],
} as const satisfies Record<DocumentationArea, readonly DocumentationGroup[]>;
