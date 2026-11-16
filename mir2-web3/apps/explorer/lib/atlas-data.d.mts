export type Locale = "zh-CN" | "en" | "ja" | "ko" | "zh-TW" | "es" | "pt-BR" | "de" | "fr" | "ru" | "pl" | "tr" | "th" | "vi" | "id" | "ar";
export interface AtlasCopy { name:string; short:string; dir:'ltr'|'rtl'; nav:string[]; hero:string[]; ui:string[]; kinds:string[]; record:string[]; }
export interface ArchiveLink {kind:string;slug:string;}
export interface ArchiveRecord extends ArchiveLink {kindLabel:string;name:string;englishName:string;eyebrow:string;summary:string;status:string;emblem:string;tone:string;metrics:{id:string;label:string;value:string;note:string}[];overview:string[];facts:{label:string;value:string}[];timelineTitle:string;timeline:{time:string;title:string;detail:string}[];related:ArchiveLink[];}
export const locales:Locale[];
export const copy:Record<Locale,AtlasCopy>;
export const facts:Record<Locale,[string,string]>;
export const archiveKinds:string[];
export const archiveRecords:(ArchiveLink & {tone:string;related:ArchiveLink[]})[];
export function isLocale(value:string):value is Locale;
export function archiveHref(record:ArchiveLink,locale?:Locale):string;
export function getArchiveRecord(locale:Locale,kind:string,slug:string):ArchiveRecord|undefined;
export function requireArchiveRecord(locale:Locale,kind:string,slug:string):ArchiveRecord;
export const searchRecords:[string,string][];
export const ledgerRecords:[string,string,string][];
export const trendRecords:[string,string][][];
export const marketRows:[string,string,string,string][];
export const mapPoints:[string,string,string,string,string,string][];
