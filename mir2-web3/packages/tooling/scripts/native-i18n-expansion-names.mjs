// Explicit display-name review. Variant/gender/class/size codes are copied;
// neither map filenames nor item/monster IDs are ever changed.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve } from "node:path";
import { loadSources, validateValue } from "./native-i18n-expansion.mjs";
const root = resolve(import.meta.dirname, "../../..");
const target = resolve(root, "packages/tooling/data/native-i18n/expansion");
const tables = {
ar: `
10 EXP, Small HP Drug x1|10 EXP، دواء HP صغير x1
120 EXP, 30 gold, Worn Iron Bracelet x1|120 EXP، 30 ذهب، سوار حديدي بالٍ x1
30 EXP, 200 gold, Golden Pendant x1, Copper Ring x1|30 EXP، 200 ذهب، قلادة ذهبية x1، خاتم نحاسي x1
48 EXP, 60 gold|48 EXP، 60 ذهب
50 EXP, 200 gold, Precision Pendant x1|50 EXP، 200 ذهب، قلادة الدقة x1
80 EXP, 20 gold, Old Copper Ring x1|80 EXP، 20 ذهب، خاتم نحاسي قديم x1
Amethyst Room|غرفة الجمشت
Ancient S Tomb 1F|المقبرة القديمة S 1F
Ancient S Tomb 2F|المقبرة القديمة S 2F
Ancient S Tomb 3F|المقبرة القديمة S 3F
Armadillo|مدرّع
Armour Book|كتاب الدروع
Axe Oma|أوما الفأس
Bichon Tales (1)|حكايات بيشون (1)
Bichon Tales (2)|حكايات بيشون (2)
Bichon Tales (3)|حكايات بيشون (3)
Cheongbo Belt|حزام تشيونغبو
Cheongbo Boots|أحذية تشيونغبو
Cheongbo Helmet|خوذة تشيونغبو
Chieftain Warlock|زعيم المشعوذين
Cl Zombie|زومبي Cl
Convex Lens|عدسة محدبة
DY Craft|ورشة DY
DY Head|مقر DY
DY Med Storage|مخزن أدوية DY
DY Mine|منجم DY
DY Wearhouse|مستودع DY
Ebony Bangle|سوار الأبنوس
Ebony Pendant|قلادة الأبنوس
Elixir Of Ginseng|إكسير الجنسنغ
Excavenger Stuart|المنقّب Stuart
Fire Magic Robe|رداء سحر النار
Flail Oma|أوما المدراس
Forest Yetis {0}/{1}|يتي الغابة {0}/{1}
GM Quest|مهام GM
GT Store Mira|متجر GT Mira
Ginseng|جنسنغ
Gobby|غوبي
Gold Dark Armour|درع الظلام الذهبي
Gonryun He Publication (Divine Relic)|غونريون هي بابليكيشن (أثر إلهي)
Gonryun He Publication (Divine)|غونريون هي بابليكيشن (إلهي)
Gonryun He Publication (Holy Relic)|غونريون هي بابليكيشن (أثر مقدس)
Gonryun He Publication (Treasure)|غونريون هي بابليكيشن (كنز)
Gonryun Yeoseonru (Treasure)|غونريون يوسونرو (كنز)
Gonryunpasackle (Divine)|غونريون باساكل (إلهي)
Gonryunpasackle (Holy)|غونريون باساكل (مقدس)
Gonryunpasackle (Treasure)|غونريون باساكل (كنز)
Gonryunyongdrama (Holy)|غونريون يونغدراما (مقدس)
Gonryunyongdrama (Treasure)|غونريون يونغدراما (كنز)
Gt Invite|دعوة GT
Hyeoncheon Maseok|هيونتشون ماسوك
Hyeoncheon Suho Steel (Divine)|فولاذ هيونتشون سوهو (إلهي)
Hyeoncheon Suho Steel (Treasure)|فولاذ هيونتشون سوهو (كنز)
Ice Hell Temple|معبد الجحيم الجليدي
Ice Hell Temple (N)|معبد الجحيم الجليدي (N)
Keel Archer|رامي عظام التنين
Keratoid|كيراتُويد
Knapsack|حقيبة ظهر
Lamia|لاميا
Lantern|فانوس
Leather Armour|درع جلدي
Mackeral|إسقمري
Mackerel|إسقمري
Manticore|مانتيكور
Minotaur|مينوتور
Mir Statue|تمثال مير
Mobs from Prajna|وحوش براجنا
Mutton|لحم الضأن
Nd Zombie|زومبي Nd
Nephrite|نفريت
Oma King Robe|رداء ملك أوما
Platinum|بلاتين
Power Stone|حجر القوة
Progress: {0}/{1} target defeated.|التقدم: هُزم {0}/{1} من الأهداف.
Raiders Armour|درع المغيرين
Rev.Tao Office|مكتب كاهن الطاو
Scimitar|سيف معقوف
Shi Zombie|زومبي شي
Shrine|مزار
Soul Glyph|نقش الروح
Spirit Robe|رداء الروح
Stamina Aid|مقوّي التحمل
Steel Armour|درع فولاذي
Stiletto|خنجر رفيع
Tao Armour|درع الطاو
Tao Coronet|تاج الطاو
Taoist Drug|دواء الطاوي
Tiger Viper|أفعى النمر
Tinker|سمك التنش
Titan Armour|درع العملاق
Tongs|تونغز
Trident|رمح ثلاثي الشعب
Tucson Mage|ساحر توكسون
Venison|لحم الغزال
Walleye|سمك الوولاي
Weaver|النسّاج
Wedge Moth|عثة الوتد
West Oma Valley|غرب وادي أوما
Wizard Robe|رداء الساحر
Yob|يوب
Zinc Blades|نصال الزنك
`,
id: `
Algae|Alga
Amethyst|Kecubung
Anc Bringer|Utusan Purba
Apus Bow|Busur Apus
Armadillo|Armadilo
Baek Ta Glove|Sarung Tangan Baek Ta
Belt Of Darkness|Sabuk Kegelapan
Bengal Tiger|Harimau Benggala
Bichon Soldier Frank|Prajurit Bichon Frank
Big Hedge Kek Tal|Kek Tal Semak Besar
Black Creature Stone|Batu Makhluk Hitam
Black Fox Man|Manusia Rubah Hitam
Blue Fox Ring|Cincin Rubah Biru
Blue Horo Blaster|Penembak Horo Biru
Blue Ribbon|Pita Biru
Blue Tiger|Harimau Biru
Blue Wolf|Serigala Biru
Bluerecoverywater|Air Pemulihan Biru
Bok Ma Wheel|Roda Bok Ma
Bone Archer|Pemanah Tulang
Bone Captain|Kapten Tulang
Bone Whoo|Pemimpin Tengkorak
Bossy|Busana Pemimpin
Boulder Spirit|Roh Batu Besar
Bug Bat|Kelelawar Serangga
Bull|Banteng
Cat Widow|Janda Kucing
Cherry|Ceri
Cl Zombie|Zombi Cl
Compound Bow|Busur Majemuk
Craft Ring|Cincin Kerajinan
Currish|Anjing Ganas
Dark Devourer|Pemangsa Kegelapan
Dog Yo Arena|Arena Dog Yo
Dog Yo Mine Lobby|Lobi Tambang Dog Yo
Double Flower|Bunga Ganda
Evil Dragon Ring|Cincin Naga Jahat
Evil Slayer Gem|Permata Pembasmi Kejahatan
Eyeof Golden Snake|Mata Ular Emas
Fighting Cat|Kucing Petarung
Flail Oma|Oma Bandul Pukul
Formal|Busana Resmi
Ginseng|Ginseng
Gon Ryun Holy Light Blades (Divine)|Bilah Cahaya Suci Gon Ryun (Ilahi)
Gon Ryun Holy Light Bow (Divine)|Busur Cahaya Suci Gon Ryun (Ilahi)
Great Blue Fox Brace|Gelang Rubah Biru Agung
Great Blue Fox Ring|Cincin Rubah Biru Agung
Great Purple Fox Ring|Cincin Rubah Ungu Agung
Great Red Fox Ring|Cincin Rubah Merah Agung
Grim Bow|Busur Kelam
Hang Ma Wheel|Roda Hang Ma
Harden Rhino|Badak Berkulit Keras
Hedge Kek Tal|Kek Tal Semak
Hell Yama Blade|Bilah Yama Neraka
Horo Blaster|Penembak Horo
Hwan Ma Jin|Hwan Ma Jin
Hydra|Hidra
Hyeoncheon Suho Steel (Holy)|Baja Hyeoncheon Suho (Suci)
Ice Hell Temple KR|Kuil Neraka Es KR
Ice Minotaur|Minotaur Es
Insomnia|Insomnia
Jar|Tempayan
Keratoid|Keratoid
Lamia|Lamia
Long Bow|Busur Panjang
Magic Aid|Penunjang Sihir
Magic Wine|Arak Sihir
Mandible|Rahang Bawah
Manectric Blest|Manectric Terberkati
Manectric Claw|Manectric Cakar
Manticore|Mantikora
Minion Rouge|Minion Penyamun
Minion Warrior|Minion Prajurit
Minotaur|Minotaur
Mir Boots|Sepatu Bot Mir
Mir Ring|Cincin Mir
Mir Sword|Pedang Mir
Mongchon Delegate Michael|Utusan Mongchon Michael
Mongchon Wine|Arak Mongchon
Ninja|Ninja
Nok Chi Ring|Cincin Nok Chi
Oma Guard|Penjaga Oma
Oma Valley|Lembah Oma
Phoenix Bead|Manik Feniks
Platinum|Platina
Prajna Heart|Jantung Prajna
Rabbit|Kelinci
Raiders Boots|Sepatu Bot Penyerbu
Ram|Domba Jantan
Red Eye Skull|Tengkorak Mata Merah
Red Fox Ring|Cincin Rubah Merah
Red Moon Evil|Iblis Bulan Merah
Red Snake|Ular Merah
Retsuen Bow|Busur Retsuen
Ruby|Rubi
Rudolph|Rudolph
Santa|Santa
Scimitar|Pedang Lengkung
Shell Nipper|Pencapit Bercangkang
Shi Zombie|Zombi Shi
Sinseok Miner|Penambang Sinseok
Sky Stinger|Penyengat Langit
Soul Glyph|Aksara Jiwa
Soul Liquor|Arak Jiwa
Spider Frog|Katak Laba-laba
Stiletto|Belati Ramping
String|Tali
Tiger Snake|Ular Harimau
Tinker|Ikan Tench
Translucent|Kristal Tembus Cahaya
Tree Queen|Ratu Pohon
Treepath|Jalur Pepohonan
Trout|Ikan Trout
Walleye|Ikan Walleye
Way to Mongchon|Jalan ke Mongchon
Web|Jaring
Wooma Heart|Jantung Wooma
Worn Beadof Phoenix|Manik Feniks Usang
Yellow Dragon Bow (Golden Dragon)|Busur Naga Kuning (Naga Emas)
Yob|Yob
Zuma Fiend Bow|Busur Iblis Zuma
`};

