import type { Metadata } from "next";
import { ItemsCodexClient } from "./items-codex-client";

export const metadata: Metadata = {
  title: "物品资料库 · mir2 Codex",
  description:
    "传奇2(Crystal) 全物品资料库 — 玩家可查属性/职业需求/掉落来源，开发者可查 Crystal 原始字段与解码出处。",
};

export default function ItemsCodexPage() {
  return <ItemsCodexClient />;
}
