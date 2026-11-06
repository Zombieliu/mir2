export type DocumentationArea = "guide" | "developers";

export type DocumentationSection = {
  id: string;
  title: string;
  paragraphs?: string[];
  items?: string[];
  ordered?: boolean;
  collapsible?: boolean;
  table?: { headers: string[]; rows: string[][] };
  code?: { language: string; value: string };
  image?: { src: string; alt: string; caption: string; width: number; height: number };
  note?: string;
  links?: { label: string; href: string }[];
};

export type DocumentationArticle = {
  slug: string;
  updated?: string;
  title: string;
  description: string;
  status: "live" | "testing" | "planned";
  environment: string;
  sections: DocumentationSection[];
};

export const documentationUpdated = "2026-11-02";

export const documentationStatus = {
  live: "已上线",
  testing: "测试中",
  planned: "规划中",
} as const;