const variants = /(?: \d+| ?\((?:M|F|S|L|XL|ARCH|ASSA|TAO|WAR|WIZ|ACC|DC|MC|SC)\)| ?\[1d\])+$/;
const existing = {};
for (const file of ["content-overrides.json", "review-overrides.json"]) {
  const p = resolve(target, file);
  if (existsSync(p)) for (const [key, values] of Object.entries(JSON.parse(readFileSync(p, "utf8")))) existing[key] = { ...existing[key], ...values };
}
const overrides = {};
for (const [locale, table] of Object.entries(tables)) {
  const rows = new Map(table.trim().split("\n").map(line => line.split("|")));
  for (const entry of loadSources().filter(e => e.key.startsWith("content."))) {
    if (existing[entry.key]?.[locale]) continue;
    const base = entry.en.replace(variants, "");
    const exact = rows.get(entry.en), value = exact ?? rows.get(base);
    if (value === undefined) continue;
    const text = exact ?? (value + entry.en.slice(base.length));
    const errors = validateValue(entry, locale, text);
    if (errors.length) throw new Error(`${entry.key}/${locale}: ${errors.join(", ")}`);
    (overrides[entry.key] ??= {})[locale] = text;
  }
}
const json = JSON.stringify(overrides, null, 2) + "\n";
const output = resolve(target, "names-overrides.json");
if (process.argv.includes("--check")) {
  if (readFileSync(output, "utf8") !== json) throw new Error("Reviewed display names are stale");
} else writeFileSync(output, json);
console.log(JSON.stringify({ reviewedKeys: Object.keys(overrides).length, values: Object.values(overrides).reduce((n, v) => n + Object.keys(v).length, 0) }));
