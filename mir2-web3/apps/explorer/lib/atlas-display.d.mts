import type { Locale } from "./atlas-data.mjs";
export type MetricId = "onlinePlayers" | "activeRegions" | "publicEvents" | "marketVolume";
export const atlasDataState: Readonly<{kind:"prototype";connected:false}>;
export const metricOrder: MetricId[];
export const displayCopy: Record<Locale,{summary:string;metrics:Record<MetricId,string>;policies:[string,string,string,string]}>;
export function getDisplayMetrics(locale:Locale,state:string):{id:MetricId;label:string;value:string;note:string}[];
export function getPolicyLinks(locale:Locale):{label:string;href:string}[];
