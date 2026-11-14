import type { PromotionLocale } from "./promotion-copy";

export const PROMOTION_WINDOWS_COPY: Record<PromotionLocale, { title: string; body: string; download: string }> = {
  "zh-Hans": { title: "在 Windows 游戏中创建角色", body: "请在 Windows 客户端使用与本站相同的 Dubhe Wallet 登录并创建角色，然后返回此页刷新。邀请确认、充值和月卡记录属于同一游戏账号。", download: "下载 Windows 客户端" },
  "zh-Hant": { title: "在 Windows 遊戲中建立角色", body: "請在 Windows 用戶端使用與本站相同的 Dubhe Wallet 登入並建立角色，再返回此頁重新整理。邀請確認、儲值及月卡紀錄屬於同一遊戲帳號。", download: "下載 Windows 用戶端" },
  en: { title: "Create your character in the Windows game", body: "Sign in to the Windows client with the same Dubhe Wallet you use here, create a character, then refresh this page. Invitations, top-ups and monthly cards use that same game account.", download: "Download the Windows client" },
  "pt-BR": { title: "Crie seu personagem no jogo para Windows", body: "Entre no cliente Windows com a mesma Dubhe Wallet usada aqui, crie um personagem e atualize esta página. Convites, recargas e cartões mensais usam a mesma conta do jogo.", download: "Baixar o cliente Windows" },
  ru: { title: "Создайте персонажа в игре для Windows", body: "Войдите в клиент Windows через тот же Dubhe Wallet, создайте персонажа и обновите эту страницу. Приглашения, пополнения и месячные карты привязаны к той же игровой учётной записи.", download: "Скачать клиент Windows" },
  hi: { title: "Windows गेम में अपना पात्र बनाएँ", body: "यहाँ इस्तेमाल किए गए उसी Dubhe Wallet से Windows क्लाइंट में लॉग इन करें, पात्र बनाएँ और इस पेज को रीफ़्रेश करें। आमंत्रण, टॉप-अप और मासिक कार्ड उसी गेम खाते से जुड़े हैं।", download: "Windows क्लाइंट डाउनलोड करें" },
  id: { title: "Buat karakter di game Windows", body: "Masuk ke klien Windows dengan Dubhe Wallet yang sama, buat karakter, lalu muat ulang halaman ini. Undangan, isi ulang, dan kartu bulanan menggunakan akun game yang sama.", download: "Unduh klien Windows" },
  vi: { title: "Tạo nhân vật trong trò chơi Windows", body: "Đăng nhập máy khách Windows bằng cùng Dubhe Wallet, tạo nhân vật rồi làm mới trang này. Lời mời, nạp tiền và thẻ tháng dùng chung tài khoản trò chơi.", download: "Tải máy khách Windows" },
  th: { title: "สร้างตัวละครในเกม Windows", body: "เข้าสู่ไคลเอนต์ Windows ด้วย Dubhe Wallet เดียวกับที่ใช้ที่นี่ สร้างตัวละครแล้วรีเฟรชหน้านี้ คำเชิญ การเติมเงิน และบัตรรายเดือนจะอยู่ในบัญชีเกมเดียวกัน", download: "ดาวน์โหลดไคลเอนต์ Windows" },
  ar: { title: "أنشئ شخصيتك في لعبة Windows", body: "سجّل الدخول إلى عميل Windows باستخدام Dubhe Wallet نفسها، وأنشئ شخصية ثم حدّث هذه الصفحة. ترتبط الدعوات والشحن والبطاقات الشهرية بحساب اللعبة نفسه.", download: "تنزيل عميل Windows" },
};

export function windowsDownloadPath(locale: PromotionLocale): string {
  return `/${locale === "zh-Hans" ? "zh-CN" : locale === "zh-Hant" ? "zh-TW" : locale === "hi" ? "en" : locale}/download`;
}
