import type { Locale } from "@/lib/site-copy";

// Navigation labels only. Article translations require separate content review.
const labels: Record<Locale, { docs: string; guide: string; developers: string }> = {
  "zh-CN": { docs: "指南与文档", guide: "玩家指南", developers: "开发者文档" },
  en: { docs: "Guides & docs", guide: "Player guide", developers: "Developers" },
  ja: { docs: "ガイドと資料", guide: "プレイヤーガイド", developers: "開発者向け" },
  ko: { docs: "가이드 및 문서", guide: "플레이어 가이드", developers: "개발자 문서" },
  "zh-TW": { docs: "指南與文件", guide: "玩家指南", developers: "開發者文件" },
  es: { docs: "Guías y documentos", guide: "Guía del jugador", developers: "Desarrolladores" },
  "pt-BR": { docs: "Guias e documentos", guide: "Guia do jogador", developers: "Desenvolvedores" },
  de: { docs: "Anleitungen", guide: "Spielerhandbuch", developers: "Entwickler" },
  fr: { docs: "Guides et documentation", guide: "Guide du joueur", developers: "Développeurs" },
  ru: { docs: "Руководства", guide: "Руководство игрока", developers: "Разработчикам" },
  pl: { docs: "Poradniki i dokumentacja", guide: "Poradnik gracza", developers: "Dla programistów" },
  tr: { docs: "Rehberler ve belgeler", guide: "Oyuncu rehberi", developers: "Geliştiriciler" },
  th: { docs: "คู่มือและเอกสาร", guide: "คู่มือผู้เล่น", developers: "เอกสารนักพัฒนา" },
  vi: { docs: "Hướng dẫn và tài liệu", guide: "Hướng dẫn người chơi", developers: "Nhà phát triển" },
  id: { docs: "Panduan dan dokumentasi", guide: "Panduan pemain", developers: "Pengembang" },
  ar: { docs: "الأدلة والوثائق", guide: "دليل اللاعب", developers: "للمطورين" },
};

export function getDocumentationLabels(locale: Locale) {
  return labels[locale];
}
