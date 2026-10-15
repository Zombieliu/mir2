#!/usr/bin/env node
// Curated terminology only. This does not generate or approve a complete locale.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const catalogRoot = path.join(root, 'packages/game-data/data/native-i18n');
const outputRoot = path.join(root, 'packages/tooling/data/native-i18n/expansion');
export const locales = Object.freeze(['ru', 'hi', 'id', 'vi', 'th', 'ar']);
const terms = new Map();
const groups = new Map();
const normalize = (text) => text.trim().normalize('NFC').toLowerCase();
const nameKey = (text) => normalize(text).replace(/[\s_'-]/g, '');
function rows(group, table) {
  for (const line of table.trim().split('\n')) {
    const fields = line.split('|').map((s) => s.trim());
    if (fields.length !== 7 || fields.some((s) => !s)) throw new Error(`Invalid terminology row: ${fields[0]}`);
    const [en, ...values] = fields;
    const key = normalize(en);
    if (terms.has(key)) throw new Error(`Duplicate curated term: ${en}`);
    terms.set(key, Object.fromEntries(locales.map((locale, i) => [locale, values[i]])));
    groups.set(key, group);
  }
}

rows('class', `
Warrior | Воин | योद्धा | Prajurit | Chiến binh | นักรบ | محارب
Wizard | Маг | जादूगर | Penyihir | Pháp sư | นักเวท | ساحر
Taoist | Даос | ताओवादी | Taois | Đạo sĩ | นักพรต | طاوي
Assassin | Ассасин | हत्यारा | Pembunuh | Sát thủ | มือสังหาร | قاتل
Archer | Лучник | धनुर्धर | Pemanah | Cung thủ | นักธนู | رامي
`);

// Spell IDs remain canonical. In Crystal, Lightning is the beam (疾光電影),
// whereas ThunderBolt is the single-target sky strike (雷電術).
rows('skill', `
Back Step | Шаг назад | पीछे कदम | Langkah Mundur | Lùi bước | ก้าวถอยหลัง | خطوة للخلف
Battle Cry | Боевой клич | युद्ध घोष | Seruan Perang | Chiến hống | เสียงคำรามรบ | صيحة المعركة
Binding Shot | Сковывающий выстрел | बंधन बाण | Tembakan Pengikat | Tiễn trói buộc | ศรพันธนาการ | طلقة التقييد
Blade Avalanche | Лавина клинков | तलवारों की बौछार | Hujan Bilah | Bão kiếm | พายุคมดาบ | عاصفة النصال
Blessed Armour | Благословенная броня | आशीषित कवच | Zirah Berkah | Giáp chúc phúc | เกราะพรศักดิ์สิทธิ์ | درع مبارك
Blink | Скачок | त्वरित स्थानांतरण | Kedipan | Tốc biến | เคลื่อนย้ายฉับพลัน | انتقال خاطف
Blizzard | Метель | हिम झंझा | Badai Salju | Bão tuyết | พายุหิมะ | عاصفة ثلجية
Cat Tongue | Кошачий язык | बिल्ली की जीभ | Lidah Kucing | Lưỡi mèo | ลิ้นแมว | لسان القط
Concentration | Сосредоточение | एकाग्रता | Konsentrasi | Tập trung | สมาธิ | تركيز
Counter Attack | Контратака | प्रत्याक्रमण | Serangan Balik | Phản công | โจมตีสวนกลับ | هجوم مضاد
Cresent Slash | Удар полумесяца | अर्धचंद्र प्रहार | Tebasan Sabit | Trảm trăng khuyết | ฟันจันทร์เสี้ยว | شق الهلال
Cripple Shot | Калечащий выстрел | पंगु बाण | Tembakan Pelumpuh | Tiễn gây tàn phế | ศรพิการ | طلقة الإعاقة
Cross Half-Moon | Крест полумесяца | पार अर्धचंद्र | Silang Bulan Sabit | Thập tự bán nguyệt | จันทร์เสี้ยวกากบาท | الهلال المتقاطع
Curse | Проклятие | अभिशाप | Kutukan | Nguyền rủa | คำสาป | لعنة
Dark Body | Тёмное тело | अंधकार देह | Tubuh Kegelapan | Hắc thể | ร่างแห่งความมืด | جسد الظلام
Delayed Explosion | Отложенный взрыв | विलंबित विस्फोट | Ledakan Tertunda | Nổ chậm | ระเบิดหน่วงเวลา | انفجار مؤجل
Double Shot | Двойной выстрел | दोहरा बाण | Tembakan Ganda | Song tiễn | ยิงสองครั้ง | طلقة مزدوجة
Double Slash | Двойной удар | दोहरा प्रहार | Tebasan Ganda | Song trảm | ฟันสองครั้ง | شق مزدوج
Electric Shock | Электрошок | विद्युत आघात | Sengatan Listrik | Điện giật | ไฟฟ้าช็อต | صدمة كهربائية
Elemental Barrier | Стихийный барьер | तत्त्व अवरोध | Penghalang Elemen | Kết giới nguyên tố | ม่านพลังธาตุ | حاجز العناصر
Elemental Shot | Стихийный выстрел | तत्त्व बाण | Tembakan Elemen | Tiễn nguyên tố | ศรธาตุ | طلقة العناصر
Energy Repulsor | Энергетическое отталкивание | ऊर्जा प्रतिकर्षण | Penolak Energi | Đẩy lùi năng lượng | ผลักพลังงาน | صد الطاقة
Energy Shield | Энергетический щит | ऊर्जा ढाल | Perisai Energi | Khiên năng lượng | โล่พลังงาน | درع الطاقة
Entrapment | Захват | जकड़न | Penjeratan | Giam giữ | พันธนาการ | إيقاع في الفخ
Explosive Trap | Взрывная ловушка | विस्फोटक जाल | Jebakan Peledak | Bẫy nổ | กับดักระเบิด | فخ متفجر
Fatal Sword | Смертоносный меч | घातक तलवार | Pedang Maut | Đoạt mệnh kiếm | ดาบมรณะ | سيف قاتل
Fencing | Фехтование | तलवारबाज़ी | Ilmu Pedang | Kiếm thuật | วิชาดาบ | المبارزة
Fire Ball | Огненный шар | अग्निगोला | Bola Api | Hỏa cầu | ลูกไฟ | كرة نارية
Fire Bang | Огненный взрыв | अग्नि विस्फोट | Ledakan Api | Hỏa bộc | เพลิงระเบิด | انفجار ناري
Fire Bounce | Рикошет огня | उछलती अग्नि | Pantulan Api | Hỏa cầu nảy | ลูกไฟเด้ง | ارتداد النار
Fire Burst | Вспышка огня | अग्नि प्रस्फोट | Semburan Api | Hỏa bùng | เพลิงปะทุ | اندفاع النار
Fire Wall | Огненная стена | अग्नि दीवार | Dinding Api | Hỏa tường | กำแพงไฟ | جدار النار
Flame Disruptor | Разрыв пламени | ज्वाला भंजन | Pemecah Api | Phá diệm | เพลิงทำลาย | مبدد اللهب
Flame Field | Поле пламени | ज्वाला क्षेत्र | Medan Api | Hỏa vực | เขตเปลวไฟ | ميدان اللهب
Flaming Sword | Пылающий меч | ज्वलंत तलवार | Pedang Menyala | Liệt hỏa kiếm | ดาบเพลิง | سيف مشتعل
Flash Dash | Молниеносный рывок | त्वरित धावा | Terjangan Kilat | Thiểm kích | พุ่งพริบตา | اندفاع خاطف
Focus | Фокусировка | ध्यान | Fokus | Chuyên chú | ตั้งสมาธิ | تثبيت التركيز
Frost Crunch | Ледяная хватка | हिम प्रहार | Hantaman Beku | Băng chưởng | ฝ่ามือเยือกแข็ง | قبضة الصقيع
Fury | Неистовство | प्रचंड क्रोध | Amukan | Cuồng nộ | คลุ้มคลั่ง | هيجان
Great Fire Ball | Большой огненный шар | विशाल अग्निगोला | Bola Api Besar | Đại hỏa cầu | ลูกไฟใหญ่ | كرة نارية عظيمة
Half Moon | Полумесяц | अर्धचंद्र | Bulan Sabit | Bán nguyệt | จันทร์เสี้ยว | الهلال
Hallucination | Галлюцинация | मतिभ्रम | Halusinasi | Ảo giác | ภาพหลอน | هلوسة
Haste | Ускорение | तीव्रता | Percepatan | Tăng tốc | เร่งความเร็ว | تسريع
Healing | Исцеление | उपचार | Penyembuhan | Trị liệu | รักษา | الشفاء
Healing Circle | Круг исцеления | उपचार चक्र | Lingkar Penyembuhan | Vòng trị liệu | วงรักษา | دائرة الشفاء
Heavenly Sword | Небесный меч | दिव्य तलवार | Pedang Langit | Thiên kiếm | ดาบสวรรค์ | سيف سماوي
Hell Fire | Адское пламя | नरकाग्नि | Api Neraka | Địa ngục hỏa | เพลิงนรก | نار الجحيم
Hemorrhage | Кровотечение | रक्तस्राव | Pendarahan | Xuất huyết | เลือดไหล | نزيف
Hiding | Незаметность | छिपाव | Penyembunyian | Ẩn thân | ซ่อนตัว | تخفٍّ
Ice Storm | Ледяная буря | हिम तूफ़ान | Badai Es | Bão băng | พายุน้ำแข็ง | عاصفة جليدية
Ice Thrust | Ледяной выпад | हिम भेदन | Tusukan Es | Băng thứ | แทงน้ำแข็ง | طعنة جليدية
Immortal Skin | Бессмертная кожа | अमर त्वचा | Kulit Abadi | Da bất tử | ผิวอมตะ | جلد خالد
Light Body | Лёгкое тело | हल्की देह | Tubuh Ringan | Khinh thân | กายเบา | جسد خفيف
Lightning | Линия молнии | विद्युत किरण | Sinar Petir | Tia sét | ลำแสงสายฟ้า | شعاع البرق
Lion Roar | Львиный рёв | सिंह गर्जना | Auman Singa | Sư tử hống | สิงโตคำราม | زئير الأسد
Magic Booster | Усиление магии | जादू वृद्धि | Penguat Sihir | Cường hóa phép | เสริมเวทมนตร์ | تعزيز السحر
Magic Shield | Магический щит | जादुई ढाल | Perisai Sihir | Khiên phép | โล่เวทมนตร์ | درع سحري
Mass-Healing | Массовое исцеление | सामूहिक उपचार | Penyembuhan Massal | Quần thể trị liệu | รักษาหมู่ | شفاء جماعي
Mass-Hiding | Массовая незаметность | सामूहिक छिपाव | Penyembunyian Massal | Quần thể ẩn thân | ซ่อนตัวหมู่ | تخفٍّ جماعي
Meditation | Медитация | ध्यान साधना | Meditasi | Thiền định | ทำสมาธิ | تأمل
Mental State | Состояние духа | मानसिक अवस्था | Kondisi Batin | Tâm cảnh | สภาวะจิต | حالة ذهنية
Meteor Shower | Метеоритный дождь | उल्कावर्षा | Hujan Meteor | Mưa sao băng | ฝนอุกกาบาต | وابل شهب
Meteor Strike | Удар метеорита | उल्का प्रहार | Hantaman Meteor | Thiên thạch giáng | อุกกาบาตถล่ม | ضربة نيزكية
Mirroring | Зеркальный двойник | प्रतिबिंब | Pencerminan | Phân thân gương | ร่างสะท้อน | انعكاس
Moon Light | Лунный свет | चंद्रप्रकाश | Cahaya Bulan | Nguyệt quang | แสงจันทร์ | ضوء القمر
Moon Mist | Лунная дымка | चंद्र कुहासा | Kabut Bulan | Nguyệt vụ | หมอกจันทร์ | ضباب القمر
MP Eater | Поглощение MP | MP अवशोषण | Penyerap MP | Hút MP | ดูด MP | امتصاص MP
Napalm Shot | Напалмовый выстрел | अग्निदाह बाण | Tembakan Napalm | Tiễn napalm | ศรนาปาล์ม | طلقة نابالم
One With Nature | Единение с природой | प्रकृति से एकत्व | Menyatu dengan Alam | Hòa cùng thiên nhiên | เป็นหนึ่งกับธรรมชาติ | اتحاد مع الطبيعة
Pet Enhancer | Усиление питомца | साथी सुदृढ़ीकरण | Penguat Peliharaan | Cường hóa thú nuôi | เสริมพลังสัตว์เลี้ยง | تعزيز المرافق
Plague | Чума | महामारी | Wabah | Dịch bệnh | โรคระบาด | طاعون
Poison Cloud | Ядовитое облако | विष मेघ | Awan Racun | Độc vân | เมฆพิษ | سحابة سم
Poisoning | Отравление | विषप्रयोग | Peracunan | Thi độc | การวางยาพิษ | التسميم
Poison Shot | Ядовитый выстрел | विष बाण | Tembakan Racun | Độc tiễn | ศรพิษ | طلقة سامة
Poison Sword | Ядовитый меч | विष तलवार | Pedang Racun | Độc kiếm | ดาบพิษ | سيف مسموم
Portal | Портал | द्वार | Gerbang Teleportasi | Cổng dịch chuyển | ประตูวาร์ป | بوابة انتقال
Protection Field | Защитное поле | सुरक्षा क्षेत्र | Medan Pelindung | Trường bảo hộ | เขตป้องกัน | مجال حماية
Purification | Очищение | शुद्धिकरण | Pemurnian | Thanh tẩy | ชำระล้าง | تطهير
Rage | Ярость | क्रोध | Kemarahan | Nộ khí | โทสะ | غضب
Reincarnation | Перерождение | पुनर्जन्म | Reinkarnasi | Chuyển sinh | กลับชาติมาเกิด | تناسخ
Repulsion | Отталкивание | प्रतिकर्षण | Tolakan | Kháng cự | ผลักกระเด็น | صد
Revelation | Откровение | उद्घाटन | Penyingkapan | Khải thị | เผยความจริง | كشف
Shoulder Dash | Удар плечом | कंधे का धावा | Terjangan Bahu | Thiết sơn kháo | พุ่งชนไหล่ | اندفاع الكتف
Slashing Burst | Шквал ударов | प्रहार बौछार | Rentetan Tebasan | Liên trảm | ฟันต่อเนื่อง | وابل الشقوق
Slaying | Смертельный удар | घातक प्रहार | Tebasan Maut | Trảm sát | ฟันสังหาร | ضربة قاتلة
Soul Fire Ball | Духовный огненный шар | आत्मिक अग्निगोला | Bola Api Roh | Linh hồn hỏa phù | ลูกไฟวิญญาณ | كرة نار الروح
Soul Shield | Щит души | आत्मा ढाल | Perisai Jiwa | Linh hồn thuẫn | โล่วิญญาณ | درع الروح
Spirit Sword | Духовный меч | आत्मिक तलवार | Pedang Roh | Linh hồn kiếm | ดาบวิญญาณ | سيف الروح
Stonetrap | Каменная ловушка | पाषाण जाल | Jebakan Batu | Bẫy đá | กับดักหิน | فخ حجري
Storm Escape | Бегство в буре | तूफ़ानी पलायन | Pelarian Badai | Phong độn | หลบหนีพายุ | هروب العاصفة
Straight Shot | Прямой выстрел | सीधा बाण | Tembakan Lurus | Trực tiễn | ศรตรง | طلقة مستقيمة
Summon HolyDeva | Призыв святого дэвы | पवित्र देव आह्वान | Panggil Dewa Suci | Triệu hồi thiên thần | อัญเชิญเทพศักดิ์สิทธิ์ | استدعاء ديفا مقدس
Summon Shinsu | Призыв Shinsu | Shinsu आह्वान | Panggil Shinsu | Triệu hồi Shinsu | อัญเชิญ Shinsu | استدعاء Shinsu
Summon Skeleton | Призыв скелета | कंकाल आह्वान | Panggil Kerangka | Triệu hồi khô lâu | อัญเชิญโครงกระดูก | استدعاء هيكل عظمي
Summon Snakes | Призыв змей | सर्प आह्वान | Panggil Ular | Triệu hồi rắn | อัญเชิญงู | استدعاء الأفاعي
Summon Toad | Призыв жабы | भेक आह्वान | Panggil Katak | Triệu hồi cóc | อัญเชิญคางคก | استدعاء علجوم
Summon Vampire | Призыв вампира | पिशाच आह्वान | Panggil Vampir | Triệu hồi ma cà rồng | อัญเชิญแวมไพร์ | استدعاء مصاص دماء
Swift Feet | Быстрые ноги | तीव्र चरण | Langkah Cepat | Tật hành | เท้าว่องไว | أقدام سريعة
Teleport | Телепортация | स्थानांतरण | Teleportasi | Dịch chuyển | วาร์ป | انتقال آني
Thrusting | Пронзающий удар | भेदी प्रहार | Tusukan | Thứ sát | แทงทะลวง | طعنة نافذة
Thunder Bolt | Удар молнии | वज्र प्रहार | Sambaran Petir | Lôi điện | สายฟ้าฟาด | صاعقة
Thunder Storm | Грозовая буря | विद्युत तूफ़ान | Badai Petir | Lôi bạo | พายุสายฟ้า | عاصفة رعدية
Trap | Ловушка | जाल | Jebakan | Cạm bẫy | กับดัก | فخ
Trap Hexagon | Шестиугольная ловушка | षट्कोण जाल | Jebakan Segi Enam | Lục giác trận | กับดักหกเหลี่ยม | فخ سداسي
Turn Undead | Изгнание нежити | मृतजीव निष्कासन | Usir Mayat Hidup | Thánh ngôn | ขับไล่อันเดด | طرد الموتى الأحياء
Twin Drake Blade | Клинок двух драконов | जुड़वाँ ड्रैगन धार | Bilah Naga Kembar | Song long đao | ดาบมังกรคู่ | نصل التنينين
Ultimate Enchancer | Предельное усиление | परम सुदृढ़ीकरण | Penguat Tertinggi | Cường hóa tối thượng | เสริมพลังสูงสุด | تعزيز أقصى
Vampire Shot | Вампирский выстрел | रक्तचूषक बाण | Tembakan Vampir | Tiễn hút máu | ศรดูดเลือด | طلقة مصاص الدماء
Vampirism | Вампиризм | रक्तपान | Vampirisme | Hút máu | ดูดเลือด | امتصاص الدم
`);

rows('supply-and-equipment', `
Health | Здоровье | स्वास्थ्य | Kesehatan | Sinh lực | พลังชีวิต | الصحة
Mana | Мана | माना | Energi Mana | Pháp lực | มานา | المانا
HP potion | Зелье здоровья | स्वास्थ्य औषधि | Ramuan HP | Bình HP | ยาฟื้นฟู HP | جرعة صحة
MP potion | Зелье маны | माना औषधि | Ramuan MP | Bình MP | ยาฟื้นฟู MP | جرعة مانا
Red Potion | Зелье здоровья | स्वास्थ्य औषधि | Ramuan HP | Bình HP | ยาฟื้นฟู HP | جرعة صحة
Blue Potion | Зелье маны | माना औषधि | Ramuan MP | Bình MP | ยาฟื้นฟู MP | جرعة مانا
Sun Potion | Солнечное зелье | सूर्य औषधि | Ramuan Matahari | Bình thái dương | ยาตะวัน | جرعة الشمس
Potion | Зелье | औषधि | Ramuan | Bình thuốc | ยา | جرعة
Amulet | Талисман | ताबीज़ | Jimat | Bùa | เครื่องราง | تعويذة
Poison | Яд | विष | Racun | Độc | พิษ | سم
Poison powder | Ядовитый порошок | विष चूर्ण | Bubuk Racun | Bột độc | ผงพิษ | مسحوق سم
Red poison powder | Красный ядовитый порошок | लाल विष चूर्ण | Bubuk Racun Merah | Bột độc đỏ | ผงพิษแดง | مسحوق سم أحمر
Green poison powder | Зелёный ядовитый порошок | हरा विष चूर्ण | Bubuk Racun Hijau | Bột độc xanh | ผงพิษเขียว | مسحوق سم أخضر
Red Poison | Красный яд | लाल विष | Racun Merah | Độc đỏ | พิษแดง | سم أحمر
Green Poison | Зелёный яд | हरा विष | Racun Hijau | Độc xanh | พิษเขียว | سم أخضر
Town scroll | Свиток возвращения в город | नगर वापसी पत्र | Gulungan Pulang Kota | Cuộn về thành | คัมภีร์กลับเมือง | لفافة العودة إلى المدينة
Random scroll | Свиток случайного телепорта | यादृच्छिक स्थानांतरण पत्र | Gulungan Teleportasi Acak | Cuộn dịch chuyển ngẫu nhiên | คัมภีร์วาร์ปสุ่ม | لفافة انتقال عشوائي
Town Teleport | Свиток возвращения в город | नगर वापसी पत्र | Gulungan Pulang Kota | Cuộn về thành | คัมภีร์กลับเมือง | لفافة العودة إلى المدينة
Random Teleport | Свиток случайного телепорта | यादृच्छिक स्थानांतरण पत्र | Gulungan Teleportasi Acak | Cuộn dịch chuyển ngẫu nhiên | คัมภีร์วาร์ปสุ่ม | لفافة انتقال عشوائي
Amulet Of Revival | Талисман возрождения | पुनर्जीवन ताबीज़ | Jimat Kebangkitan | Bùa hồi sinh | เครื่องรางคืนชีพ | تعويذة الإحياء
Repair Oil | Масло ремонта | मरम्मत तेल | Minyak Perbaikan | Dầu sửa chữa | น้ำมันซ่อมแซม | زيت الإصلاح
War God Oil | Масло бога войны | युद्धदेव तेल | Minyak Dewa Perang | Dầu chiến thần | น้ำมันเทพสงคราม | زيت إله الحرب
Benediction Oil | Масло благословения | आशीर्वाद तेल | Minyak Berkah | Dầu chúc phúc | น้ำมันอวยพร | زيت البركة
Scroll Of Seal | Свиток печати | मुहर पत्र | Gulungan Segel | Cuộn phong ấn | คัมภีร์ผนึก | لفافة الختم
Small | Малый | छोटा | Kecil | Nhỏ | เล็ก | صغير
Medium | Средний | मध्यम | Sedang | Vừa | กลาง | متوسط
Large | Большой | बड़ा | Besar | Lớn | ใหญ่ | كبير
Weapon | Оружие | हथियार | Senjata | Vũ khí | อาวุธ | سلاح
Armour | Броня | कवच | Zirah | Áo giáp | เกราะ | درع
Helmet | Шлем | शिरस्त्राण | Helm | Mũ giáp | หมวกเกราะ | خوذة
Bracelet | Браслет | कंगन | Gelang | Vòng tay | กำไล | سوار
Ring | Кольцо | अंगूठी | Cincin | Nhẫn | แหวน | خاتم
Necklace | Ожерелье | हार | Kalung | Dây chuyền | สร้อยคอ | قلادة
Belt | Пояс | कमरबंध | Sabuk | Thắt lưng | เข็มขัด | حزام
Boots | Сапоги | जूते | Sepatu Bot | Giày | รองเท้าบูต | أحذية
Stone | Камень | पत्थर | Batu | Đá | หิน | حجر
Gem | Самоцвет | रत्न | Permata | Ngọc | อัญมณี | جوهرة
Orb | Сфера | गोला | Bola | Bảo châu | ลูกแก้ว | كرة
Book | Книга | पुस्तक | Buku | Sách | หนังสือ | كتاب
Scroll | Свиток | पत्र | Gulungan | Cuộn giấy | คัมภีร์ | لفافة
Sword | Меч | तलवार | Pedang | Kiếm | ดาบ | سيف
Blade | Клинок | धार | Bilah | Đao | คมดาบ | نصل
Bow | Лук | धनुष | Busur | Cung | ธนู | قوس
Axe | Топор | कुल्हाड़ी | Kapak | Rìu | ขวาน | فأس
Staff | Посох | दंड | Tongkat | Trượng | คทา | عصا
Shield | Щит | ढाल | Perisai | Khiên | โล่ | ترس
Torch | Факел | मशाल | Obor | Đuốc | คบเพลิง | مشعل
Meat | Мясо | मांस | Daging | Thịt | เนื้อ | لحم
Ore | Руда | अयस्क | Bijih | Quặng | แร่ | خام
Gold | Золото | सोना | Emas | Vàng | ทอง | ذهب
Gold Bar | Золотой слиток | सोने की ईंट | Batangan Emas | Thỏi vàng | ทองแท่ง | سبيكة ذهب
Equipment | Снаряжение | उपकरण | Perlengkapan | Trang bị | อุปกรณ์ | عتاد
Accessory Items | Аксессуары | आभूषण | Aksesori | Trang sức | เครื่องประดับ | إكسسوارات
Consumable Items | Расходуемые предметы | उपभोज्य वस्तुएँ | Barang Habis Pakai | Vật phẩm tiêu hao | ไอเทมใช้สิ้นเปลือง | مواد استهلاكية
Mount | Ездовое животное | सवारी | Tunggangan | Thú cưỡi | สัตว์ขี่ | دابة ركوب
Crafting Material | Материал изготовления | निर्माण सामग्री | Bahan Kerajinan | Nguyên liệu chế tạo | วัตถุดิบการผลิต | مادة تصنيع
Fishing | Рыбалка | मछली पकड़ना | Memancing | Câu cá | ตกปลา | صيد السمك
`);

// Keep the terse stat abbreviations recognisable across builds; the spelled-out
// labels below explain their gameplay meaning instead of changing a combat ID.
rows('stat', `
Physical attack | Физическая атака | भौतिक आक्रमण | Serangan Fisik | Công vật lý | พลังโจมตีกายภาพ | هجوم جسدي
Magic attack | Магическая атака | जादुई आक्रमण | Serangan Sihir | Công phép | พลังโจมตีเวท | هجوم سحري
Spiritual attack | Духовная атака | आत्मिक आक्रमण | Serangan Roh | Đạo thuật | พลังโจมตีวิญญาณ | هجوم روحي
Physical defence | Физическая защита | भौतिक रक्षा | Pertahanan Fisik | Thủ vật lý | ป้องกันกายภาพ | دفاع جسدي
Magic defence | Магическая защита | जादुई रक्षा | Pertahanan Sihir | Thủ phép | ป้องกันเวท | دفاع سحري
Accuracy | Точность | सटीकता | Akurasi | Chính xác | ความแม่นยำ | دقة
Agility | Ловкость | चपलता | Kelincahan | Nhanh nhẹn | ความคล่องตัว | رشاقة
Attack Speed | Скорость атаки | आक्रमण गति | Kecepatan Serangan | Tốc đánh | ความเร็วโจมตี | سرعة الهجوم
Luck | Удача | भाग्य | Keberuntungan | May mắn | โชค | حظ
Durability | Прочность | टिकाऊपन | Ketahanan | Độ bền | ความทนทาน | متانة
Weight | Вес | भार | Berat | Trọng lượng | น้ำหนัก | وزن
`);

rows('contextual-button', `
Finish | Завершить задание | कार्य पूरा करें | Selesaikan Misi | Hoàn thành nhiệm vụ | ส่งภารกิจ | إكمال المهمة
Return | Назад | वापस | Kembali | Quay lại | กลับ | رجوع
Share | Поделиться заданием | कार्य साझा करें | Bagikan Misi | Chia sẻ nhiệm vụ | แบ่งปันภารกิจ | مشاركة المهمة
`);

// These drafts lost negation, left core nouns in English, or dropped a sender
// placeholder. Explicit complete sentences preserve the actionable condition.
rows('gameplay-feedback', `
Not Enough Mana to cast. | Недостаточно маны для применения навыка. | कौशल प्रयोग के लिए पर्याप्त माना नहीं है। | Mana tidak cukup untuk menggunakan keterampilan. | Không đủ pháp lực để thi triển kỹ năng. | มานาไม่พอสำหรับใช้วิชา | لا توجد مانا كافية لاستخدام المهارة.
No Amulet | Нет талисмана | ताबीज़ नहीं है | Tidak Ada Jimat | Không có bùa | ไม่มีเครื่องราง | لا توجد تعويذة
You cannot use Potions here | Здесь нельзя использовать зелья. | यहाँ औषधियाँ उपयोग नहीं कर सकते। | Anda tidak dapat menggunakan ramuan di sini. | Không thể dùng bình thuốc ở đây. | ไม่สามารถใช้ยาได้ที่นี่ | لا يمكنك استخدام الجرعات هنا.
Warriors cannot use this item. | Воины не могут использовать этот предмет. | योद्धा यह वस्तु उपयोग नहीं कर सकते। | Prajurit tidak dapat menggunakan barang ini. | Chiến binh không thể dùng vật phẩm này. | นักรบไม่สามารถใช้ไอเทมนี้ได้ | لا يمكن للمحاربين استخدام هذه الأداة.
Wizards cannot use this item. | Маги не могут использовать этот предмет. | जादूगर यह वस्तु उपयोग नहीं कर सकते। | Penyihir tidak dapat menggunakan barang ini. | Pháp sư không thể dùng vật phẩm này. | นักเวทไม่สามารถใช้ไอเทมนี้ได้ | لا يمكن للسحرة استخدام هذه الأداة.
Taoists cannot use this item. | Даосы не могут использовать этот предмет. | ताओवादी यह वस्तु उपयोग नहीं कर सकते। | Taois tidak dapat menggunakan barang ini. | Đạo sĩ không thể dùng vật phẩm này. | นักพรตไม่สามารถใช้ไอเทมนี้ได้ | لا يمكن للطاويين استخدام هذه الأداة.
Assassins cannot use this item. | Ассасины не могут использовать этот предмет. | हत्यारे यह वस्तु उपयोग नहीं कर सकते। | Pembunuh tidak dapat menggunakan barang ini. | Sát thủ không thể dùng vật phẩm này. | มือสังหารไม่สามารถใช้ไอเทมนี้ได้ | لا يمكن للقتلة استخدام هذه الأداة.
Archers cannot use this item. | Лучники не могут использовать этот предмет. | धनुर्धर यह वस्तु उपयोग नहीं कर सकते। | Pemanah tidak dapat menggunakan barang ini. | Cung thủ không thể dùng vật phẩm này. | นักธนูไม่สามารถใช้ไอเทมนี้ได้ | لا يمكن للرماة استخدام هذه الأداة.
{0} would like to share a quest with you. Do you accept? | {0} хочет поделиться с вами заданием. Принять? | {0} आपके साथ एक कार्य साझा करना चाहता है। स्वीकार करें? | {0} ingin membagikan misi kepada Anda. Terima? | {0} muốn chia sẻ nhiệm vụ với bạn. Bạn có đồng ý không? | {0} ต้องการแบ่งปันภารกิจกับคุณ จะยอมรับหรือไม่? | يريد {0} مشاركة مهمة معك. هل تقبل؟
Quest could not be shared with anyone. | Не удалось поделиться заданием ни с кем. | किसी के साथ कार्य साझा नहीं हो सका। | Misi tidak dapat dibagikan kepada siapa pun. | Không thể chia sẻ nhiệm vụ với ai. | ไม่สามารถแบ่งปันภารกิจกับใครได้ | تعذرت مشاركة المهمة مع أي شخص.
Poison Resist + {0} | Сопротивление яду + {0} | विष प्रतिरोध + {0} | Ketahanan Racun + {0} | Kháng độc + {0} | ต้านทานพิษ + {0} | مقاومة السم + {0}
Poison Recovery + {0} | Восстановление после отравления + {0} | विष से उबरना + {0} | Pemulihan Racun + {0} | Hồi phục trúng độc + {0} | ฟื้นตัวจากพิษ + {0} | التعافي من السم + {0}
Health Recovery + {0} | Восстановление здоровья + {0} | स्वास्थ्य पुनर्प्राप्ति + {0} | Pemulihan Kesehatan + {0} | Hồi sinh lực + {0} | ฟื้นฟูพลังชีวิต + {0} | استعادة الصحة + {0}
Mana Recovery + {0} | Восстановление маны + {0} | माना पुनर्प्राप्ति + {0} | Pemulihan Mana + {0} | Hồi pháp lực + {0} | ฟื้นฟูมานา + {0} | استعادة المانا + {0}
`);

rows('map', `
Bichon Province | Провинция Бичон | Bichon प्रांत | Provinsi Bichon | Tỉnh Bichon | แคว้น Bichon | مقاطعة Bichon
Border Village | Пограничная деревня | सीमांत गाँव | Desa Perbatasan | Làng biên giới | หมู่บ้านชายแดน | قرية الحدود
Serpent Valley | Змеиная долина | सर्प घाटी | Lembah Ular | Thung lũng rắn | หุบเขางู | وادي الأفاعي
Oma Cave | Пещера Oma | Oma गुफा | Gua Oma | Hang Oma | ถ้ำ Oma | كهف Oma
Dead Mine | Мёртвая шахта | मृत खदान | Tambang Mati | Mỏ chết | เหมืองมรณะ | المنجم الميت
Dead Mine Entrance | Вход в мёртвую шахту | मृत खदान प्रवेश | Pintu Tambang Mati | Cửa mỏ chết | ทางเข้าเหมืองมรณะ | مدخل المنجم الميت
Wooma Temple | Храм Wooma | Wooma मंदिर | Kuil Wooma | Đền Wooma | วิหาร Wooma | معبد Wooma
Wooma Temple Entrance | Вход в храм Wooma | Wooma मंदिर प्रवेश | Pintu Kuil Wooma | Cửa đền Wooma | ทางเข้าวิหาร Wooma | مدخل معبد Wooma
Woomyon Woods | Лес Woomyon | Woomyon वन | Hutan Woomyon | Rừng Woomyon | ป่า Woomyon | غابة Woomyon
Mongchon Province | Провинция Mongchon | Mongchon प्रांत | Provinsi Mongchon | Tỉnh Mongchon | แคว้น Mongchon | مقاطعة Mongchon
Zuma Temple | Храм Zuma | Zuma मंदिर | Kuil Zuma | Đền Zuma | วิหาร Zuma | معبد Zuma
Zuma Palace | Дворец Zuma | Zuma महल | Istana Zuma | Cung điện Zuma | วัง Zuma | قصر Zuma
Wooma Palace | Дворец Wooma | Wooma महल | Istana Wooma | Cung điện Wooma | วัง Wooma | قصر Wooma
Palace | Дворец | महल | Istana | Cung điện | พระราชวัง | قصر
Meat Store | Мясная лавка | मांस की दुकान | Toko Daging | Tiệm thịt | ร้านขายเนื้อ | متجر اللحوم
Reagent Store | Лавка реагентов | अभिकर्मक दुकान | Toko Reagen | Tiệm pháp liệu | ร้านวัตถุดิบเวท | متجر الكواشف
Medicine Room | Аптечная комната | औषधि कक्ष | Ruang Obat | Phòng thuốc | ห้องยา | غرفة الأدوية
Library | Библиотека | पुस्तकालय | Perpustakaan | Thư viện | ห้องสมุด | مكتبة
Beauty Saloon | Салон красоты | सौंदर्य कक्ष | Salon Kecantikan | Tiệm làm đẹp | ร้านเสริมสวย | صالون تجميل
Inn | Трактир | सराय | Penginapan | Nhà trọ | โรงเตี๊ยม | نزل
Weapon Store | Оружейная лавка | हथियार की दुकान | Toko Senjata | Tiệm vũ khí | ร้านอาวุธ | متجر الأسلحة
Accessory Store | Лавка аксессуаров | आभूषण की दुकान | Toko Aksesori | Tiệm trang sức | ร้านเครื่องประดับ | متجر الإكسسوارات
Drapers Store | Лавка тканей | कपड़े की दुकान | Toko Kain | Tiệm vải | ร้านผ้า | متجر الأقمشة
Drapery Store | Лавка тканей | कपड़े की दुकान | Toko Kain | Tiệm vải | ร้านผ้า | متجر الأقمشة
Tavern | Таверна | मधुशाला | Kedai | Quán rượu | ร้านเหล้า | حانة
Kitchen | Кухня | रसोई | Dapur | Nhà bếp | ห้องครัว | مطبخ
Private House | Частный дом | निजी घर | Rumah Pribadi | Nhà riêng | บ้านส่วนตัว | منزل خاص
Academy | Академия | अकादमी | Akademi | Học viện | สำนักศึกษา | أكاديمية
Contest Ground | Арена состязаний | प्रतियोगिता मैदान | Arena Pertandingan | Đấu trường | ลานประลอง | ساحة المنافسة
Barracks | Казармы | सैनिक छावनी | Barak | Doanh trại | ค่ายทหาร | ثكنة
Prison | Тюрьма | कारागार | Penjara | Nhà tù | เรือนจำ | سجن
Border Restaurant | Пограничная харчевня | सीमांत भोजनालय | Restoran Perbatasan | Quán ăn biên giới | ร้านอาหารชายแดน | مطعم الحدود
Grocery Store | Лавка припасов | परचून की दुकान | Toko Kelontong | Tiệm tạp hóa | ร้านของชำ | متجر البقالة
Warehouse | Склад | गोदाम | Gudang | Kho | คลัง | مستودع
Mage House | Дом мага | जादूगर का घर | Rumah Penyihir | Nhà pháp sư | บ้านนักเวท | منزل الساحر
Natural Cave | Природная пещера | प्राकृतिक गुफा | Gua Alami | Hang tự nhiên | ถ้ำธรรมชาติ | كهف طبيعي
Ancient Natural Cave | Древняя природная пещера | प्राचीन प्राकृतिक गुफा | Gua Alami Kuno | Hang tự nhiên cổ | ถ้ำธรรมชาติโบราณ | كهف طبيعي قديم
Hidden Room | Тайная комната | गुप्त कक्ष | Ruang Tersembunyi | Phòng bí mật | ห้องลับ | غرفة خفية
Connection Path | Соединительный проход | संपर्क मार्ग | Jalur Penghubung | Lối thông | ทางเชื่อม | ممر رابط
Kings Tomb | Гробница короля | राजा की समाधि | Makam Raja | Lăng vua | สุสานกษัตริย์ | قبر الملك
Over Pass | Верхний переход | ऊपरी मार्ग | Jalan Atas | Đường vượt | ทางข้าม | ممر علوي
Ore Storage Place | Склад руды | अयस्क भंडार | Gudang Bijih | Kho quặng | คลังแร่ | مخزن الخام
Ghoul Cave | Пещера упырей | पिशाच गुफा | Gua Ghoul | Hang quỷ ăn xác | ถ้ำผีกินศพ | كهف الغيلان
Troll Mine | Шахта троллей | ट्रोल खदान | Tambang Troll | Mỏ quỷ khổng lồ | เหมืองโทรลล์ | منجم الترول
Mineral Mine | Рудная шахта | खनिज खदान | Tambang Mineral | Mỏ khoáng sản | เหมืองแร่ | منجم المعادن
Dark Forest | Тёмный лес | अंधकार वन | Hutan Gelap | Rừng tối | ป่ามืด | غابة مظلمة
Town Hall | Ратуша | नगर भवन | Balai Kota | Tòa thị chính | ศาลากลาง | دار البلدية
Armory | Арсенал | शस्त्रागार | Gudang Senjata | Kho vũ khí | คลังอาวุธ | ترسانة
Laundry | Прачечная | धुलाई घर | Binatu | Tiệm giặt | ร้านซักผ้า | مغسلة
Item Lab | Лаборатория предметов | वस्तु प्रयोगशाला | Laboratorium Barang | Phòng nghiên cứu vật phẩm | ห้องทดลองไอเทม | مختبر الأدوات
Drug Store | Аптека | औषधालय | Toko Obat | Tiệm thuốc | ร้านยา | متجر الأدوية
Red Valley | Красная долина | लाल घाटी | Lembah Merah | Thung lũng đỏ | หุบเขาแดง | الوادي الأحمر
Viper Maze | Змеиный лабиринт | विषधर भूलभुलैया | Labirin Ular Berbisa | Mê cung rắn độc | เขาวงกตอสรพิษ | متاهة الأفاعي
Viper Path | Змеиная тропа | विषधर मार्ग | Jalur Ular Berbisa | Đường rắn độc | ทางอสรพิษ | درب الأفاعي
Viper Cave | Змеиная пещера | विषधर गुफा | Gua Ular Berbisa | Hang rắn độc | ถ้ำอสรพิษ | كهف الأفاعي
Yimoogi Nest | Гнездо Yimoogi | Yimoogi का घोंसला | Sarang Yimoogi | Tổ Yimoogi | รัง Yimoogi | عش Yimoogi
Mystery Cave | Загадочная пещера | रहस्यमयी गुफा | Gua Misteri | Hang bí ẩn | ถ้ำปริศนา | كهف غامض
Mill | Мельница | चक्की | Penggilingan | Cối xay | โรงสี | طاحونة
Tactical Maze | Тактический лабиринт | रणनीतिक भूलभुलैया | Labirin Taktis | Mê cung chiến thuật | เขาวงกตกลยุทธ์ | متاهة تكتيكية
Angled Stone Tomb | Гробница угловатых камней | कोणीय पाषाण समाधि | Makam Batu Bersudut | Lăng đá góc cạnh | สุสานหินเหลี่ยม | قبر الحجر الزاوي
Angled Stone Tomb Entrance | Вход в гробницу угловатых камней | कोणीय पाषाण समाधि प्रवेश | Pintu Makam Batu Bersudut | Cửa lăng đá góc cạnh | ทางเข้าสุสานหินเหลี่ยม | مدخل قبر الحجر الزاوي
Black Dragon Dungeon | Подземелье чёрного дракона | काले ड्रैगन की कालकोठरी | Ruang Bawah Tanah Naga Hitam | Địa lao hắc long | ดันเจี้ยนมังกรดำ | دهليز التنين الأسود
Black Snake Palace | Дворец чёрной змеи | काले सर्प का महल | Istana Ular Hitam | Cung điện hắc xà | วังงูดำ | قصر الأفعى السوداء
White Dragon Passage | Проход белого дракона | श्वेत ड्रैगन मार्ग | Lorong Naga Putih | Lối bạch long | ทางมังกรขาว | ممر التنين الأبيض
Prison Hall | Тюремный зал | कारागार सभागार | Aula Penjara | Đại sảnh ngục | โถงเรือนจำ | قاعة السجن
Purgatory Hall | Зал чистилища | शोधन कक्ष | Aula Penyucian | Đại sảnh luyện ngục | โถงชำระบาป | قاعة المطهر
`);

rows('npc-job', `
Merchant | Торговец | व्यापारी | Pedagang | Thương nhân | พ่อค้า | تاجر
Travelling Merchant | Странствующий торговец | घुमंतू व्यापारी | Pedagang Keliling | Thương nhân lữ hành | พ่อค้าเร่ | تاجر متجول
Material Dealer | Торговец материалами | सामग्री व्यापारी | Pedagang Bahan | Thương nhân nguyên liệu | พ่อค้าวัตถุดิบ | تاجر مواد
Trust Merchant | Торговый посредник | विक्रय मध्यस्थ | Pedagang Titipan | Thương nhân ký gửi | พ่อค้าฝากขาย | تاجر بالوكالة
Inn Keeper | Хозяин трактира | सरायपाल | Pengelola Penginapan | Chủ quán trọ | เจ้าของโรงเตี๊ยม | صاحب النزل
Blacksmith | Кузнец | लोहार | Pandai Besi | Thợ rèn | ช่างตีเหล็ก | حداد
Crafts Lady | Мастерица | शिल्पकार | Perajin | Thợ thủ công | ช่างฝีมือ | حرفية
Master | Наставник | गुरु | Guru | Sư phụ | อาจารย์ | معلم
High Priest | Верховный жрец | महापुरोहित | Imam Agung | Đại tư tế | มหาปุโรหิต | كاهن أعلى
High Assassin | Главный ассасин | प्रधान हत्यारा | Pembunuh Utama | Đại sát thủ | หัวหน้ามือสังหาร | قائد القتلة
Captain | Капитан | कप्तान | Kapten | Đội trưởng | กัปตัน | نقيب
Alchemist | Алхимик | रसायनविद् | Alkemis | Nhà giả kim | นักเล่นแร่แปรธาตุ | خيميائي
Lottery | Продавец лотереи | लॉटरी विक्रेता | Penjual Lotre | Người bán xổ số | คนขายสลาก | بائع اليانصيب
Mir Guide | Проводник Mir | Mir मार्गदर्शक | Pemandu Mir | Hướng dẫn viên Mir | ผู้นำทาง Mir | دليل Mir
Assistant | Помощник | सहायक | Asisten | Trợ lý | ผู้ช่วย | مساعد
Investigator | Следователь | अन्वेषक | Penyelidik | Điều tra viên | ผู้สืบสวน | محقق
Lead Trainer | Главный тренер | प्रधान प्रशिक्षक | Pelatih Utama | Huấn luyện viên trưởng | หัวหน้าผู้ฝึก | مدرب رئيسي
Cave Guide | Пещерный проводник | गुफा मार्गदर्शक | Pemandu Gua | Người dẫn đường hang động | ผู้นำทางถ้ำ | دليل الكهف
Sailor | Моряк | नाविक | Pelaut | Thủy thủ | กะลาสี | بحار
Transport | Перевозчик | परिवाहक | Pengangkut | Người vận chuyển | ผู้ขนส่ง | ناقل
Protector | Защитник | रक्षक | Pelindung | Người bảo vệ | ผู้พิทักษ์ | حامٍ
Specialist | Специалист | विशेषज्ञ | Spesialis | Chuyên gia | ผู้เชี่ยวชาญ | اختصاصي
Librarian | Библиотекарь | पुस्तकालयाध्यक्ष | Pustakawan | Thủ thư | บรรณารักษ์ | أمين مكتبة
General | Генерал | सेनापति | Jenderal | Tướng quân | แม่ทัพ | جنرال
Administrator | Администратор | प्रशासक | Pengelola | Quản lý | ผู้ดูแล | مسؤول
Old Fisherman | Старый рыбак | वृद्ध मछुआरा | Nelayan Tua | Lão ngư | ชาวประมงเฒ่า | صياد سمك مسن
Wise Fisherman | Мудрый рыбак | ज्ञानी मछुआरा | Nelayan Bijak | Ngư ông thông thái | ชาวประมงผู้รอบรู้ | صياد سمك حكيم
Inspector | Инспектор | निरीक्षक | Inspektur | Thanh tra | ผู้ตรวจการ | مفتش
Village Chief | Староста | ग्राम प्रधान | Kepala Desa | Trưởng làng | ผู้ใหญ่บ้าน | زعيم القرية
Master Mage | Верховный маг | महाजादूगर | Penyihir Agung | Đại pháp sư | จอมเวท | ساحر أعظم
Fish Monger | Рыботорговец | मछली विक्रेता | Penjual Ikan | Người bán cá | คนขายปลา | بائع سمك
Squad Leader | Командир отряда | दल प्रमुख | Pemimpin Regu | Tiểu đội trưởng | หัวหน้าหมู่ | قائد فرقة
Hairdresser | Парикмахер | केश शिल्पी | Penata Rambut | Thợ làm tóc | ช่างทำผม | مصفف شعر
Wanderer | Странник | पथिक | Pengembara | Lãng khách | นักพเนจร | جوال
Examiner | Экзаменатор | परीक्षक | Penguji | Giám khảo | ผู้คุมสอบ | ممتحن
Pet Master | Наставник питомцев | साथी प्रशिक्षक | Pelatih Peliharaan | Huấn luyện thú nuôi | ผู้ฝึกสัตว์เลี้ยง | مدرب المرافقين
Commander | Командир | कमांडर | Komandan | Chỉ huy | ผู้บัญชาการ | قائد
Traveller | Путешественник | यात्री | Pelancong | Lữ khách | นักเดินทาง | مسافر
Apprentice | Ученик | शिष्य | Murid | Học đồ | ศิษย์ฝึกหัด | متدرب
Prison Guard | Тюремный страж | कारागार रक्षक | Penjaga Penjara | Lính gác ngục | ผู้คุมเรือนจำ | حارس السجن
Signpost | Указатель | दिशासूचक | Papan Petunjuk | Biển chỉ đường | ป้ายบอกทาง | لافتة إرشاد
Bulletin Board | Доска объявлений | सूचना पट्ट | Papan Pengumuman | Bảng thông báo | กระดานประกาศ | لوحة إعلانات
Peddler | Разносчик | फेरीवाला | Penjaja | Người bán hàng rong | หาบเร่ | بائع جوال
Steward | Управляющий | प्रबंधक | Pengurus | Quản sự | พ่อบ้าน | مشرف
`);

rows('monster', `
Scarecrow | Пугало | बिजूका | Orang-orangan Sawah | Bù nhìn | หุ่นไล่กา | فزاعة
Raking Cat | Кот с граблями | रेक वाली बिल्ली | Kucing Bergaru | Mèo cào | แมวคราด | قط بمشط زراعي
Hooking Cat | Кот с крюком | हुक वाली बिल्ली | Kucing Berkait | Mèo móc | แมวตะขอ | قط الخطاف
Hen | Курица | मुर्गी | Ayam Betina | Gà mái | แม่ไก่ | دجاجة
Chicken | Курица | मुर्गी | Ayam | Gà | ไก่ | دجاجة
Deer | Олень | हिरन | Rusa | Hươu | กวาง | أيل
Forest Yeti | Лесной йети | वन येति | Yeti Hutan | Dã nhân rừng | เยติป่า | يتي الغابة
Oma | Oma | Oma | Oma | Oma | Oma | Oma
Oma Warrior | Воин Oma | Oma योद्धा | Prajurit Oma | Chiến binh Oma | นักรบ Oma | محارب Oma
Oma Fighter | Боец Oma | Oma लड़ाका | Petarung Oma | Đấu sĩ Oma | นักสู้ Oma | مقاتل Oma
Oma Wizard | Маг Oma | Oma जादूगर | Penyihir Oma | Pháp sư Oma | นักเวท Oma | ساحر Oma
Oma Archer | Лучник Oma | Oma धनुर्धर | Pemanah Oma | Cung thủ Oma | นักธนู Oma | رامي Oma
Skeleton | Скелет | कंकाल | Kerangka | Khô lâu | โครงกระดูก | هيكل عظمي
Bone Fighter | Костяной боец | अस्थि लड़ाका | Petarung Tulang | Cốt đấu sĩ | นักสู้กระดูก | مقاتل عظمي
Bone Warrior | Костяной воин | अस्थि योद्धा | Prajurit Tulang | Cốt chiến binh | นักรบกระดูก | محارب عظمي
Bone Elite | Костяная элита | श्रेष्ठ अस्थि योद्धा | Elite Tulang | Cốt tinh anh | กระดูกชั้นยอด | نخبة عظمية
Ancient Skeleton | Древний скелет | प्राचीन कंकाल | Kerangka Kuno | Khô lâu cổ | โครงกระดูกโบราณ | هيكل عظمي قديم
Ancient Bone Fighter | Древний костяной боец | प्राचीन अस्थि लड़ाका | Petarung Tulang Kuno | Cốt đấu sĩ cổ | นักสู้กระดูกโบราณ | مقاتل عظمي قديم
Ancient Bone Warrior | Древний костяной воин | प्राचीन अस्थि योद्धा | Prajurit Tulang Kuno | Cốt chiến binh cổ | นักรบกระดูกโบราณ | محارب عظمي قديم
Ancient Bone Elite | Древняя костяная элита | प्राचीन श्रेष्ठ अस्थि योद्धा | Elite Tulang Kuno | Cốt tinh anh cổ | กระดูกชั้นยอดโบราณ | نخبة عظمية قديمة
Zombie | Зомби | ज़ॉम्बी | Zombi | Cương thi | ซอมบี้ | زومبي
Priest Zombie | Зомби-жрец | पुरोहित ज़ॉम्बी | Zombi Pendeta | Cương thi tư tế | ซอมบี้นักบวช | زومبي كاهن
Crawler Zombie | Ползающий зомби | रेंगता ज़ॉम्बी | Zombi Perayap | Cương thi bò | ซอมบี้คลาน | زومبي زاحف
Shaman Zombie | Зомби-шаман | शमन ज़ॉम्बी | Zombi Dukun | Cương thi pháp sư | ซอมบี้หมอผี | زومبي شامان
Ghoul | Упырь | शवभक्षी | Pemakan Mayat | Quỷ ăn xác | ผีกินศพ | غول
Cave Maggot | Пещерная личинка | गुफा कीड़ा | Belatung Gua | Giòi hang động | หนอนถ้ำ | يرقة الكهف
Cave Bat | Пещерная летучая мышь | गुफा चमगादड़ | Kelelawar Gua | Dơi hang | ค้างคาวถ้ำ | خفاش الكهف
Dung | Навозный червь | गोबर कीड़ा | Cacing Kotoran | Giun phân | หนอนมูล | دودة الروث
Wooma Soldier | Солдат Wooma | Wooma सैनिक | Serdadu Wooma | Binh sĩ Wooma | ทหาร Wooma | جندي Wooma
Wooma Fighter | Боец Wooma | Wooma लड़ाका | Petarung Wooma | Đấu sĩ Wooma | นักสู้ Wooma | مقاتل Wooma
Wooma Warrior | Воин Wooma | Wooma योद्धा | Prajurit Wooma | Chiến binh Wooma | นักรบ Wooma | محارب Wooma
Flaming Wooma | Пылающий Wooma | ज्वलंत Wooma | Wooma Menyala | Wooma rực lửa | Wooma เพลิง | Wooma المشتعل
Wooma Guardian | Страж Wooma | Wooma रक्षक | Penjaga Wooma | Hộ vệ Wooma | ผู้พิทักษ์ Wooma | حارس Wooma
Wooma Taurus | Бык Wooma | Wooma वृषभ | Banteng Wooma | Ngưu vương Wooma | กระทิง Wooma | ثور Wooma
Zuma Archer | Лучник Zuma | Zuma धनुर्धर | Pemanah Zuma | Cung thủ Zuma | นักธนู Zuma | رامي Zuma
Zuma Statue | Статуя Zuma | Zuma प्रतिमा | Patung Zuma | Tượng Zuma | รูปปั้น Zuma | تمثال Zuma
Zuma Guardian | Страж Zuma | Zuma रक्षक | Penjaga Zuma | Hộ vệ Zuma | ผู้พิทักษ์ Zuma | حارس Zuma
Zuma Taurus | Бык Zuma | Zuma वृषभ | Banteng Zuma | Ngưu vương Zuma | กระทิง Zuma | ثور Zuma
Archer Guard | Страж-лучник | धनुर्धर प्रहरी | Penjaga Pemanah | Lính gác cung | ยามธนู | حارس رامي
Giant Rat | Гигантская крыса | विशाल चूहा | Tikus Raksasa | Chuột khổng lồ | หนูยักษ์ | جرذ عملاق
Black Maggot | Чёрная личинка | काला कीड़ा | Belatung Hitam | Giòi đen | หนอนดำ | يرقة سوداء
Centipede | Сороконожка | कनखजूरा | Kelabang | Rết | ตะขาบ | حريش
Giant Worm | Гигантский червь | विशाल कृमि | Cacing Raksasa | Giun khổng lồ | หนอนยักษ์ | دودة عملاقة
Evil Centipede | Злобная сороконожка | दुष्ट कनखजूरा | Kelabang Jahat | Rết ác | ตะขาบชั่วร้าย | حريش شرير
Red Boar | Красный кабан | लाल वराह | Babi Hutan Merah | Lợn rừng đỏ | หมูป่าแดง | خنزير بري أحمر
Black Boar | Чёрный кабан | काला वराह | Babi Hutan Hitam | Lợn rừng đen | หมูป่าดำ | خنزير بري أسود
White Boar | Белый кабан | सफेद वराह | Babi Hutan Putih | Lợn rừng trắng | หมูป่าขาว | خنزير بري أبيض
Baby Pig | Поросёнок | सूअर का बच्चा | Anak Babi | Heo con | ลูกหมู | خنوص
Chick | Цыплёнок | चूज़ा | Anak Ayam | Gà con | ลูกไก่ | كتكوت
Kitten | Котёнок | बिलौटा | Anak Kucing | Mèo con | ลูกแมว | هر صغير
Black Kitten | Чёрный котёнок | काला बिलौटा | Anak Kucing Hitam | Mèo con đen | ลูกแมวดำ | هر أسود
Baby Skeleton | Маленький скелет | छोटा कंकाल | Kerangka Kecil | Khô lâu nhỏ | โครงกระดูกจิ๋ว | هيكل عظمي صغير
White Serpent | Белый змей | श्वेत सर्प | Ular Putih | Bạch xà | งูขาว | أفعى بيضاء
Blue Viper | Синяя гадюка | नीला विषधर | Ular Berbisa Biru | Rắn độc xanh lam | อสรพิษน้ำเงิน | أفعى زرقاء
Yellow Viper | Жёлтая гадюка | पीला विषधर | Ular Berbisa Kuning | Rắn độc vàng | อสรพิษเหลือง | أفعى صفراء
Guardian Viper | Гадюка-страж | रक्षक विषधर | Ular Penjaga | Rắn độc hộ vệ | อสรพิษพิทักษ์ | أفعى حارسة
Evil Spider | Злобный паук | दुष्ट मकड़ी | Laba-laba Jahat | Nhện ác | แมงมุมชั่วร้าย | عنكبوت شرير
Vampire Spider | Паук-вампир | रक्तचूषक मकड़ी | Laba-laba Vampir | Nhện hút máu | แมงมุมดูดเลือด | عنكبوت مصاص دماء
Snake Scorpion | Змей-скорпион | सर्प बिच्छू | Kalajengking Ular | Rắn bọ cạp | งูแมงป่อง | عقرب أفعواني
Charmed Snake | Зачарованная змея | मंत्रमुग्ध सर्प | Ular Terpesona | Rắn mê hoặc | งูต้องมนตร์ | أفعى مسحورة
Evil Snake | Злобная змея | दुष्ट सर्प | Ular Jahat | Rắn ác | งูชั่วร้าย | أفعى شريرة
Treasure Box | Сундук сокровищ | खज़ाना पेटी | Peti Harta | Rương báu | หีบสมบัติ | صندوق كنز
Elite Archer | Элитный лучник | श्रेष्ठ धनुर्धर | Pemanah Elite | Cung thủ tinh anh | นักธนูชั้นยอด | رامي نخبة
Elite Statue | Элитная статуя | श्रेष्ठ प्रतिमा | Patung Elite | Tượng tinh anh | รูปปั้นชั้นยอด | تمثال نخبة
Elite Guardian | Элитный страж | श्रेष्ठ रक्षक | Penjaga Elite | Hộ vệ tinh anh | ผู้พิทักษ์ชั้นยอด | حارس نخبة
Clone | Двойник | प्रतिरूप | Klon | Phân thân | ร่างแยก | نسخة
Assassin Clone | Двойник ассасина | हत्यारे का प्रतिरूप | Klon Pembunuh | Phân thân sát thủ | ร่างแยกมือสังหาร | نسخة قاتل
Holy Deva | Святой дэва | पवित्र देव | Dewa Suci | Thiên thần | เทพศักดิ์สิทธิ์ | ديفا مقدس
Shinsu | Shinsu | Shinsu | Shinsu | Shinsu | Shinsu | Shinsu
Yimoogi | Yimoogi | Yimoogi | Yimoogi | Yimoogi | Yimoogi | Yimoogi
`);

rows('contextual-npc-and-map', `
Teleporter | Телепортер | स्थानांतरणकर्ता | Pengantar Teleportasi | Người dịch chuyển | ผู้ให้บริการวาร์ป | ناقل آني
Warehouse keeper | Хранитель склада | गोदामपाल | Penjaga Gudang | Thủ kho | ผู้ดูแลคลัง | أمين المستودع
Cloth merchant | Торговец тканями | कपड़ा व्यापारी | Pedagang Kain | Người bán vải | พ่อค้าผ้า | تاجر أقمشة
Potion merchant | Торговец зельями | औषधि व्यापारी | Pedagang Ramuan | Người bán thuốc | พ่อค้ายา | تاجر جرعات
Grocery merchant | Торговец припасами | परचून व्यापारी | Pedagang Kelontong | Người bán tạp hóa | พ่อค้าของชำ | تاجر بقالة
Accessory merchant | Торговец аксессуарами | आभूषण व्यापारी | Pedagang Aksesori | Người bán trang sức | พ่อค้าเครื่องประดับ | تاجر إكسسوارات
Book merchant | Книготорговец | पुस्तक व्यापारी | Pedagang Buku | Người bán sách | คนขายหนังสือ | بائع كتب
Trainer Warrior | Наставник воинов | योद्धा प्रशिक्षक | Pelatih Prajurit | Huấn luyện viên chiến binh | ครูฝึกนักรบ | مدرب المحاربين
Trainer Wizard | Наставник магов | जादूगर प्रशिक्षक | Pelatih Penyihir | Huấn luyện viên pháp sư | ครูฝึกนักเวท | مدرب السحرة
Trainer Taoist | Наставник даосов | ताओवादी प्रशिक्षक | Pelatih Taois | Huấn luyện viên đạo sĩ | ครูฝึกนักพรต | مدرب الطاويين
Bichon Wall Board | Доска объявлений Бичона | Bichon सूचना पट्ट | Papan Pengumuman Bichon | Bảng thông báo Bichon | กระดานประกาศ Bichon | لوحة إعلانات Bichon
Mud Wall Board | Доска объявлений Mud Wall | Mud Wall सूचना पट्ट | Papan Pengumuman Mud Wall | Bảng thông báo Mud Wall | กระดานประกาศ Mud Wall | لوحة إعلانات Mud Wall
Border Village Board | Доска пограничной деревни | सीमांत गाँव सूचना पट्ट | Papan Desa Perbatasan | Bảng làng biên giới | กระดานหมู่บ้านชายแดน | لوحة قرية الحدود
Blacksmith shop | Кузница | लोहार की दुकान | Bengkel Pandai Besi | Lò rèn | โรงตีเหล็ก | ورشة الحدادة
Mine | Шахта | खदान | Tambang | Mỏ | เหมือง | منجم
Residence | Жилище | निवास | Kediaman | Nơi ở | ที่พัก | مسكن
Dungeon | Подземелье | कालकोठरी | Ruang Bawah Tanah | Địa lao | ดันเจี้ยน | دهليز
`);

rows('sized-potion', `
(HP)Potion Small | Малое зелье здоровья | छोटी स्वास्थ्य औषधि | Ramuan HP Kecil | Bình HP nhỏ | ยาฟื้นฟู HP เล็ก | جرعة صحة صغيرة
(MP)Potion Small | Малое зелье маны | छोटी माना औषधि | Ramuan MP Kecil | Bình MP nhỏ | ยาฟื้นฟู MP เล็ก | جرعة مانا صغيرة
(HP)Potion Medium | Среднее зелье здоровья | मध्यम स्वास्थ्य औषधि | Ramuan HP Sedang | Bình HP vừa | ยาฟื้นฟู HP กลาง | جرعة صحة متوسطة
(MP)Potion Medium | Среднее зелье маны | मध्यम माना औषधि | Ramuan MP Sedang | Bình MP vừa | ยาฟื้นฟู MP กลาง | جرعة مانا متوسطة
(HP)Potion Large | Большое зелье здоровья | बड़ी स्वास्थ्य औषधि | Ramuan HP Besar | Bình HP lớn | ยาฟื้นฟู HP ใหญ่ | جرعة صحة كبيرة
(MP)Potion Large | Большое зелье маны | बड़ी माना औषधि | Ramuan MP Besar | Bình MP lớn | ยาฟื้นฟู MP ใหญ่ | جرعة مانا كبيرة
`);

// Full statements are selected by stable key below, not by replacing words in
// prose. In particular, preserve the saved-safe-zone rule and real skill effect.
const descriptions = {
  'content.item.719.description': [
    'Перемещает персонажа в последнюю сохранённую безопасную зону.',
    'पात्र को अंतिम दर्ज सुरक्षित क्षेत्र में पहुँचाता है।',
    'Memindahkan karakter ke zona aman terakhir yang tersimpan.',
    'Đưa nhân vật về vùng an toàn được lưu gần nhất.',
    'วาร์ปตัวละครไปยังเขตปลอดภัยที่บันทึกไว้ล่าสุด',
    'ينقل الشخصية إلى آخر منطقة آمنة محفوظة.'
  ],
  'content.item.711.description': [
    'Используется в некоторых заклинаниях даоса для наложения красного яда, снижающего защиту.',
    'कुछ ताओवादी मंत्रों में लाल विष लगाने के लिए प्रयुक्त होता है; यह रक्षा घटाता है।',
    'Digunakan dalam mantra Taois tertentu untuk memberi racun merah yang mengurangi pertahanan.',
    'Dùng trong một số phép của đạo sĩ để gây độc đỏ, làm giảm phòng thủ.',
    'ใช้กับวิชานักพรตบางชนิดเพื่อทำให้ติดพิษแดงซึ่งลดพลังป้องกัน',
    'يُستخدم في بعض تعاويذ الطاوي لإحداث سم أحمر يقلل الدفاعات.'
  ],
  'content.item.973.description': [
    'Каждый уровень навыка повышает точность попадания.',
    'कौशल के प्रत्येक स्तर के साथ प्रहार की सटीकता बढ़ती है।',
    'Setiap tingkat keterampilan meningkatkan akurasi serangan.',
    'Mỗi cấp kỹ năng tăng độ chính xác khi đánh.',
    'ความแม่นยำในการโจมตีเพิ่มขึ้นตามระดับวิชา',
    'تزداد دقة الإصابة مع كل مستوى للمهارة.'
  ],
  'content.item.974.description': [
    'Наполняет оружие силой.',
    'हथियार में शक्ति भरता है।',
    'Mengisi senjata dengan kekuatan.',
    'Truyền sức mạnh vào vũ khí.',
    'อัดพลังเข้าสู่อาวุธ',
    'يشبع السلاح بالقوة.'
  ],
  'content.item.975.description': [
    'Увеличивает дальность вашей атаки.',
    'आपके प्रहार की पहुँच बढ़ाता है।',
    'Meningkatkan jangkauan serangan Anda.',
    'Tăng tầm đánh của bạn.',
    'เพิ่มระยะการโจมตีของคุณ',
    'يزيد مدى هجومك.'
  ],
  'content.item.976.description': [
    'Удар по дуге позволяет поразить до 4 целей одновременно.',
    'हथियार का घुमावदार प्रहार एक साथ अधिकतम 4 लक्ष्यों को मार सकता है।',
    'Ayunan melengkung memungkinkan senjata mengenai hingga 4 sasaran sekaligus.',
    'Vung vũ khí theo đường cong, có thể đánh tối đa 4 mục tiêu cùng lúc.',
    'เหวี่ยงอาวุธเป็นวงโค้งเพื่อโจมตีได้สูงสุด 4 เป้าหมายพร้อมกัน',
    'تسمح أرجحة السلاح بقوس بإصابة ما يصل إلى 4 أهداف في وقت واحد.'
  ],
  'content.item.990.description': [
    'Атакует издалека, выпуская огненный шар.',
    'अग्निगोला फेंककर दूर से आक्रमण करता है।',
    'Menyerang dari jarak jauh dengan melempar bola api.',
    'Ném hỏa cầu để tấn công từ xa.',
    'โจมตีระยะไกลด้วยการขว้างลูกไฟ',
    'يهاجم من مسافة بعيدة بإطلاق كرة نارية.'
  ],
  'content.item.993.description': [
    'Усиленная версия навыка «Огненный шар».',
    'अग्निगोला मंत्र का अधिक शक्तिशाली रूप।',
    'Versi lebih kuat dari mantra Bola Api.',
    'Phiên bản mạnh hơn của phép Hỏa cầu.',
    'วิชาลูกไฟที่ทรงพลังยิ่งขึ้น',
    'نسخة أقوى من تعويذة الكرة النارية.'
  ],
  'content.item.995.description': [
    'Призывает молнию с неба для удара по цели.',
    'लक्ष्य पर आक्रमण करने के लिए आकाश से बिजली बुलाता है।',
    'Memanggil sambaran petir dari langit untuk menyerang sasaran.',
    'Gọi sét từ trời đánh xuống mục tiêu.',
    'เรียกสายฟ้าจากท้องฟ้าลงมาโจมตีเป้าหมาย',
    'يستدعي صاعقة من السماء لمهاجمة الهدف.'
  ],
  'content.item.999.description': [
    'Заклинатель создаёт разряд молнии.',
    'मंत्रकर्ता विद्युत किरण उत्पन्न करता है।',
    'Perapal menciptakan sinar petir.',
    'Người thi triển tạo ra một tia sét.',
    'ผู้ร่ายสร้างลำแสงสายฟ้า',
    'ينشئ الساحر شعاعًا من البرق.'
  ],
  'content.item.1016.description': [
    'Постепенно восстанавливает здоровье вам или дружественной цели.',
    'आपका या मित्र लक्ष्य का स्वास्थ्य समय के साथ बहाल करता है।',
    'Memulihkan kesehatan diri sendiri atau sasaran sekutu secara bertahap.',
    'Hồi sinh lực theo thời gian cho bản thân hoặc mục tiêu đồng minh.',
    'ฟื้นฟูพลังชีวิตให้ตนเองหรือเป้าหมายฝ่ายเดียวกันอย่างต่อเนื่อง',
    'يستعيد صحتك أو صحة هدف حليف تدريجيًا مع الوقت.'
  ],
  'content.item.1017.description': [
    'Наполняет ваше оружие духовной силой.',
    'आपके हथियार में आत्मिक शक्ति भरता है।',
    'Mengisi senjata Anda dengan kekuatan roh.',
    'Truyền sức mạnh linh hồn vào vũ khí của bạn.',
    'อัดพลังวิญญาณเข้าสู่อาวุธของคุณ',
    'يشبع سلاحك بقوة الروح.'
  ],
  'content.item.1018.description': [
    'Отравляет цель вредоносным ядом.',
    'लक्ष्य पर हानिकारक विष डालता है।',
    'Memberikan racun berbahaya pada sasaran.',
    'Gây độc có hại lên mục tiêu.',
    'วางพิษอันตรายใส่เป้าหมาย',
    'يلقي سمًا ضارًا على الهدف.'
  ],
  'content.item.1019.description': [
    'Бросает в цель пылающий талисман, нанося урон огнём.',
    'लक्ष्य पर ज्वलंत ताबीज़ फेंकता है और अग्नि क्षति पहुँचाता है।',
    'Melempar jimat menyala ke sasaran dan menimbulkan kerusakan api.',
    'Phóng bùa lửa vào mục tiêu, gây sát thương lửa.',
    'ขว้างเครื่องรางเพลิงใส่เป้าหมายเพื่อสร้างความเสียหายไฟ',
    'يلقي تعويذة مشتعلة على الهدف لتسبب ضررًا ناريًا.'
  ],
  'content.item.1003.description': [
    'Есть шанс одним применением уничтожить нежить, соответствующую ограничению по уровню.',
    'स्तर की शर्त पूरी करने वाले मृतजीव को एक ही प्रयोग में मारने की संभावना है।',
    'Berpeluang membunuh mayat hidup yang memenuhi syarat tingkat dengan satu penggunaan.',
    'Có cơ hội tiêu diệt xác sống đáp ứng yêu cầu cấp độ chỉ bằng một lần thi triển.',
    'มีโอกาสกำจัดอันเดดที่ผ่านเงื่อนไขระดับได้ในการร่ายครั้งเดียว',
    'توجد فرصة لقتل مخلوق من الموتى الأحياء يستوفي شرط المستوى بإلقاء واحد.'
  ],
  'content.item.1004.description': [
    'Поглощает здоровье жертвы, восстанавливая здоровье заклинателя.',
    'लक्ष्य का स्वास्थ्य चूसकर मंत्रकर्ता का स्वास्थ्य बहाल करता है।',
    'Menyerap kesehatan korban untuk memulihkan kesehatan perapal.',
    'Hút sinh lực của nạn nhân để hồi sinh lực cho người thi triển.',
    'ดูดพลังชีวิตจากเหยื่อเพื่อฟื้นฟูพลังชีวิตของผู้ร่าย',
    'يمتص صحة الضحية لاستعادة صحة ملقي التعويذة.'
  ]
};

// Explicit keys from the v2 identical-value review. These must never become
// global English aliases: a short UI label can differ from an NPC/content name.
const reviewedCommon = new Map();
function reviewedCommonLines(table) {
  for (const line of table.trim().split('\n')) {
    const [keys, ...columns] = line.split('|').map((part) => part.trim());
    if (columns.length !== 6 || columns.some((value) => !value)) throw new Error(`Invalid common review: ${keys}`);
    const values = Object.fromEntries(locales.map((locale, i) => [locale, columns[i].replaceAll('\\n', '\n')]));
    for (const key of keys.split(',')) {
      if (reviewedCommon.has(key)) throw new Error(`Duplicate common review key: ${key}`);
      if (key.startsWith('content.') || key.startsWith('npc.')) throw new Error(`Out-of-scope common review: ${key}`);
      reviewedCommon.set(key, values);
    }
  }
}
reviewedCommonLines(String.raw`
client.AutoReelChance | Шанс автоподмотки + {0}% | डोरी अपने-आप समेटने की संभावना + {0}% | Peluang Gulung Otomatis + {0}% | Tỉ lệ tự thu dây + {0}% | โอกาสม้วนสายอัตโนมัติ + {0}% | احتمال لف الخيط تلقائيًا + {0}%
client.AutoRunOff | [Автобег: выкл.] | [स्वतः दौड़: बंद] | [Lari Otomatis: Mati] | [Tự chạy: Tắt] | [วิ่งอัตโนมัติ: ปิด] | [الركض التلقائي: متوقف]
client.AutoRunOn | [Автобег: вкл.] | [स्वतः दौड़: चालू] | [Lari Otomatis: Aktif] | [Tự chạy: Bật] | [วิ่งอัตโนมัติ: เปิด] | [الركض التلقائي: مفعل]
client.BagWeightPercent | Грузоподъёмность сумки + {0}% | थैले की भार क्षमता + {0}% | Kapasitas Beban Tas + {0}% | Sức chứa trọng lượng túi + {0}% | ความจุน้ำหนักกระเป๋า + {0}% | سعة حمل الحقيبة + {0}%
client.BagWeightPlus | Грузоподъёмность сумки + {0} | थैले की भार क्षमता + {0} | Kapasitas Beban Tas + {0} | Sức chứa trọng lượng túi + {0} | ความจุน้ำหนักกระเป๋า + {0} | سعة حمل الحقيبة + {0}
client.BigMapKey | Большая карта ({0}) | बड़ा नक्शा ({0}) | Peta Besar ({0}) | Bản đồ lớn ({0}) | แผนที่ใหญ่ ({0}) | الخريطة الكبيرة ({0})
client.BigmapOpenClose | Открыть/закрыть большую карту | बड़ा नक्शा खोलें/बंद करें | Buka/Tutup Peta Besar | Mở/đóng bản đồ lớn | เปิด/ปิดแผนที่ใหญ่ | فتح/إغلاق الخريطة الكبيرة
client.ExpPercent | Опыт + {0}% | अनुभव + {0}% | Pengalaman + {0}% | Kinh nghiệm + {0}% | ค่าประสบการณ์ + {0}% | الخبرة + {0}%
client.GameMaster | Администратор игры | खेल प्रशासक | Pengelola Gim | Quản trị viên trò chơi | ผู้ดูแลเกม | مدير اللعبة
client.ItemTypeMonsterSpawn | Яйцо призыва монстра | राक्षस आह्वान अंडा | Telur Pemanggil Monster | Trứng triệu hồi quái vật | ไข่อัญเชิญมอนสเตอร์ | بيضة استدعاء وحش
client.SoulboundTo | Привязано к душе: | आत्मा से बँधा है: | Terikat jiwa pada: | Ràng buộc linh hồn với: | ผูกวิญญาณกับ: | مرتبط بروح:
client.SoulBindsOnEquip | Привязывается к душе при надевании | पहनने पर आत्मा से बँधता है | Terikat jiwa saat dipakai | Ràng buộc linh hồn khi trang bị | ผูกวิญญาณเมื่อสวมใส่ | يرتبط بالروح عند التجهيز
client.Weight | Вес: | भार: | Berat: | Trọng lượng: | น้ำหนัก: | الوزن:
ui.home | В начало | आरंभ में जाएँ | Ke Awal | Về đầu | ไปจุดเริ่ม | إلى البداية
ui.end | В конец | अंत में जाएँ | Ke Akhir | Về cuối | ไปท้าย | إلى النهاية
client.AddAttackSpeed | Скорость атаки +{0} | आक्रमण गति +{0} | Kecepatan Serang +{0} | Tốc đánh +{0} | ความเร็วโจมตี +{0} | سرعة الهجوم +{0}
client.AddsAgilityPlus | Ловкость +{0} | चपलता +{0} | Kelincahan +{0} | Nhanh nhẹn +{0} | ความคล่องตัว +{0} | الرشاقة +{0}
client.AddsPoisonPlus | Отравление +{0} | विष प्रभाव +{0} | Efek Racun +{0} | Hiệu ứng độc +{0} | ผลพิษ +{0} | تأثير السم +{0}
client.AddsAccuracyValue | Точность +{0} | सटीकता +{0} | Akurasi +{0} | Chính xác +{0} | ความแม่นยำ +{0} | الدقة +{0}
client.AddsDurability | Прочность +{0} | टिकाऊपन +{0} | Ketahanan +{0} | Độ bền +{0} | ความทนทาน +{0} | المتانة +{0}
client.AddsFreezingPlus | Замораживание +{0} | हिम प्रभाव +{0} | Efek Pembekuan +{0} | Hiệu ứng đóng băng +{0} | ผลแช่แข็ง +{0} | تأثير التجميد +{0}
client.AttackMode_All | [Режим: атаковать всех] | [मोड: सभी पर आक्रमण] | [Mode: Serang Semua] | [Chế độ: Đánh tất cả] | [โหมด: โจมตีทั้งหมด] | [الوضع: مهاجمة الجميع]
client.AttackMode_EnemyGuild | [Режим: вражеская гильдия] | [मोड: शत्रु गिल्ड] | [Mode: Guild Musuh] | [Chế độ: Bang hội thù địch] | [โหมด: กิลด์ศัตรู] | [الوضع: النقابة المعادية]
client.AttackMode_Group | [Режим: группа] | [मोड: दल] | [Mode: Kelompok] | [Chế độ: Tổ đội] | [โหมด: ปาร์ตี้] | [الوضع: الفريق]
client.AttackMode_Guild | [Режим: гильдия] | [मोड: गिल्ड] | [Mode: Guild] | [Chế độ: Bang hội] | [โหมด: กิลด์] | [الوضع: النقابة]
client.AttackMode_RedBrown | [Режим: красные/коричневые имена] | [मोड: लाल/भूरे नाम] | [Mode: Nama Merah/Cokelat] | [Chế độ: Tên đỏ/nâu] | [โหมด: ชื่อแดง/น้ำตาล] | [الوضع: الأسماء الحمراء/البنية]
client.AttackSpeedValue | Скорость атаки: {0}{1} | आक्रमण गति: {0}{1} | Kecepatan Serang: {0}{1} | Tốc đánh: {0}{1} | ความเร็วโจมตี: {0}{1} | سرعة الهجوم: {0}{1}
client.BraveryGlyph | Глиф храбрости | वीरता चिह्न | Glif Keberanian | Phù văn dũng khí | อักขระความกล้า | نقش الشجاعة
client.EvilSlayerGlyph | Глиф истребителя зла | दुष्ट-वध चिह्न | Glif Pembasmi Kejahatan | Phù văn diệt tà | อักขระปราบมาร | نقش قاهر الشر
client.BodyGlyph | Глиф тела | देह चिह्न | Glif Tubuh | Phù văn thân thể | อักขระร่างกาย | نقش الجسد
client.MagicGlyph | Глиф магии | जादू चिह्न | Glif Sihir | Phù văn phép thuật | อักขระเวทมนตร์ | نقش السحر
client.SoulGlyph | Глиф души | आत्मा चिह्न | Glif Jiwa | Phù văn linh hồn | อักขระวิญญาณ | نقش الروح
client.ProtectionGlyph | Глиф защиты | सुरक्षा चिह्न | Glif Perlindungan | Phù văn bảo hộ | อักขระป้องกัน | نقش الحماية
client.Complete,server.TaskCompleted | (Завершено) | (पूर्ण) | (Selesai) | (Hoàn thành) | (สำเร็จ) | (مكتمل)
client.InProgress | (В процессе) | (जारी) | (Sedang Berlangsung) | (Đang thực hiện) | (กำลังดำเนินการ) | (قيد التنفيذ)
client.CreatureItemPickup | Подбор предметов питомцем | साथी द्वारा वस्तु उठाना | Pengambilan Barang oleh Peliharaan | Thú nuôi nhặt vật phẩm | สัตว์เลี้ยงเก็บไอเทม | التقاط المرافق للأدوات
client.CreatureAutoPickup | Автоподбор питомцем | साथी का स्वतः वस्तु उठाना | Pengambilan Otomatis oleh Peliharaan | Thú nuôi tự nhặt đồ | สัตว์เลี้ยงเก็บไอเทมอัตโนมัติ | التقاط المرافق التلقائي
client.CreaturesOpenClose | Открыть/закрыть питомцев | साथी खिड़की खोलें/बंद करें | Buka/Tutup Peliharaan | Mở/đóng thú nuôi | เปิด/ปิดหน้าต่างสัตว์เลี้ยง | فتح/إغلاق المرافقين
client.GuildOpenClose | Открыть/закрыть гильдию | गिल्ड खिड़की खोलें/बंद करें | Buka/Tutup Guild | Mở/đóng bang hội | เปิด/ปิดกิลด์ | فتح/إغلاق النقابة
client.Hero | (Герой) | (सहायक वीर) | (Pahlawan) | (Anh hùng) | (วีรบุรุษ) | (بطل)
client.Invisible | -Невидимость\n | -अदृश्य\n | -Tidak Terlihat\n | -Vô hình\n | -ล่องหน\n | -غير مرئي\n
client.KeybindsOpenClose | Открыть/закрыть назначение клавиш | कुंजी विन्यास खोलें/बंद करें | Buka/Tutup Pengaturan Tombol | Mở/đóng gán phím | เปิด/ปิดการตั้งค่าปุ่ม | فتح/إغلاق تعيين المفاتيح
client.MenteeExp | ОПЫТ УЧЕНИКА: | शिष्य का अनुभव: | PENGALAMAN MURID: | KINH NGHIỆM ĐỆ TỬ: | ค่าประสบการณ์ศิษย์: | خبرة المتدرب:
client.MENTOR | НАСТАВНИК | गुरु | PEMBIMBING | SƯ PHỤ | อาจารย์ | المرشد
client.MENTEE | УЧЕНИК | शिष्य | MURID | ĐỆ TỬ | ศิษย์ | المتدرب
client.MentorOpenClose | Открыть/закрыть наставничество | गुरु-शिष्य खिड़की खोलें/बंद करें | Buka/Tutup Pembimbing | Mở/đóng sư đồ | เปิด/ปิดอาจารย์และศิษย์ | فتح/إغلاق الإرشاد
client.Orbs | Сферы | गोले | Bola-Bola | Bảo châu | ลูกแก้ว | كرات
client.SellerExpiry | ПРОДАВЕЦ / СРОК | विक्रेता / समाप्ति | PENJUAL / KEDALUWARSA | NGƯỜI BÁN / HẠN | ผู้ขาย / วันหมดอายุ | البائع / الانتهاء
client.SkillbarOpenClose | Открыть/закрыть панель навыков | कौशल पट्टी खोलें/बंद करें | Buka/Tutup Bilah Keterampilan | Mở/đóng thanh kỹ năng | เปิด/ปิดแถบวิชา | فتح/إغلاق شريط المهارات
client.ToggleAutorun | Переключить автобег | स्वतः दौड़ चालू/बंद करें | Alihkan Lari Otomatis | Bật/tắt tự chạy | เปิด/ปิดวิ่งอัตโนมัติ | تبديل الركض التلقائي
client.ToggleAttackMode | Сменить режим атаки | आक्रमण मोड बदलें | Ganti Mode Serangan | Đổi chế độ tấn công | เปลี่ยนโหมดโจมตี | تبديل وضع الهجوم
client.ToggleCameraMode | Сменить режим камеры | कैमरा मोड बदलें | Ganti Mode Kamera | Đổi chế độ camera | เปลี่ยนโหมดกล้อง | تبديل وضع الكاميرا
client.TogglePetMode | Сменить режим питомца | साथी का मोड बदलें | Ganti Mode Peliharaan | Đổi chế độ thú nuôi | เปลี่ยนโหมดสัตว์เลี้ยง | تبديل وضع المرافق
client.BeltSlot | Ячейка пояса {0} | कमरबंध खाना {0} | Slot Sabuk {0} | Ô thắt lưng {0} | ช่องเข็มขัด {0} | خانة الحزام {0}
client.BeltSlotAlt | Доп. ячейка пояса {0} | वैकल्पिक कमरबंध खाना {0} | Slot Alternatif Sabuk {0} | Ô thắt lưng phụ {0} | ช่องเข็มขัดสำรอง {0} | خانة الحزام البديلة {0}
client.Bid | Ставка | बोली | Tawar | Ra giá | เสนอราคา | مزايدة
client.BidAmount | Сумма ставки: | बोली राशि: | Jumlah Tawaran: | Giá đấu: | จำนวนเงินเสนอราคา: | مبلغ المزايدة:
client.CasterName | \nПрименил: {0} | \nमंत्रकर्ता: {0} | \nPerapal: {0} | \nNgười thi triển: {0} | \nผู้ร่าย: {0} | \nملقي المهارة: {0}
client.CraftAmount | Количество для изготовления: | निर्माण संख्या: | Jumlah Pembuatan: | Số lượng chế tạo: | จำนวนที่จะผลิต: | كمية التصنيع:
client.Deposit | Внести | जमा करें | Setor | Gửi vào | ฝาก | إيداع
client.Downgrade | Понижение: | स्तर घटाना: | Penurunan: | Hạ cấp: | ลดระดับ: | تخفيض المستوى:
client.DuraPanel,ui.duraPanel | Панель прочности | टिकाऊपन पटल | Panel Ketahanan | Bảng độ bền | แผงความทนทาน | لوحة المتانة
client.Expire | Срок истекает: {0} | समाप्ति: {0} | Kedaluwarsa: {0} | Hết hạn: {0} | หมดอายุ: {0} | الانتهاء: {0}
client.Expiry | СРОК ДЕЙСТВИЯ | समाप्ति | KEDALUWARSA | HẠN DÙNG | วันหมดอายุ | تاريخ الانتهاء
client.FriendMemo | Заметка | टिप्पणी | Catatan | Ghi chú | บันทึก | ملاحظة
client.GameShopKey | Игровой магазин ({0}) | खेल दुकान ({0}) | Toko Gim ({0}) | Cửa hàng trò chơi ({0}) | ร้านค้าเกม ({0}) | متجر اللعبة ({0})
client.Gems | Самоцветы | रत्न | Permata | Ngọc | อัญมณี | جواهر
client.GuildKey | Гильдия ({0}) | गिल्ड ({0}) | Guild ({0}) | Bang hội ({0}) | กิลด์ ({0}) | النقابة ({0})
client.HeroBehaviourFormat | Поведение героя: {0} | सहायक वीर का व्यवहार: {0} | Perilaku Pahlawan: {0} | Hành vi anh hùng: {0} | พฤติกรรมวีรบุรุษ: {0} | سلوك البطل: {0}
client.HeroExperienceGained | Получен опыт героя: {0} | सहायक वीर का प्राप्त अनुभव: {0} | Pengalaman Pahlawan Diperoleh: {0} | Kinh nghiệm anh hùng nhận được: {0} | วีรบุรุษได้รับประสบการณ์: {0} | خبرة البطل المكتسبة: {0}
client.Heroic,client.ItemGradeHeroic | Героический | वीर श्रेणी | Kepahlawanan | Anh hùng | ระดับวีรบุรุษ | بطولي
client.HeroInventory | Сумка героя ({0}) | सहायक वीर की वस्तु-सूची ({0}) | Inventaris Pahlawan ({0}) | Túi anh hùng ({0}) | กระเป๋าวีรบุรุษ ({0}) | حقيبة البطل ({0})
client.Item | ПРЕДМЕТ | वस्तु | BARANG | VẬT PHẨM | ไอเทม | الأداة
client.Materials | Материалы | सामग्री | Bahan | Nguyên liệu | วัตถุดิบ | مواد
client.Max | Максимум | अधिकतम | Maksimum | Tối đa | สูงสุด | الحد الأقصى
`);

reviewedCommonLines(String.raw`
client.ItemTypeBells | Колокольчики | घंटियाँ | Lonceng | Chuông | กระดิ่ง | أجراس
client.ItemTypeCraftingMaterial | Материал изготовления | निर्माण सामग्री | Bahan Kerajinan | Nguyên liệu chế tạo | วัตถุดิบการผลิต | مادة تصنيع
client.ItemTypeDeco | Украшение | सजावट | Hiasan | Đồ trang trí | ของตกแต่ง | زينة
client.ItemTypeFinder | Детектор | खोजक | Pendeteksi | Máy dò | อุปกรณ์ตรวจจับ | كاشف
client.ItemTypeHook | Рыболовный крючок | मछली का काँटा | Kail | Lưỡi câu | เบ็ด | خطاف صيد
client.ItemTypeMask | Маска | मुखौटा | Topeng | Mặt nạ | หน้ากาก | قناع
client.ItemTypeReel | Рыболовная катушка | डोरी की चरखी | Penggulung Pancing | Máy cuốn dây câu | รอกตกปลา | بكرة صيد
client.ItemTypeReins | Поводья | लगाम | Tali Kekang | Dây cương | บังเหียน | لجام
client.ItemTypeRibbon | Лента | फीता | Pita | Ruy băng | ริบบิ้น | شريط
client.ItemTypeSaddle | Седло | काठी | Pelana | Yên | อาน | سرج
client.ItemTypeBait | Наживка | चारा | Umpan | Mồi | เหยื่อ | طعم
client.ItemTypeFloat | Поплавок | तैरता संकेतक | Pelampung | Phao | ทุ่น | عوامة
client.ItemTypePets | Питомцы | पालतू साथी | Peliharaan | Thú nuôi | สัตว์เลี้ยง | مرافقون
client.ItemTypeSealedHero | Запечатанный герой | मुहरबंद सहायक वीर | Pahlawan Tersegel | Anh hùng bị phong ấn | วีรบุรุษที่ถูกผนึก | بطل مختوم
client.ItemTypeTransform | Преображение | रूपांतरण | Perubahan Wujud | Biến hình | แปลงร่าง | تحول
client.Logout,ui.logout | Выйти из персонажа | पात्र से बाहर निकलें | Keluar dari Karakter | Thoát nhân vật | ออกจากตัวละคร | خروج من الشخصية
client.MailGoldLabel | Золото: | सोना: | Emas: | Vàng: | ทอง: | الذهب:
client.Menu,ui.menu | Меню | मेनू | Menu | Trình đơn | เมนู | القائمة
client.MiniMapKey | Мини-карта ({0}) | छोटा नक्शा ({0}) | Peta Mini ({0}) | Bản đồ nhỏ ({0}) | แผนที่ย่อ ({0}) | الخريطة المصغرة ({0})
client.MountKey | Ездовое животное ({0}) | सवारी ({0}) | Tunggangan ({0}) | Thú cưỡi ({0}) | สัตว์ขี่ ({0}) | دابة الركوب ({0})
client.NeedItemQuantity | {0}\nКоличество: {1} | {0}\nमात्रा: {1} | {0}\nJumlah: {1} | {0}\nSố lượng: {1} | {0}\nจำนวน: {1} | {0}\nالكمية: {1}
client.NutritionValue | Питательность {0} | पोषण {0} | Nutrisi {0} | Dinh dưỡng {0} | คุณค่าทางอาหาร {0} | القيمة الغذائية {0}
client.Observer | -Наблюдатель | -दर्शक | -Pengamat | -Người quan sát | -ผู้ชม | -مراقب
client.ONLINE | В СЕТИ | ऑनलाइन | DARING | TRỰC TUYẾN | ออนไลน์ | متصل
client.OpenSocketsTips | Ctrl + правая кнопка мыши: открыть гнёзда | Ctrl + दायाँ क्लिक: सॉकेट खोलें | Ctrl + Klik Kanan untuk Membuka Soket | Ctrl + nhấp phải để mở lỗ khảm | Ctrl + คลิกขวาเพื่อเปิดช่องฝัง | Ctrl + النقر بالزر الأيمن لفتح تجاويف الترصيع
client.OverallTop20 | Общий рейтинг: 20 лучших | समग्र शीर्ष 20 | Peringkat Umum 20 Teratas | 20 người đứng đầu toàn bộ | 20 อันดับแรกโดยรวม | أفضل 20 إجمالًا
client.PriceBid | ЦЕНА / СТАВКА | कीमत / बोली | HARGA / TAWARAN | GIÁ / GIÁ ĐẤU | ราคา / ราคาเสนอ | السعر / المزايدة
client.QuestReturn | Сдача задания | कार्य जमा करना | Penyerahan Misi | Nộp nhiệm vụ | ส่งภารกิจ | تسليم المهمة
client.Ranked | Место: {0} | स्थान: {0} | Peringkat: {0} | Thứ hạng: {0} | อันดับ: {0} | الترتيب: {0}
client.Refine | Улучшение: | परिष्करण: | Pemurnian: | Tinh luyện: | หลอมเสริม: | الصقل:
client.ReflectChance | Шанс отражения: {0} | परावर्तन की संभावना: {0} | Peluang Pantulan: {0} | Tỉ lệ phản lại: {0} | โอกาสสะท้อน: {0} | احتمال الانعكاس: {0}
client.RentalFeeGoldAmount | Плата за аренду: {0:###,###,##0} | किराया शुल्क: {0:###,###,##0} | Biaya Sewa: {0:###,###,##0} | Phí thuê: {0:###,###,##0} | ค่าเช่า: {0:###,###,##0} | رسوم الإيجار: {0:###,###,##0}
client.Reset | Сброс: | रीसेट: | Atur Ulang: | Đặt lại: | รีเซ็ต: | إعادة الضبط:
client.Sale | Продажа: | बिक्री: | Penjualan: | Bán: | ขาย: | البيع:
client.SellItem | ПРОДАТЬ ПРЕДМЕТ | वस्तु बेचें | JUAL BARANG | BÁN VẬT PHẨM | ขายไอเทม | بيع الأداة
client.Sender | ОТПРАВИТЕЛЬ | प्रेषक | PENGIRIM | NGƯỜI GỬI | ผู้ส่ง | المرسل
client.SkillbarAltSlot | Доп. ячейка навыка | वैकल्पिक कौशल खाना | Slot Alternatif Keterampilan | Ô kỹ năng phụ | ช่องวิชาสำรอง | خانة المهارة البديلة
client.SkillbarSlot | Ячейка навыка | कौशल खाना | Slot Keterampilan | Ô kỹ năng | ช่องวิชา | خانة المهارة
client.HeroSkillbarSlot | Ячейка навыка героя | सहायक वीर का कौशल खाना | Slot Keterampilan Pahlawan | Ô kỹ năng anh hùng | ช่องวิชาวีรบุรุษ | خانة مهارة البطل
client.Superman | -Сверхчеловек | -महामानव | -Manusia Super | -Siêu nhân | -ยอดมนุษย์ | -إنسان خارق
client.Warriors | Воины | योद्धा | Para Prajurit | Chiến binh | นักรบ | المحاربون
client.Archers | Лучники | धनुर्धर | Para Pemanah | Cung thủ | นักธนู | الرماة
client.Assasins | Ассасины | हत्यारे | Para Pembunuh | Sát thủ | มือสังหาร | القتلة
client.Taoists | Даосы | ताओवादी | Para Taois | Đạo sĩ | นักพรต | الطاويون
client.WarWizTao | Воин / Маг / Даос | योद्धा / जादूगर / ताओवादी | Prajurit / Penyihir / Taois | Chiến binh / Pháp sư / Đạo sĩ | นักรบ / นักเวท / นักพรต | محارب / ساحر / طاوي
client.WeddingRing | Обручальное кольцо | विवाह की अंगूठी | Cincin Pernikahan | Nhẫn cưới | แหวนแต่งงาน | خاتم الزواج
client.FoodBuff | Усиление от еды: {0} | भोजन का सुदृढ़ीकरण: {0} | Penguatan Makanan: {0} | Cường hóa thức ăn: {0} | บัฟอาหาร: {0} | تعزيز الطعام: {0}
client.MailHeaderNew | [Новое] | [नया] | [Baru] | [Mới] | [ใหม่] | [جديد]
client.MailLover | Письмо супругу | जीवनसाथी को पत्र भेजें | Kirim Surat ke Pasangan | Gửi thư cho bạn đời | ส่งจดหมายให้คู่สมรส | إرسال بريد إلى الشريك
client.MentorKey | Наставник ({0}) | गुरु ({0}) | Pembimbing ({0}) | Sư phụ ({0}) | อาจารย์ ({0}) | المرشد ({0})
client.NibbleChance | Шанс поклёвки + {0}~{1}% | मछली के चारा काटने की संभावना + {0}~{1}% | Peluang Ikan Menggigit + {0}~{1}% | Tỉ lệ cá cắn câu + {0}~{1}% | โอกาสปลากินเหยื่อ + {0}~{1}% | احتمال قضم السمكة للطعم + {0}~{1}%
client.Price | ЦЕНА | कीमत | HARGA | GIÁ | ราคา | السعر
client.Purchase | Купить | खरीदें | Beli | Mua | ซื้อ | شراء
client.RankTitle,server.RankNum | Ранг-{0} | पद-{0} | Peringkat-{0} | Hạng-{0} | ยศ-{0} | الرتبة-{0}
client.SalePrice | ЦЕНА ПРОДАЖИ | विक्रय मूल्य | HARGA JUAL | GIÁ BÁN | ราคาขาย | سعر البيع
client.Type | ТИП | प्रकार | JENIS | LOẠI | ประเภท | النوع
client.ItemGradeRare,client.Rare | Редкий | दुर्लभ | Langka | Hiếm | หายาก | نادر
client.ActivationCostGold | Стоимость активации: {0} золота. | सक्रिय करने की लागत: {0} सोना। | Biaya Aktivasi: {0} emas. | Phí kích hoạt: {0} vàng. | ค่าเปิดใช้งาน: {0} ทอง | تكلفة التفعيل: {0} ذهب.
client.AndPlaceholder | и {0} | और {0} | dan {0} | và {0} | และ {0} | و{0}
client.BuffPots | Зелья усиления | सुदृढ़ीकरण औषधियाँ | Ramuan Penguatan | Bình thuốc cường hóa | ยาเสริมพลัง | جرعات التعزيز
client.CriticalChancePlus | Шанс критического удара: + {0} | गंभीर प्रहार की संभावना: + {0} | Peluang Serangan Kritis: + {0} | Tỉ lệ chí mạng: + {0} | โอกาสคริติคอล: + {0} | احتمال الضربة الحرجة: + {0}
client.CtrlRightClick | Ctrl + правая кнопка мыши | Ctrl + दायाँ क्लिक | Ctrl + Klik Kanan | Ctrl + nhấp phải | Ctrl + คลิกขวา | Ctrl + النقر بالزر الأيمن
client.DeletingCharactersDisabled | Удаление персонажей сейчас отключено. | अभी पात्र हटाना बंद है। | Penghapusan karakter sedang dinonaktifkan. | Hiện không thể xóa nhân vật. | ขณะนี้ปิดการลบตัวละคร | حذف الشخصيات معطل حاليًا.
client.EditRank | Изменить ранг | पद बदलें | Ubah Pangkat | Sửa chức vụ | แก้ไขยศ | تعديل الرتبة
client.ExpirePaused | Срок действия: приостановлен | समाप्ति: स्थगित | Kedaluwarsa: Dijeda | Hạn dùng: Tạm dừng | วันหมดอายุ: หยุดชั่วคราว | الانتهاء: معلق
client.ExpiresIn | Истекает через {0} | {0} में समाप्त होगा | Kedaluwarsa dalam {0} | Hết hạn sau {0} | หมดอายุใน {0} | ينتهي خلال {0}
client.FreezingPlus | Замораживание: + {0} | हिम प्रभाव: + {0} | Pembekuan: + {0} | Đóng băng: + {0} | แช่แข็ง: + {0} | التجميد: + {0}
client.GTPrice | Цена территории гильдии | गिल्ड क्षेत्र की कीमत | Harga Wilayah Guild | Giá lãnh địa bang hội | ราคาอาณาเขตกิลด์ | سعر أرض النقابة
client.GTStatus | Статус территории гильдии | गिल्ड क्षेत्र की स्थिति | Keadaan Wilayah Guild | Trạng thái lãnh địa bang hội | สถานะอาณาเขตกิลด์ | حالة أرض النقابة
client.HandWeightPlus | Допустимый вес в руках + {0} | हाथों की भार क्षमता + {0} | Kapasitas Beban Tangan + {0} | Sức chịu trọng lượng tay + {0} | ความจุน้ำหนักมือ + {0} | سعة الحمل باليد + {0}
client.HighestBid | НАИВЫСШАЯ СТАВКА | सबसे ऊँची बोली | TAWARAN TERTINGGI | GIÁ ĐẤU CAO NHẤT | ราคาเสนอสูงสุด | أعلى مزايدة
client.Holy | Священная сила: + {0} | पवित्र शक्ति: + {0} | Kekuatan Suci: + {0} | Thánh lực: + {0} | พลังศักดิ์สิทธิ์: + {0} | القوة المقدسة: + {0}
client.HpMpMode1 | [Режим HP/MP 1] | [HP/MP मोड 1] | [Mode HP/MP 1] | [Chế độ HP/MP 1] | [โหมด HP/MP 1] | [وضع HP/MP 1]
client.HpMpMode2 | [Режим HP/MP 2] | [HP/MP मोड 2] | [Mode HP/MP 2] | [Chế độ HP/MP 2] | [โหมด HP/MP 2] | [وضع HP/MP 2]
client.LengthDays | Длительность: {0} дн. | अवधि: {0} दिन | Durasi: {0} hari | Thời hạn: {0} ngày | ระยะเวลา: {0} วัน | المدة: {0} يوم
client.LoverLength | Длительность: | अवधि: | Durasi: | Thời gian: | ระยะเวลา: | المدة:
client.MagicResistPlus | Сопротивление магии + {0} | जादू प्रतिरोध + {0} | Ketahanan Sihir + {0} | Kháng phép + {0} | ต้านทานเวทมนตร์ + {0} | مقاومة السحر + {0}
client.MailGuildLeader | Письмо главе гильдии | गिल्ड प्रमुख को पत्र भेजें | Kirim Surat ke Ketua Guild | Gửi thư cho bang chủ | ส่งจดหมายให้หัวหน้ากิลด์ | إرسال بريد إلى قائد النقابة
client.Message | СООБЩЕНИЕ | संदेश | PESAN | TIN NHẮN | ข้อความ | الرسالة
client.PurityValue | Чистота {0} | शुद्धता {0} | Kemurnian {0} | Độ tinh khiết {0} | ความบริสุทธิ์ {0} | النقاء {0}
client.Repair | Ремонт: | मरम्मत: | Perbaikan: | Sửa chữa: | ซ่อมแซม: | الإصلاح:
client.SelectRank | Выбрать ранг | पद चुनें | Pilih Pangkat | Chọn chức vụ | เลือกยศ | اختيار الرتبة
client.SellingPriceGold | Цена продажи: {0} золота | विक्रय मूल्य: {0} सोना | Harga Jual: {0} emas | Giá bán: {0} vàng | ราคาขาย: {0} ทอง | سعر البيع: {0} ذهب
client.SkillModeCtrl | [Режим навыков: Ctrl] | [कौशल मोड: Ctrl] | [Mode Keterampilan: Ctrl] | [Chế độ kỹ năng: Ctrl] | [โหมดวิชา: Ctrl] | [وضع المهارات: Ctrl]
client.SkillsOpenCloseAlt | Открыть/закрыть навыки (доп.) | कौशल खोलें/बंद करें (वैकल्पिक) | Buka/Tutup Keterampilan (Alternatif) | Mở/đóng kỹ năng (phụ) | เปิด/ปิดวิชา (สำรอง) | فتح/إغلاق المهارات (بديل)
client.SocketWithValue | Гнездо: {0} | सॉकेट: {0} | Soket: {0} | Lỗ khảm: {0} | ช่องฝัง: {0} | تجويف ترصيع: {0}
client.SpecialRepair | Особый ремонт: | विशेष मरम्मत: | Perbaikan Khusus: | Sửa chữa đặc biệt: | ซ่อมแซมพิเศษ: | إصلاح خاص:
client.SubmitBug | Отправить ошибку | त्रुटि भेजें | Kirim Laporan Galat | Gửi báo lỗi | ส่งรายงานข้อผิดพลาด | إرسال بلاغ خلل
client.UsageCurrentMax | Использовано {0}/{1} | उपयोग {0}/{1} | Penggunaan {0}/{1} | Đã dùng {0}/{1} | ใช้ไป {0}/{1} | الاستخدام {0}/{1}
client.WhisperLover | Личное сообщение супругу | जीवनसाथी से निजी बातचीत | Bisik ke Pasangan | Nhắn riêng bạn đời | กระซิบหาคู่สมรส | همس إلى الشريك
`);

reviewedCommonLines(String.raw`
log.recv | получено {0} | प्राप्त {0} | diterima {0} | nhận {0} | รับ {0} | استلام {0}
server.BuffsLoaded | Загружено усилений: {0}. | {0} सुदृढ़ीकरण लोड हुए। | {0} penguatan dimuat. | Đã tải {0} hiệu ứng cường hóa. | โหลดบัฟ {0} รายการแล้ว | تم تحميل {0} من التعزيزات.
server.NoGTFoundContactGM | Территории гильдий не найдены. Обратитесь к администратору игры! | गिल्ड क्षेत्र नहीं मिले। खेल प्रशासक से संपर्क करें! | Wilayah guild tidak ditemukan. Hubungi pengelola gim! | Không tìm thấy lãnh địa bang hội. Hãy liên hệ quản trị viên! | ไม่พบอาณาเขตกิลด์ กรุณาติดต่อผู้ดูแลเกม! | لم يتم العثور على أراض للنقابات. تواصل مع مدير اللعبة!
server.NoGuildTerritoryAvailable | Нет доступных территорий гильдий! | कोई गिल्ड क्षेत्र उपलब्ध नहीं है! | Tidak ada wilayah guild yang tersedia! | Không có lãnh địa bang hội khả dụng! | ไม่มีอาณาเขตกิลด์ว่าง! | لا توجد أراض متاحة للنقابات!
server.NoMount | Нет ездового животного | सवारी नहीं है | Tidak Ada Tunggangan | Không có thú cưỡi | ไม่มีสัตว์ขี่ | لا توجد دابة ركوب
server.PetBurp | *Отрыжка* | *डकार* | *Sendawa* | *Ợ* | *เรอ* | *تجشؤ*
server.PetPickedUp | Питомец подобрал: {{{0}}} | साथी ने उठाया: {{{0}}} | Peliharaan mengambil: {{{0}}} | Thú nuôi đã nhặt: {{{0}}} | สัตว์เลี้ยงเก็บได้: {{{0}}} | التقط المرافق: {{{0}}}
ui.onchainMine.optimisticOre | Предварительная руда | अनुमानित अयस्क | Bijih Perkiraan | Quặng tạm tính | แร่ที่คาดการณ์ | الخام التقديري
ui.onchainMine.veinStage.depleted | истощена | समाप्त | habis | cạn kiệt | หมดแล้ว | مستنفد
ui.questAbandon | Отказаться от задания | कार्य छोड़ें | Tinggalkan Misi | Bỏ nhiệm vụ | ละทิ้งภารกิจ | التخلي عن المهمة
ui.reconnectConnecting | Переподключение... | फिर से जुड़ रहा है... | Menghubungkan ulang... | Đang kết nối lại... | กำลังเชื่อมต่อใหม่... | جارٍ إعادة الاتصال...
ui.spells | Заклинания | मंत्र | Mantra | Pháp thuật | เวทมนตร์ | التعويذات
custom.interaction.questHint | Задание: {0} | कार्य: {0} | Misi: {0} | Nhiệm vụ: {0} | ภารกิจ: {0} | المهمة: {0}
log.gatewayError | ошибка шлюза: {0} | गेटवे त्रुटि: {0} | galat gerbang: {0} | lỗi cổng kết nối: {0} | ข้อผิดพลาดเกตเวย์: {0} | خطأ البوابة: {0}
log.gatewayWsOpen | WebSocket шлюза открыт | गेटवे WebSocket खुला | WebSocket gerbang terbuka | WebSocket của cổng đã mở | WebSocket ของเกตเวย์เปิดแล้ว | اتصال WebSocket للبوابة مفتوح
runtime.phase.boot-error | Ошибка запуска | आरंभ त्रुटि | Galat Memulai | Lỗi khởi động | ข้อผิดพลาดการเริ่มต้น | خطأ بدء التشغيل
server.Conquest | Завоевание: {0} | विजय अभियान: {0} | Penaklukan: {0} | Chinh phục: {0} | ศึกยึดครอง: {0} | الاستيلاء: {0}
server.ConquestGuard | Страж завоевания | विजय रक्षक | Penjaga Penaklukan | Hộ vệ chinh phục | ยามศึกยึดครอง | حارس الاستيلاء
server.ConquestNotFound | Завоевание не найдено | विजय अभियान नहीं मिला | Penaklukan Tidak Ditemukan | Không tìm thấy cuộc chinh phục | ไม่พบศึกยึดครอง | لم يتم العثور على الاستيلاء
server.DropsLoaded | Данные добычи загружены. | लूट का डेटा लोड हुआ। | Data jarahan dimuat. | Đã tải dữ liệu rơi đồ. | โหลดข้อมูลไอเทมตกแล้ว | تم تحميل بيانات الغنائم.
server.DropsReloaded | Данные добычи перезагружены. | लूट का डेटा फिर लोड हुआ। | Data jarahan dimuat ulang. | Đã tải lại dữ liệu rơi đồ. | โหลดข้อมูลไอเทมตกใหม่แล้ว | تمت إعادة تحميل بيانات الغنائم.
server.Flag | Флаг {0} | ध्वज {0} | Bendera {0} | Cờ {0} | ธง {0} | العلم {0}
server.GameMasterMode | Режим администратора игры. | खेल प्रशासक मोड। | Mode pengelola gim. | Chế độ quản trị viên trò chơi. | โหมดผู้ดูแลเกม | وضع مدير اللعبة.
server.Guild | Гильдия | गिल्ड | Guild | Bang hội | กิลด์ | النقابة
server.HeroOwnerReceiveChat | [Герой: {0}] {1} | [सहायक वीर: {0}] {1} | [Pahlawan: {0}] {1} | [Anh hùng: {0}] {1} | [วีรบุรุษ: {0}] {1} | [البطل: {0}] {1}
server.MapsLoaded | Загружено карт: {0}. | {0} नक्शे लोड हुए। | {0} peta dimuat. | Đã tải {0} bản đồ. | โหลดแผนที่ {0} รายการแล้ว | تم تحميل {0} من الخرائط.
server.NpcInfoTitle | --Сведения об NPC-- | --NPC जानकारी-- | --Informasi NPC-- | --Thông tin NPC-- | --ข้อมูล NPC-- | --معلومات NPC--
server.PlayerInfoTitle | --Сведения об игроке-- | --खिलाड़ी जानकारी-- | --Informasi Pemain-- | --Thông tin người chơi-- | --ข้อมูลผู้เล่น-- | --معلومات اللاعب--
server.UpperNoGuild | НЕТ ГИЛЬДИИ | कोई गिल्ड नहीं | TIDAK ADA GUILD | KHÔNG CÓ BANG HỘI | ไม่มีกิลด์ | لا توجد نقابة
sim.castSkill | Применено: {0}. | प्रयोग किया: {0}। | Menggunakan {0}. | Thi triển {0}. | ใช้ {0} | تم إلقاء {0}.
sim.echoChat | эхо: {0} | प्रतिध्वनि: {0} | gema: {0} | lặp lại: {0} | สะท้อน: {0} | صدى: {0}
sim.equippedItem | Надето: {0}. | पहना: {0}। | Memakai {0}. | Đã trang bị {0}. | สวมใส่ {0} แล้ว | تم تجهيز {0}.
ui.audio | Звук | ध्वनि | Suara | Âm thanh | เสียง | الصوت
ui.deleteItem | Удалить предмет | वस्तु हटाएँ | Hapus Barang | Xóa vật phẩm | ลบไอเทม | حذف الأداة
ui.gameShop | Игровой магазин | खेल दुकान | Toko Gim | Cửa hàng trò chơi | ร้านค้าเกม | متجر اللعبة
ui.monster | Монстр | राक्षस | Monster | Quái vật | มอนสเตอร์ | الوحش
ui.onchainMine.swing | Удар киркой (отладка) | कुदाल का प्रहार (डीबग) | Ayun Beliung (Debug) | Vung cuốc (gỡ lỗi) | เหวี่ยงพลั่ว (ดีบัก) | ضربة معول (تصحيح)
ui.onchainMine.vein | Рудная жила | अयस्क शिरा | Urat Bijih | Mạch quặng | สายแร่ | عرق الخام
ui.questObjective | Цель задания | कार्य का लक्ष्य | Tujuan Misi | Mục tiêu nhiệm vụ | เป้าหมายภารกิจ | هدف المهمة
ui.questRewardCredit | Кредиты: {0} | क्रेडिट: {0} | Kredit: {0} | Điểm: {0} | เครดิต: {0} | الرصيد: {0}
ui.runtime | Среда выполнения | निष्पादन परिवेश | Lingkungan Eksekusi | Môi trường thực thi | สภาพแวดล้อมการทำงาน | بيئة التشغيل
ui.self | Вы | स्वयं | Diri Sendiri | Bản thân | ตนเอง | الذات
ui.statsPrimary | Характеристики 1 | आँकड़े 1 | Atribut 1 | Chỉ số 1 | ค่าสถานะ 1 | الإحصاءات 1
ui.statsSecondary | Характеристики 2 | आँकड़े 2 | Atribut 2 | Chỉ số 2 | ค่าสถานะ 2 | الإحصاءات 2
ui.target | Цель | लक्ष्य | Sasaran | Mục tiêu | เป้าหมาย | الهدف
common.6df1bb18a59a | Навык | कौशल | Keterampilan | Kỹ năng | วิชา | المهارة
common.822bab8d41bc | Количество | मात्रा | Jumlah | Số lượng | จำนวน | الكمية
client.BuffEffect,client.ValueByOwnerPercent | {0} {1} на: {2}{3}.\n | {0} {1}, मात्रा: {2}{3}.\n | {0} {1} sebesar: {2}{3}.\n | {0} {1} với mức: {2}{3}.\n | {0} {1} เป็นจำนวน: {2}{3}\n | {0} {1} بمقدار: {2}{3}.\n
ui.tick | Такт | चक्र | Langkah Simulasi | Nhịp xử lý | รอบประมวลผล | دورة المعالجة
client.ReceiveDamageEveryTick,client.RecieveDamageEveryTime | Получает {0} урона каждые {1} {2}.\n | हर {1} {2} पर {0} क्षति प्राप्त होती है।\n | Menerima {0} kerusakan setiap {1} {2}.\n | Nhận {0} sát thương mỗi {1} {2}.\n | ได้รับความเสียหาย {0} ทุก {1} {2}\n | يتلقى {0} ضرر كل {1} {2}.\n
runtime.phase.idle,sim.npcIdleTitle | Ожидание | निष्क्रिय | Siaga | Chờ | รอ | انتظار
server.ActivateFlag | Активировать флаг {0} {1} | ध्वज सक्रिय करें {0} {1} | Aktifkan Bendera {0} {1} | Kích hoạt cờ {0} {1} | เปิดใช้งานธง {0} {1} | تفعيل العلم {0} {1}
server.AuctionSoldAndExpired,server.AuctionSoldExpired | Аукцион, продано и истекло: {0} | नीलामी में बिके और समाप्त: {0} | Lelang terjual dan kedaluwarsa: {0} | Đấu giá đã bán và hết hạn: {0} | ประมูลที่ขายแล้วและหมดอายุ: {0} | المزاد، المبيع والمنتهي: {0}
server.ConquestSiege | Осада завоевания | विजय की घेराबंदी | Pengepungan Penaklukan | Công thành chinh phục | การล้อมศึกยึดครอง | حصار الاستيلاء
server.ConquestWall | Стена завоевания | विजय प्राचीर | Tembok Penaklukan | Tường thành chinh phục | กำแพงศึกยึดครอง | جدار الاستيلاء
server.ExpireOnExtendFee | Истекает: {0}, плата за продление: {1} | समाप्ति: {0}, अवधि बढ़ाने का शुल्क: {1} | Kedaluwarsa: {0}, biaya perpanjangan: {1} | Hết hạn: {0}, phí gia hạn: {1} | หมดอายุ: {0}, ค่าต่อเวลา: {1} | ينتهي في: {0}، رسوم التمديد: {1}
server.FailedMovementToMap1 | Не удалось перейти на карту {0} | नक्शे {0} पर जाने में विफल | Gagal berpindah ke peta {0} | Không thể chuyển sang bản đồ {0} | ย้ายไปแผนที่ {0} ไม่สำเร็จ | تعذر الانتقال إلى الخريطة {0}
server.FailedMovementToMap2 | Не удалось перейти на карту {0}:[{1}] | नक्शे {0}:[{1}] पर जाने में विफल | Gagal berpindah ke peta {0}:[{1}] | Không thể chuyển sang bản đồ {0}:[{1}] | ย้ายไปแผนที่ {0}:[{1}] ไม่สำเร็จ | تعذر الانتقال إلى الخريطة {0}:[{1}]
server.FailedMovementToMap3 | Не удалось перейти на карту {0} в {1}:{2} | नक्शे {0} पर {1}:{2} जाने में विफल | Gagal berpindah ke peta {0} pada {1}:{2} | Không thể chuyển sang bản đồ {0} tại {1}:{2} | ย้ายไปแผนที่ {0} ที่ {1}:{2} ไม่สำเร็จ | تعذر الانتقال إلى الخريطة {0} عند {1}:{2}
server.FailedMovementToMap4 | Не удалось перейти на карту {0}:[{1}] в {2}:{3} | नक्शे {0}:[{1}] पर {2}:{3} जाने में विफल | Gagal berpindah ke peta {0}:[{1}] pada {2}:{3} | Không thể chuyển sang bản đồ {0}:[{1}] tại {2}:{3} | ย้ายไปแผนที่ {0}:[{1}] ที่ {2}:{3} ไม่สำเร็จ | تعذر الانتقال إلى الخريطة {0}:[{1}] عند {2}:{3}
server.GatesRepaired | Отремонтировано ворот: {0}/{1} | मरम्मत हुए द्वार: {0}/{1} | Gerbang diperbaiki: {0}/{1} | Đã sửa cổng: {0}/{1} | ซ่อมประตูแล้ว: {0}/{1} | البوابات التي أصلحت: {0}/{1}
server.LosingControlOf | Утрата контроля над {1}: {0:P0} | {1} पर नियंत्रण खो रहा है: {0:P0} | Kehilangan kendali atas {1}: {0:P0} | Mất kiểm soát {1}: {0:P0} | สูญเสียการควบคุม {1}: {0:P0} | فقدان السيطرة على {1}: {0:P0}
server.MapCouldNotBeFound | Карта {0}:[{1}] не найдена | नक्शा {0}:[{1}] नहीं मिला | Peta {0}:[{1}] tidak ditemukan | Không tìm thấy bản đồ {0}:[{1}] | ไม่พบแผนที่ {0}:[{1}] | لم يتم العثور على الخريطة {0}:[{1}]
server.MentalstateGroupMode | Состояние духа: режим группы. | मानसिक अवस्था: दल मोड। | Kondisi Batin: mode kelompok. | Tâm cảnh: chế độ tổ đội. | สภาวะจิต: โหมดปาร์ตี้ | الحالة الذهنية: وضع الفريق.
server.MonsterInfo1,server.NpcInfo1 | ID: {0}, имя: {1} | ID: {0}, नाम: {1} | ID: {0}, nama: {1} | ID: {0}, tên: {1} | ID: {0}, ชื่อ: {1} | ID: {0}، الاسم: {1}
server.MonsterInfo2 | Уровень: {0}, X: {1}, Y: {2}, направление: {3} | स्तर: {0}, X: {1}, Y: {2}, दिशा: {3} | Level: {0}, X: {1}, Y: {2}, arah: {3} | Cấp: {0}, X: {1}, Y: {2}, hướng: {3} | ระดับ: {0}, X: {1}, Y: {2}, ทิศ: {3} | المستوى: {0}، X: {1}، Y: {2}، الاتجاه: {3}
server.MonsterInfoTitle | --Сведения о монстре-- | --राक्षस जानकारी-- | --Informasi Monster-- | --Thông tin quái vật-- | --ข้อมูลมอนสเตอร์-- | --معلومات الوحش--
server.NewAccountBeingCreated | {0}, {1}, создаётся новая учётная запись. | {0}, {1}, नया खाता बनाया जा रहा है। | {0}, {1}, akun baru sedang dibuat. | {0}, {1}, đang tạo tài khoản mới. | {0}, {1}, กำลังสร้างบัญชีใหม่ | {0}، {1}، جارٍ إنشاء حساب جديد.
server.NoBracelet | Нет браслета | कंगन नहीं है | Tidak Ada Gelang | Không có vòng tay | ไม่มีกำไล | لا يوجد سوار
server.NoGuild | Нет гильдии | गिल्ड नहीं है | Tidak Ada Guild | Không có bang hội | ไม่มีกิลด์ | لا توجد نقابة
server.NoNecklace | Нет ожерелья | हार नहीं है | Tidak Ada Kalung | Không có dây chuyền | ไม่มีสร้อยคอ | لا توجد قلادة
server.NoWarScheduled | Война не запланирована | कोई युद्ध तय नहीं है | Tidak Ada Perang Terjadwal | Chưa có lịch chiến tranh | ยังไม่กำหนดสงคราม | لا توجد حرب مجدولة
server.NpcInfo2 | X : {0}, Y : {1} | X : {0}, Y : {1} | X : {0}, Y : {1} | X : {0}, Y : {1} | X : {0}, Y : {1} | X : {0}, Y : {1}
server.NpcScriptsReloaded | Скрипты NPC перезагружены. | NPC स्क्रिप्ट फिर लोड हुईं। | Skrip NPC dimuat ulang. | Đã tải lại kịch bản NPC. | โหลดสคริปต์ NPC ใหม่แล้ว | تمت إعادة تحميل نصوص NPC البرمجية.
server.SiegesRepaired | Отремонтировано осадных объектов: {0}/{1} | मरम्मत हुई घेराबंदी इकाइयाँ: {0}/{1} | Unit pengepungan diperbaiki: {0}/{1} | Đã sửa đơn vị công thành: {0}/{1} | ซ่อมหน่วยล้อมเมืองแล้ว: {0}/{1} | وحدات الحصار التي أصلحت: {0}/{1}
server.SpellLearntSetLevelByGM | {0}: заклинание {1} изучено и установлено на уровень {2} администратором игры {3} | {0}: मंत्र {1} सिखाया और स्तर {2} किया गया, खेल प्रशासक: {3} | {0}: mantra {1} dipelajari dan diatur ke level {2} oleh pengelola gim {3} | {0}: học phép {1} và đặt cấp {2} bởi quản trị viên {3} | {0}: เรียนเวท {1} และกำหนดระดับ {2} โดยผู้ดูแลเกม {3} | {0}: تم تعلم التعويذة {1} وضبطها على المستوى {2} بواسطة مدير اللعبة {3}
server.SyntaxResetConquest | Формат команды: /ResetConquest [ConquestID] | कमांड का प्रारूप: /ResetConquest [ConquestID] | Format perintah: /ResetConquest [ConquestID] | Cú pháp: /ResetConquest [ConquestID] | รูปแบบคำสั่ง: /ResetConquest [ConquestID] | صيغة الأمر: /ResetConquest [ConquestID]
server.SyntaxStartConquest | Формат команды: /StartConquest [ConquestID] | कमांड का प्रारूप: /StartConquest [ConquestID] | Format perintah: /StartConquest [ConquestID] | Cú pháp: /StartConquest [ConquestID] | รูปแบบคำสั่ง: /StartConquest [ConquestID] | صيغة الأمر: /StartConquest [ConquestID]
server.UnableToDisassemble | Не удалось разобрать {0} | {0} को विघटित नहीं कर सकते | Tidak dapat membongkar {0} | Không thể phân rã {0} | ไม่สามารถแยกชิ้นส่วน {0} | تعذر تفكيك {0}
sim.questProgressWasps | Прогресс задания: побеждено полевых ос {0}/{1}. | कार्य प्रगति: {0}/{1} मैदानी ततैये हराए। | Kemajuan misi: {0}/{1} tawon padang dikalahkan. | Tiến độ nhiệm vụ: đã hạ {0}/{1} ong bắp cày đồng. | ความคืบหน้าภารกิจ: กำจัดตัวต่อทุ่งแล้ว {0}/{1} | تقدم المهمة: تمت هزيمة {0}/{1} من دبابير الحقول.
sim.questReturnForReward | Задание обновлено: вернитесь к проводнику деревни за наградой. | कार्य बदला: पुरस्कार लेने गाँव के मार्गदर्शक के पास लौटें। | Misi diperbarui: kembali ke Pemandu Desa untuk mengambil hadiah. | Nhiệm vụ đã cập nhật: trở lại người hướng dẫn làng để nhận thưởng. | อัปเดตภารกิจแล้ว: กลับไปหาผู้นำทางหมู่บ้านเพื่อรับรางวัล | تم تحديث المهمة: عد إلى دليل القرية للحصول على مكافأتك.
sim.repairedEquippedItems | Отремонтировано надетых предметов: {0}. | पहनी हुई {0} वस्तुओं की मरम्मत हुई। | Memperbaiki {0} barang yang dipakai. | Đã sửa {0} vật phẩm đang trang bị. | ซ่อมไอเทมที่สวมใส่ {0} ชิ้นแล้ว | تم إصلاح {0} من الأدوات المجهزة.
ui.characterSlotFallback | Персонаж {0} | पात्र {0} | Karakter {0} | Nhân vật {0} | ตัวละคร {0} | الشخصية {0}
ui.connect | Подключиться | जुड़ें | Hubungkan | Kết nối | เชื่อมต่อ | اتصال
ui.gamepadNavigationHint | Крестовина: навигация · {0}: выбрать · {1}: назад | दिशापैड: आगे-पीछे जाएँ · {0}: चुनें · {1}: वापस | Tombol Arah: navigasi · {0}: pilih · {1}: kembali | Phím hướng: di chuyển · {0}: chọn · {1}: quay lại | ปุ่มทิศทาง: เลื่อนเลือก · {0}: เลือก · {1}: กลับ | لوحة الاتجاهات: تنقل · {0}: اختيار · {1}: رجوع
ui.onchainMine.confirmed | Подтверждено в блокчейне | ब्लॉकचेन पर पुष्टि हुई | Dikonfirmasi di Blockchain | Đã xác nhận trên chuỗi | ยืนยันบนบล็อกเชนแล้ว | مؤكد على سلسلة الكتل
ui.onchainMine.reconcilePhantom | Удалена неподтверждённая руда: {0} | अपुष्ट अयस्क हटाया: {0} | Bijih tak terkonfirmasi dihapus: {0} | Đã xóa quặng chưa xác nhận: {0} | ลบแร่ที่ยังไม่ยืนยัน: {0} | تمت إزالة الخام غير المؤكد: {0}
ui.onchainMine.reconcileShortfall | Добавлено +{0} | जोड़ा +{0} | Ditambahkan +{0} | Đã thêm +{0} | เพิ่มแล้ว +{0} | تمت إضافة +{0}
ui.onchainMine.revokeSession | Отозвать | निरस्त करें | Cabut | Thu hồi | เพิกถอน | إلغاء التفويض
ui.questReward | Награда | पुरस्कार | Hadiah | Phần thưởng | รางวัล | المكافأة
ui.questRewardGold | Золото: {0} | सोना: {0} | Emas: {0} | Vàng: {0} | ทอง: {0} | الذهب: {0}
ui.quickEnter | Быстрый вход | त्वरित प्रवेश | Masuk Cepat | Vào nhanh | เข้าอย่างรวดเร็ว | دخول سريع
ui.scene | Сцена | दृश्य | Adegan | Cảnh | ฉาก | المشهد
ui.toggleMiniMap | Показать/скрыть мини-карту | छोटा नक्शा दिखाएँ/छिपाएँ | Tampilkan/Sembunyikan Peta Mini | Hiện/ẩn bản đồ nhỏ | แสดง/ซ่อนแผนที่ย่อ | إظهار/إخفاء الخريطة المصغرة
common.ee7da936b5cb | Положить на хранение | भंडार में रखें | Simpan | Cất kho | เก็บเข้าคลัง | إيداع في المخزن
common.bec69036aa27 | Упорядочить | क्रम में लगाएँ | Urutkan | Sắp xếp | จัดเรียง | ترتيب
common.d00aae6b7fbf | Магазин | दुकान | Toko | Cửa hàng | ร้านค้า | المتجر
`);

reviewedCommonLines(String.raw`
client.AfterArmour | -Броня | -कवच | -Zirah | -Áo giáp | -เกราะ | -درع
client.AfterBracelet | -Браслет | -कंगन | -Gelang | -Vòng tay | -กำไล | -سوار
client.AfterCandle | -Свеча | -मोमबत्ती | -Lilin | -Nến | -เทียน | -شمعة
client.AfterRing | -Кольцо | -अंगूठी | -Cincin | -Nhẫn | -แหวน | -خاتم
client.AfterWeapon | -Оружие | -हथियार | -Senjata | -Vũ khí | -อาวุธ | -سلاح
client.AfterBelt | -Пояс | -कमरबंध | -Sabuk | -Thắt lưng | -เข็มขัด | -حزام
client.AfterBoots | -Сапоги | -जूते | -Sepatu Bot | -Giày | -รองเท้าบูต | -أحذية
client.AfterHelmet | -Шлем | -शिरस्त्राण | -Helm | -Mũ giáp | -หมวกเกราะ | -خوذة
client.MaxAcPlusPercent | Макс. AC + {0}% | अधिकतम AC + {0}% | AC Maksimum + {0}% | AC tối đa + {0}% | AC สูงสุด + {0}% | الحد الأقصى لـ AC + {0}%
client.MaxDcPlusPercent | Макс. DC + {0}% | अधिकतम DC + {0}% | DC Maksimum + {0}% | DC tối đa + {0}% | DC สูงสุด + {0}% | الحد الأقصى لـ DC + {0}%
client.MaxHpPlus | Макс. HP + {0} | अधिकतम HP + {0} | HP Maksimum + {0} | HP tối đa + {0} | HP สูงสุด + {0} | الحد الأقصى لـ HP + {0}
client.MaxHpPlusPercent | Макс. HP + {0}% | अधिकतम HP + {0}% | HP Maksimum + {0}% | HP tối đa + {0}% | HP สูงสุด + {0}% | الحد الأقصى لـ HP + {0}%
client.MaxMacPlusPercent | Макс. MAC + {0}% | अधिकतम MAC + {0}% | MAC Maksimum + {0}% | MAC tối đa + {0}% | MAC สูงสุด + {0}% | الحد الأقصى لـ MAC + {0}%
client.MaxMcPlusPercent | Макс. MC + {0}% | अधिकतम MC + {0}% | MC Maksimum + {0}% | MC tối đa + {0}% | MC สูงสุด + {0}% | الحد الأقصى لـ MC + {0}%
client.MaxMpPlus | Макс. MP + {0} | अधिकतम MP + {0} | MP Maksimum + {0} | MP tối đa + {0} | MP สูงสุด + {0} | الحد الأقصى لـ MP + {0}
client.MaxMpPlusPercent | Макс. MP + {0}% | अधिकतम MP + {0}% | MP Maksimum + {0}% | MP tối đa + {0}% | MP สูงสุด + {0}% | الحد الأقصى لـ MP + {0}%
client.MaxScPlusPercent | Макс. SC + {0}% | अधिकतम SC + {0}% | SC Maksimum + {0}% | SC tối đa + {0}% | SC สูงสุด + {0}% | الحد الأقصى لـ SC + {0}%
client.Top20Warriors | Лучшие 20 воинов | शीर्ष 20 योद्धा | 20 Prajurit Terbaik | 20 chiến binh hàng đầu | นักรบ 20 อันดับแรก | أفضل 20 محاربًا
client.Top20Wizards | Лучшие 20 магов | शीर्ष 20 जादूगर | 20 Penyihir Terbaik | 20 pháp sư hàng đầu | นักเวท 20 อันดับแรก | أفضل 20 ساحرًا
client.Top20Archers | Лучшие 20 лучников | शीर्ष 20 धनुर्धर | 20 Pemanah Terbaik | 20 cung thủ hàng đầu | นักธนู 20 อันดับแรก | أفضل 20 راميًا
client.Top20Assasins | Лучшие 20 ассасинов | शीर्ष 20 हत्यारे | 20 Pembunuh Terbaik | 20 sát thủ hàng đầu | มือสังหาร 20 อันดับแรก | أفضل 20 قاتلًا
client.Top20Taoists | Лучшие 20 даосов | शीर्ष 20 ताओवादी | 20 Taois Terbaik | 20 đạo sĩ hàng đầu | นักพรต 20 อันดับแรก | أفضل 20 طاويًا
`);

// These 296 keys are the common-catalog portion of the v2 identical-text
// review, excluding the seven keys assigned to the independent errors review.
// Pin their actual English inputs, not a workstation-specific report path.
const reviewedCommonSourceDigest = '87f003c079e77bbe3a12fff940c68476f11b0b6cba84003251a1964cdf812927';
const commonKeysOwnedByErrorsReview = [
  'log.realmInfo', 'client.AwakeningWithValue', 'client.CanPickupItems',
  'client.HPDrainRate', 'server.SkillRemovedByGM',
  'server.SpellChangedByGM', 'ui.targetDistance'
];
const reviewedCommonIdentical = [
  { key: 'client.AttackMode_Guild', locale: 'id', value: '[Mode: Guild]', reason: 'Mode and Guild are the reviewed Indonesian game labels for attack mode and player guild; no untranslated sentence.' },
  { key: 'client.GuildKey', locale: 'id', value: 'Guild ({0})', reason: 'Guild is the consistent Indonesian game-community label; the shortcut argument is opaque.' },
  { key: 'client.Menu', locale: 'id', value: 'Menu', reason: 'The ordinary Indonesian word for a menu is Menu; this is a reviewed loanword, not a proper-name exemption.' },
  { key: 'ui.menu', locale: 'id', value: 'Menu', reason: 'The ordinary Indonesian word for a menu is Menu; this is a reviewed loanword, not a proper-name exemption.' },
  { key: 'ui.monster', locale: 'id', value: 'Monster', reason: 'Monster is an established Indonesian noun for a monster, not unreviewed English prose.' },
  ...locales.map((locale) => ({ key: 'server.NpcInfo2', locale, value: 'X : {0}, Y : {1}', reason: 'Only coordinate-axis notation and opaque numeric slots; no natural-language prose.' }))
];

export function reviewedIdenticalText() { return reviewedCommonIdentical.map((entry) => ({ ...entry })); }

function translatedReviewedCommon(entry, catalog) {
  if (catalog !== 'common' || !reviewedCommon.has(entry.key)) return null;
  const leading = entry.en.match(/^\s*/)[0];
  const trailing = entry.en.match(/\s*$/)[0];
  const values = Object.fromEntries(locales.map((locale) => {
    // Several labels are concatenated with opaque values, or begin/end a
    // tooltip line. Keep the original boundary whitespace byte-identical.
    const value = leading + reviewedCommon.get(entry.key)[locale].trim() + trailing;
    assert.deepEqual(numericLiterals(value), numericLiterals(entry.en), `Changed common numerals: ${entry.key}/${locale}`);
    return [locale, value];
  }));
  return { values, rule: 'reviewed-common-identical' };
}

const tooltipBodies = new Map();
function reviewedTooltipBodies(table) {
  for (const line of table.trim().split('\n')) {
    const [skill, ...values] = line.split('|').map((part) => part.trim());
    if (values.length !== locales.length || values.some((value) => !value)) throw new Error(`Invalid skill tooltip: ${skill}`);
    if (tooltipBodies.has(skill)) throw new Error(`Duplicate reviewed skill tooltip: ${skill}`);
    tooltipBodies.set(skill, values);
  }
}
reviewedTooltipBodies(`
Fencing | Каждый уровень навыка повышает точность попадания. | कौशल के प्रत्येक स्तर के साथ प्रहार की सटीकता बढ़ती है। | Setiap tingkat keterampilan meningkatkan akurasi serangan. | Mỗi cấp kỹ năng tăng độ chính xác khi đánh. | ความแม่นยำในการโจมตีเพิ่มขึ้นตามระดับวิชา | تزداد دقة الإصابة مع كل مستوى للمهارة.
Slaying | Точность попадания и сила удара растут с уровнем навыка. | कौशल स्तर बढ़ने पर प्रहार की सटीकता और शक्ति बढ़ती हैं। | Akurasi dan kekuatan serangan meningkat seiring tingkat keterampilan. | Độ chính xác và sức mạnh đòn đánh tăng theo cấp kỹ năng. | ความแม่นยำและพลังโจมตีเพิ่มขึ้นตามระดับวิชา | تزداد دقة الإصابة وقوة الضربة مع مستوى المهارة.
Thrusting | Увеличивает дальность удара. Сила удара растёт с уровнем навыка. | प्रहार की पहुँच बढ़ाता है। कौशल स्तर के साथ प्रहार की शक्ति बढ़ती है। | Meningkatkan jangkauan pukulan. Kekuatannya meningkat seiring tingkat keterampilan. | Tăng tầm đánh. Sức mạnh đòn đánh tăng theo cấp kỹ năng. | เพิ่มระยะโจมตี และพลังโจมตีเพิ่มขึ้นตามระดับวิชา | يزيد مدى الضربة، وتزداد قوتها مع مستوى المهارة.
HalfMoon | Быстрый взмах оружия создаёт ударную волну, поражающую врагов полукругом вокруг вас. | हथियार के तेज़ घुमाव से बनी आघात तरंगें आपके चारों ओर अर्धवृत्त में शत्रुओं को क्षति पहुँचाती हैं। | Ayunan senjata cepat menciptakan gelombang kejut yang melukai musuh dalam setengah lingkaran di sekitar Anda. | Vung vũ khí nhanh tạo sóng xung kích, gây sát thương lên kẻ địch trong nửa vòng tròn quanh bạn. | เหวี่ยงอาวุธอย่างรวดเร็วสร้างคลื่นกระแทก โจมตีศัตรูในครึ่งวงกลมรอบตัวคุณ | تولد أرجحة السلاح السريعة موجات صدمة تضر بالأعداء في نصف دائرة حولك.
FireBall | Собирает силу огня в огненный шар и бросает его в монстра, нанося урон. | अग्नि तत्त्वों से अग्निगोला बनाकर राक्षस पर फेंकता है और क्षति पहुँचाता है। | Mengumpulkan unsur api menjadi bola api lalu melemparkannya ke monster untuk menimbulkan kerusakan. | Tụ nguyên tố lửa thành hỏa cầu rồi ném vào quái vật để gây sát thương. | รวมธาตุไฟเป็นลูกไฟแล้วขว้างใส่มอนสเตอร์เพื่อสร้างความเสียหาย | يجمع عناصر النار في كرة نارية ويلقيها على الوحش لإحداث الضرر.
GreatFireBall | Усиленная версия навыка «Огненный шар», наносящая больше урона цели. | अग्निगोला का अधिक शक्तिशाली रूप, जो लक्ष्य को अधिक क्षति पहुँचाता है। | Versi lebih kuat dari Bola Api yang menimbulkan lebih banyak kerusakan pada sasaran. | Phiên bản nâng cấp của Hỏa cầu, gây thêm sát thương lên mục tiêu. | วิชาลูกไฟขั้นสูงที่สร้างความเสียหายแก่เป้าหมายมากขึ้น | نسخة مطورة من الكرة النارية تسبب ضررًا أكبر للهدف.
SpiritSword | Повышает вероятность попадания по цели в ближнем бою. | निकट युद्ध में लक्ष्य पर प्रहार लगने की संभावना बढ़ाता है। | Meningkatkan peluang mengenai sasaran dalam pertarungan jarak dekat. | Tăng khả năng đánh trúng mục tiêu khi cận chiến. | เพิ่มโอกาสโจมตีโดนเป้าหมายในการต่อสู้ระยะประชิด | يزيد فرصة إصابة الهدف في القتال القريب.
SoulFireBall | Наполняет талисман силой и бросает его в монстра. Талисман вспыхивает огнём. | ताबीज़ में शक्ति भरकर राक्षस पर फेंकता है। ताबीज़ आग की लपटों में फूटता है। | Mengisi jimat dengan kekuatan lalu melemparkannya ke monster. Jimat itu menyala menjadi api. | Truyền sức mạnh vào bùa rồi ném vào quái vật. Lá bùa bùng cháy. | ใส่พลังลงในเครื่องรางแล้วขว้างใส่มอนสเตอร์ เครื่องรางจะลุกเป็นไฟ | يشحن التعويذة بالقوة ويلقيها على الوحش، فتشتعل بالنار.
SummonSkeleton | Призывает сильного скелета с атаками по области, который сражается рядом с вами. | क्षेत्रीय प्रहार करने वाला शक्तिशाली कंकाल बुलाता है, जो आपके साथ लड़ता है। | Memanggil kerangka kuat dengan serangan area yang bertarung di sisi Anda. | Triệu hồi khô lâu mạnh có đòn đánh diện rộng để chiến đấu bên bạn. | อัญเชิญโครงกระดูกทรงพลังที่โจมตีเป็นพื้นที่และต่อสู้เคียงข้างคุณ | يستدعي هيكلًا عظميًا قويًا بهجمات نطاقية ليقاتل إلى جانبك.
Healing | Постепенно восстанавливает HP одной цели. | एक लक्ष्य का HP समय के साथ बहाल करता है। | Memulihkan HP satu sasaran secara bertahap. | Hồi HP theo thời gian cho một mục tiêu. | ค่อย ๆ ฟื้นฟู HP ให้เป้าหมายหนึ่งราย | يستعيد HP لهدف واحد تدريجيًا مع الوقت.
Poisoning | Ослабляет монстров ядом. Зелёный яд снижает HP, красный яд снижает защиту. | विष से राक्षसों को कमज़ोर करता है। हरा विष HP घटाता है; लाल विष रक्षा घटाता है। | Melemahkan monster dengan racun. Racun hijau mengurangi HP; racun merah mengurangi pertahanan. | Dùng độc làm suy yếu quái vật. Độc xanh làm giảm HP; độc đỏ làm giảm phòng thủ. | ใช้พิษทำให้มอนสเตอร์อ่อนแอ พิษเขียวลด HP ส่วนพิษแดงลดพลังป้องกัน | يضعف الوحوش بالسم. السم الأخضر يقلل HP، والسم الأحمر يقلل الدفاعات.
FireWall | Создаёт стену огня в выбранном месте, наносящую урон проходящим через неё монстрам. | चुने हुए स्थान पर अग्नि दीवार बनाता है, जो वहाँ से गुज़रने वाले राक्षसों को क्षति पहुँचाती है। | Membuat dinding api di lokasi pilihan yang melukai monster saat melewati area itu. | Tạo tường lửa tại vị trí chỉ định, gây sát thương cho quái vật đi qua. | สร้างกำแพงไฟในจุดที่เลือกเพื่อโจมตีมอนสเตอร์ที่เดินผ่าน | ينشئ جدارًا ناريًا في الموضع المحدد يضر بالوحوش التي تعبره.
Lightning | Выпускает разряд молнии по прямой, атакуя монстра перед вами. | सामने के राक्षस पर आक्रमण करने के लिए सीधी विद्युत किरण छोड़ता है। | Menembakkan sinar petir lurus untuk menyerang monster di depan Anda. | Phóng tia sét theo đường thẳng để tấn công quái vật phía trước. | ยิงลำแสงสายฟ้าเป็นเส้นตรงเพื่อโจมตีมอนสเตอร์ด้านหน้า | يطلق شعاعًا مستقيمًا من البرق لمهاجمة الوحش أمامك.
ThunderBolt | Поражает врага молнией, нанося большой урон. | शत्रु पर बिजली गिराकर भारी क्षति पहुँचाता है। | Menyambar musuh dengan petir dan menimbulkan kerusakan besar. | Giáng sét vào kẻ địch, gây sát thương lớn. | ฟาดสายฟ้าใส่ศัตรูเพื่อสร้างความเสียหายสูง | يضرب العدو بصاعقة مسببًا ضررًا كبيرًا.
MagicShield | Создаёт вокруг заклинателя защитное поле, поглощающее урон. | मंत्रकर्ता के चारों ओर क्षति सोखने वाला सुरक्षा क्षेत्र बनाता है। | Membuat medan pelindung di sekitar perapal yang menyerap kerusakan. | Tạo trường bảo hộ quanh người thi triển để hấp thụ sát thương. | สร้างเขตป้องกันรอบผู้ร่ายเพื่อดูดซับความเสียหาย | ينشئ مجال حماية حول ملقي التعويذة يمتص الضرر.
TurnUndead | Есть шанс одним применением уничтожить нежить, соответствующую ограничению по уровню. | स्तर की शर्त पूरी करने वाले मृतजीव को एक ही प्रयोग में मारने की संभावना है। | Berpeluang membunuh mayat hidup yang memenuhi syarat tingkat dengan satu penggunaan. | Có cơ hội tiêu diệt xác sống đáp ứng yêu cầu cấp độ chỉ bằng một lần thi triển. | มีโอกาสกำจัดอันเดดที่ผ่านเงื่อนไขระดับได้ในการร่ายครั้งเดียว | توجد فرصة لقتل مخلوق من الموتى الأحياء يستوفي شرط المستوى بإلقاء واحد.
Vampirism | Расходует MP, отнимая HP у монстра и восстанавливая ваши HP. | MP खर्च करके राक्षस का HP चूसता है और आपका HP बहाल करता है। | Menghabiskan MP untuk menyerap HP monster dan memulihkan HP Anda. | Tiêu hao MP để hút HP của quái vật và hồi HP cho bạn. | ใช้ MP เพื่อดูด HP จากมอนสเตอร์มาฟื้นฟู HP ของคุณ | يستهلك MP لامتصاص HP من الوحش واستعادة HP الخاص بك.
`);

// Each body below is authored against the complete English source, with the
// existing Traditional description as context rather than unquestioned truth.
reviewedTooltipBodies(`
BackStep | Быстро отпрыгивает назад, подальше от опасности. | ख़तरे से बचने के लिए तेज़ी से पीछे छलाँग लगाता है। | Melompat mundur dengan cepat untuk menjauh dari bahaya. | Nhanh chóng nhảy lùi để tránh nguy hiểm. | กระโดดถอยหลังอย่างรวดเร็วเพื่อหนีจากอันตราย | يقفز سريعًا إلى الخلف للابتعاد عن الخطر.
BladeAvalanche | Метает клинки в трёх направлениях перед заклинателем, создавая смертоносную бурю металла. | मंत्रकर्ता के सामने तीन दिशाओं में धारदार हथियार फेंकता है, जिससे धातु का घातक तूफ़ान बनता है। | Melempar bilah ke tiga arah di depan perapal, menciptakan badai logam yang mematikan. | Phóng các lưỡi kiếm theo ba hướng phía trước người thi triển, tạo thành cơn bão kim loại chết chóc. | ขว้างคมดาบออกไปสามทิศทางด้านหน้าผู้ร่าย ก่อให้เกิดพายุโลหะอันร้ายแรง | يقذف نصالًا في ثلاثة اتجاهات أمام ملقي المهارة، مشكلًا عاصفة معدنية قاتلة.
BlessedArmour | Благословляет заклинателя и членов группы, повышая их защиту. | मंत्रकर्ता और दल के सदस्यों को आशीर्वाद देकर उनकी रक्षा बढ़ाता है। | Memberkati perapal dan anggota kelompok untuk meningkatkan pertahanan mereka. | Ban phúc cho người thi triển và các thành viên tổ đội, tăng phòng thủ của họ. | อวยพรผู้ร่ายและสมาชิกปาร์ตี้เพื่อเพิ่มพลังป้องกัน | يبارك ملقي التعويذة وأعضاء الفريق لتعزيز دفاعهم.
Blink | Перемещает в случайное место поблизости. | आपको पास के किसी यादृच्छिक स्थान पर पहुँचाता है। | Berpindah ke lokasi acak di dekat Anda. | Dịch chuyển đến một vị trí ngẫu nhiên ở gần bạn. | วาร์ปไปยังตำแหน่งสุ่มใกล้ตัว | ينقلك إلى موضع عشوائي قريب منك.
Blizzard | Сосредотачивает внутреннюю силу и распределяет её по всему телу, усиливая защиту от врагов. Сила защиты и длительность зависят от уровня навыка. После применения нужно подождать, прежде чем использовать навык снова. | आंतरिक शक्ति को एकत्र करके पूरे शरीर में फैलाता है, जिससे शत्रुओं से रक्षा बढ़ती है। रक्षा की शक्ति और अवधि कौशल स्तर पर निर्भर हैं। दोबारा प्रयोग करने से पहले प्रतीक्षा करनी होगी। | Memusatkan tenaga dalam dan menyebarkannya ke seluruh tubuh untuk memperkuat perlindungan dari musuh. Kekuatan pertahanan dan durasi bergantung pada tingkat keterampilan. Setelah digunakan, harus menunggu sebelum dapat menggunakannya lagi. | Tập trung nội lực rồi lan tỏa khắp cơ thể để tăng khả năng phòng vệ trước kẻ địch. Sức phòng thủ và thời gian duy trì phụ thuộc vào cấp kỹ năng. Sau khi dùng phải chờ một thời gian mới có thể dùng lại. | รวมพลังภายในแล้วกระจายทั่วร่างกายเพื่อเพิ่มการป้องกันจากศัตรู พลังป้องกันและระยะเวลาขึ้นอยู่กับระดับวิชา หลังใช้ต้องรอก่อนจึงจะใช้ได้อีกครั้ง | يركز القوة الداخلية وينشرها في أنحاء الجسم لتعزيز الحماية من الأعداء. تعتمد قوة الدفاع ومدته على مستوى المهارة. بعد استخدامها يجب الانتظار قبل استعمالها مجددًا.
Concentration | Пока навык активен, повышает вероятность сбора элементов. | सक्रिय रहने पर तत्त्व इकट्ठा होने की संभावना बढ़ाता है। | Meningkatkan peluang mengumpulkan elemen selama aktif. | Tăng cơ hội thu thập nguyên tố trong khi kỹ năng có hiệu lực. | เพิ่มโอกาสรวบรวมธาตุขณะวิชามีผล | يزيد فرصة جمع العناصر أثناء تفعيل المهارة.
CounterAttack | На короткое время повышает AC и AMC. Даёт шанс отразить атаку и контратаковать. | थोड़े समय के लिए AC और AMC बढ़ाता है। प्रहार रोककर प्रत्याक्रमण करने की संभावना देता है। | Meningkatkan AC dan AMC untuk waktu singkat. Memberi peluang menahan serangan dan melakukan serangan balik. | Tăng AC và AMC trong thời gian ngắn. Có cơ hội đỡ đòn và phản công. | เพิ่ม AC และ AMC ชั่วครู่ มีโอกาสป้องกันการโจมตีและสวนกลับ | يزيد AC وAMC لفترة قصيرة، ويمنح فرصة لصد هجوم والرد بهجوم مضاد.
CrescentSlash | Высвобождает силу меча и атакует всех монстров вокруг вас. | तलवार की शक्ति मुक्त करके आपके चारों ओर के सभी राक्षसों पर प्रहार करता है। | Melepaskan kekuatan pedang untuk menyerang semua monster di sekitar Anda. | Bộc phát sức mạnh của kiếm để tấn công tất cả quái vật xung quanh bạn. | ปลดปล่อยพลังดาบเพื่อโจมตีมอนสเตอร์ทั้งหมดรอบตัวคุณ | يطلق قوة سيفك ليهاجم جميع الوحوش من حولك.
CrippleShot | Выпускает стрелу, замедляющую врага. Усиление «Ядовитый выстрел» превращает этот выстрел в ядовитую атаку по области 3×3. Усиление «Вампирский выстрел» позволяет ударить дважды и похитить HP. | ऐसा बाण छोड़ता है जो शत्रु को धीमा करता है। विष बाण का प्रभाव इसे 3×3 क्षेत्र का विषाक्त प्रहार बनाता है। रक्तचूषक बाण का प्रभाव इसे दो बार प्रहार करने और HP चुराने देता है। | Menembakkan panah yang memperlambat musuh. Penguatan Tembakan Racun membuat serangan ini menjadi serangan racun area 3×3. Penguatan Tembakan Vampir membuatnya mengenai dua kali dan mencuri HP. | Bắn mũi tên làm chậm kẻ địch. Hiệu ứng cường hóa Độc tiễn biến đòn này thành đòn độc diện rộng 3×3. Hiệu ứng cường hóa Tiễn hút máu khiến đòn này đánh hai lần và hút HP. | ยิงศรที่ทำให้ศัตรูช้าลง บัฟศรพิษทำให้การโจมตีนี้สร้างพิษในพื้นที่ 3×3 ส่วนบัฟศรดูดเลือดทำให้โจมตีสองครั้งและดูด HP | يطلق سهمًا يبطئ العدو. يجعل تعزيز الطلقة السامة هذه الضربة هجومًا سامًا بمساحة 3×3. ويجعل تعزيز طلقة مصاص الدماء الضربة تصيب مرتين وتسرق HP.
CrossHalfMoon | Воин выпускает две мощные волны навыка «Полумесяц», нанося урон всем монстрам, стоящим рядом с ним. | योद्धा अर्धचंद्र की दो शक्तिशाली तरंगों से अपने ठीक पास खड़े सभी राक्षसों को क्षति पहुँचाता है। | Prajurit menggunakan dua gelombang Bulan Sabit yang kuat untuk melukai semua monster yang berdiri tepat di dekatnya. | Chiến binh dùng hai làn sóng Bán nguyệt mạnh mẽ để gây sát thương lên tất cả quái vật đứng sát bên mình. | นักรบใช้คลื่นจันทร์เสี้ยวอันทรงพลังสองระลอกเพื่อสร้างความเสียหายแก่มอนสเตอร์ทั้งหมดที่อยู่ติดตัว | يطلق المحارب موجتين قويتين من الهلال لإلحاق الضرر بكل الوحوش الواقفة بجواره مباشرة.
Curse | Снижает скорость атаки, DC, MC и SC цели. | लक्ष्य की आक्रमण गति, DC, MC और SC घटाता है। | Mengurangi kecepatan serang, DC, MC, dan SC sasaran. | Giảm tốc độ đánh, DC, MC và SC của mục tiêu. | ลดความเร็วโจมตี DC, MC และ SC ของเป้าหมาย | يقلل سرعة هجوم الهدف وقيم DC وMC وSC لديه.
DarkBody | Создаёт вашу иллюзию, которая атакует монстра, пока вы становитесь невидимым. | अपनी एक माया-प्रतिमा बनाता है जो राक्षस पर आक्रमण करती है, जबकि आप अदृश्य हो जाते हैं। | Menciptakan ilusi diri Anda yang menyerang monster sementara Anda menjadi tidak terlihat. | Tạo ảo ảnh của bạn để tấn công quái vật trong khi bạn trở nên vô hình. | สร้างภาพลวงตาของคุณให้โจมตีมอนสเตอร์ ขณะที่คุณล่องหน | ينشئ وهمًا على هيئتك يهاجم الوحش بينما تصبح أنت غير مرئي.
DelayedExplosion | Выпускает стрелу, взрывающуюся после небольшой задержки. Использует элементы для дополнительного урона. | ऐसा बाण छोड़ता है जो थोड़ी देर बाद विस्फोट करता है। अतिरिक्त क्षति के लिए तत्त्वों का उपयोग करता है। | Menembakkan panah yang meledak setelah jeda singkat. Menggunakan elemen untuk menimbulkan kerusakan tambahan. | Bắn mũi tên phát nổ sau một khoảng trễ ngắn. Dùng nguyên tố để gây thêm sát thương. | ยิงศรที่ระเบิดหลังหน่วงเวลาสั้น ๆ ใช้ธาตุเพื่อสร้างความเสียหายเพิ่มเติม | يطلق سهمًا ينفجر بعد تأخير قصير. يستخدم العناصر لإحداث ضرر إضافي.
DoubleShot | Быстро выпускает две стрелы подряд. | तेज़ी से एक के बाद एक दो बाण छोड़ता है। | Menembakkan dua panah secara beruntun dengan cepat. | Bắn nhanh hai mũi tên liên tiếp. | ยิงศรสองดอกต่อเนื่องอย่างรวดเร็ว | يطلق سهمين متتاليين بسرعة.
DoubleSlash | Быстро наносит монстру два рубящих удара. | तेज़ गति से राक्षस पर दो बार तलवार का प्रहार करता है। | Menebas monster dua kali dengan gerakan cepat. | Chém quái vật hai lần liên tiếp bằng động tác nhanh. | ฟันมอนสเตอร์สองครั้งอย่างรวดเร็ว | يشق الوحش بضربتين سريعتين.
ElectricShock | Мощная ударная волна поражает монстра: он либо теряет способность двигаться, либо приходит в замешательство и сражается на вашей стороне. | शक्तिशाली आघात तरंग राक्षस को लगती है। वह या तो हिल नहीं पाता, या भ्रमित होकर आपकी ओर से लड़ता है। | Gelombang kejut kuat mengenai monster sehingga tidak dapat bergerak, atau membuatnya bingung dan bertarung untuk Anda. | Sóng xung kích mạnh đánh trúng quái vật, khiến nó không thể di chuyển hoặc bị mê hoặc và chiến đấu cho bạn. | คลื่นกระแทกรุนแรงโจมตีมอนสเตอร์ ทำให้มันเคลื่อนไหวไม่ได้ หรือสับสนแล้วต่อสู้ให้คุณ | تصيب موجة صدم قوية الوحش، فإما تمنعه من الحركة أو تربكه ليقاتل إلى جانبك.
ElementalBarrier | Защищает заклинателя стихийным барьером. Чем больше элементов при применении, тем сильнее снижение урона. | मंत्रकर्ता को तत्त्व अवरोध से बचाता है। प्रयोग के समय जितने अधिक तत्त्व हों, प्राप्त क्षति उतनी अधिक घटती है। | Melindungi perapal dengan penghalang elemen. Semakin banyak elemen saat digunakan, semakin besar pengurangan kerusakan. | Bảo vệ người thi triển bằng kết giới nguyên tố. Càng có nhiều nguyên tố khi thi triển, mức giảm sát thương càng cao. | ปกป้องผู้ร่ายด้วยม่านพลังธาตุ ยิ่งมีธาตุมากตอนร่าย ยิ่งลดความเสียหายได้มาก | يحمي ملقي المهارة بحاجز من العناصر. كلما زادت العناصر وقت الإلقاء زاد تقليل الضرر.
ElementalShot | Магическая атака с высоким уроном. Каждый элемент увеличивает урон. Если элементов нет, создаёт 2 элемента. Отталкивает цель, если уровень лучника выше уровня цели. | भारी क्षति वाला जादुई प्रहार। प्रत्येक तत्त्व क्षति बढ़ाता है। कोई तत्त्व न होने पर 2 तत्त्व बनते हैं। धनुर्धर का स्तर लक्ष्य से ऊँचा हो तो लक्ष्य को पीछे धकेलता है। | Serangan sihir dengan kerusakan tinggi. Setiap elemen menambah kerusakan. Jika tidak ada elemen, menghasilkan 2 elemen. Mendorong sasaran jika level pemanah lebih tinggi daripada sasaran. | Đòn tấn công phép gây sát thương cao. Mỗi nguyên tố tăng thêm sát thương. Nếu không có nguyên tố thì tạo ra 2 nguyên tố. Đẩy lùi mục tiêu nếu cấp cung thủ cao hơn cấp mục tiêu. | โจมตีเวทมนตร์ความเสียหายสูง แต่ละธาตุเพิ่มความเสียหาย หากไม่มีธาตุจะสร้าง 2 ธาตุ ผลักเป้าหมายถอยหลังเมื่อนักธนูมีระดับสูงกว่าเป้าหมาย | هجوم سحري عالي الضرر. يزيد كل عنصر الضرر. إذا لم توجد عناصر، يولد 2 من العناصر. يدفع الهدف إلى الخلف إذا كان مستوى الرامي أعلى من مستوى الهدف.
EnergyRepulsor | Сосредотачивает энергию в одном мощном взрыве, отталкивающем монстров вокруг вас. | ऊर्जा एकत्र करके एक बड़ा विस्फोट करता है, जो आसपास के राक्षसों को दूर धकेलता है। | Memusatkan energi menjadi satu ledakan besar untuk mendorong monster di sekitar Anda menjauh. | Tập trung năng lượng thành một vụ bộc phát mạnh để đẩy lùi quái vật xung quanh bạn. | รวมพลังเป็นแรงระเบิดครั้งใหญ่เพื่อผลักมอนสเตอร์รอบตัวคุณออกไป | يركز طاقتك في انفجار كبير واحد يدفع الوحوش المحيطة بك بعيدًا.
EnergyShield | Можно применять на себя и дружественные цели. Отражает часть полученного урона обратно в атакующего. | स्वयं और मित्र लक्ष्यों पर प्रयोग किया जा सकता है। प्राप्त क्षति का एक भाग हमलावर को वापस लौटाता है। | Dapat digunakan pada diri sendiri dan sasaran sekutu. Memantulkan sebagian kerusakan yang diterima kembali kepada penyerang. | Có thể dùng lên bản thân và mục tiêu đồng minh. Phản lại một phần sát thương nhận vào cho kẻ tấn công. | ใช้กับตนเองและเป้าหมายฝ่ายเดียวกันได้ สะท้อนความเสียหายที่ได้รับส่วนหนึ่งกลับไปยังผู้โจมตี | يمكن إلقاؤه على النفس وعلى الحلفاء. يعكس نسبة من الضرر المستلم إلى المهاجم.
Entrapment | Парализует монстров и притягивает их к заклинателю. | राक्षसों को लकवाग्रस्त करके मंत्रकर्ता की ओर खींचता है। | Melumpuhkan monster dan menariknya ke arah perapal. | Làm tê liệt quái vật rồi kéo chúng về phía người thi triển. | ทำให้มอนสเตอร์เป็นอัมพาตแล้วดึงเข้าหาผู้ร่าย | يشل الوحوش ويسحبها نحو ملقي المهارة.
ExplosiveTrap | Устанавливает ряд ловушек, которые взрываются при соприкосновении с врагом. | एक पंक्ति में जाल बिछाता है, जो शत्रु के संपर्क में आने पर फटते हैं। | Memasang deretan jebakan yang meledak saat bersentuhan dengan musuh. | Đặt một hàng bẫy phát nổ khi kẻ địch chạm vào. | วางกับดักเป็นแถว ซึ่งจะระเบิดเมื่อศัตรูสัมผัส | يضع صفًا من الأفخاخ التي تنفجر عند ملامسة عدو.
FatalSword | Повышает урон атак по монстрам, а также немного увеличивает точность. | राक्षसों पर प्रहार की क्षति बढ़ाता है और सटीकता भी थोड़ी बढ़ाता है। | Meningkatkan kerusakan serangan terhadap monster serta sedikit meningkatkan akurasi. | Tăng sát thương đòn đánh lên quái vật và tăng nhẹ độ chính xác. | เพิ่มความเสียหายจากการโจมตีมอนสเตอร์ และเพิ่มความแม่นยำเล็กน้อย | يزيد ضرر الهجمات على الوحوش، ويرفع الدقة قليلًا أيضًا.
`);

reviewedTooltipBodies(`
FireBang | В выбранном месте вспыхивает огонь, обжигающий всех монстров в этой области. | चुने हुए स्थान पर आग भड़काता है, जो उस क्षेत्र के सभी राक्षसों को जलाती है। | Memunculkan ledakan api di lokasi yang ditentukan untuk membakar semua monster di area tersebut. | Bùng lửa tại vị trí chỉ định, thiêu đốt tất cả quái vật trong khu vực đó. | ระเบิดไฟขึ้นในตำแหน่งที่เลือกเพื่อเผามอนสเตอร์ทั้งหมดในพื้นที่นั้น | يشعل انفجارًا ناريًا في الموضع المحدد ليحرق جميع الوحوش داخل المنطقة.
FireBurst | Отталкивает окружающих вас монстров. | आपको घेरे हुए राक्षसों को दूर धकेलता है। | Mendorong monster yang mengelilingi Anda menjauh. | Đẩy lùi quái vật đang bao quanh bạn. | ผลักมอนสเตอร์ที่ล้อมรอบคุณออกไป | يدفع الوحوش المحيطة بك بعيدًا.
FlameDisruptor | Поднимает пламя из-под земли на поверхность, чтобы атаковать монстров. | राक्षसों पर आक्रमण करने के लिए भूमिगत ज्वालाओं को सतह पर लाता है। | Membawa api dari bawah tanah ke permukaan untuk menyerang monster. | Đưa lửa từ dưới lòng đất lên bề mặt để tấn công quái vật. | ดึงเปลวไฟจากใต้ดินขึ้นสู่พื้นผิวเพื่อโจมตีมอนสเตอร์ | يرفع اللهب من باطن الأرض إلى سطحها لمهاجمة الوحوش.
FlameField | Мощное огненное заклинание наносит урон окружающим врагам. | शक्तिशाली अग्नि मंत्र आसपास के शत्रुओं को क्षति पहुँचाता है। | Menggunakan mantra api yang kuat untuk melukai musuh di sekitar. | Dùng phép lửa mạnh để gây sát thương lên kẻ địch xung quanh. | ใช้เวทไฟอันทรงพลังสร้างความเสียหายแก่ศัตรูรอบตัว | يستخدم تعويذة نارية قوية لإلحاق الضرر بالأعداء المحيطين.
FlamingSword | Призывает дух огня в вашу следующую атаку, нанося цели сокрушительный удар. | अगली चोट में अग्नि की आत्मा बुलाकर लक्ष्य पर विनाशकारी प्रहार करता है। | Memanggil roh api ke dalam serangan Anda berikutnya untuk memberikan pukulan dahsyat pada sasaran. | Triệu gọi tinh linh lửa vào đòn đánh tiếp theo của bạn, giáng một đòn tàn phá lên mục tiêu. | เรียกวิญญาณไฟเข้าสู่การโจมตีครั้งถัดไปของคุณ เพื่อโจมตีเป้าหมายอย่างรุนแรง | يستدعي روح النار في هجومك التالي ليوجه ضربة مدمرة إلى الهدف.
FlashDash | Быстро рубит монстра и парализует его. | तेज़ तलवार प्रहार से राक्षस पर आक्रमण करके उसे लकवाग्रस्त करता है। | Menyerang monster dengan tebasan cepat dan melumpuhkannya. | Chém nhanh vào quái vật và làm nó tê liệt. | ฟันโจมตีมอนสเตอร์อย่างรวดเร็วและทำให้เป็นอัมพาต | يهاجم الوحش بشق سريع ويصيبه بالشلل.
Focus | Повышает шанс попадания физическими атаками. | शारीरिक प्रहारों के लक्ष्य पर लगने की संभावना बढ़ाता है। | Meningkatkan peluang serangan fisik mengenai sasaran. | Tăng khả năng đánh trúng bằng đòn tấn công vật lý. | เพิ่มโอกาสโจมตีโดนด้วยการโจมตีกายภาพ | يزيد فرصة إصابة الهدف بالهجمات الجسدية.
FrostCrunch | Замораживает элементы воздуха вокруг монстра, замедляя его. | राक्षस के चारों ओर हवा के तत्त्व जमाकर उसे धीमा करता है। | Membekukan unsur udara di sekitar monster untuk memperlambatnya. | Đóng băng các nguyên tố trong không khí quanh quái vật để làm nó chậm lại. | แช่แข็งธาตุในอากาศรอบมอนสเตอร์เพื่อทำให้มันช้าลง | يجمد العناصر في الهواء المحيط بالوحش لإبطائه.
Fury | Повышает точность воина на определённое время. | निर्धारित समय के लिए योद्धा की सटीकता बढ़ाता है। | Meningkatkan akurasi prajurit selama jangka waktu tertentu. | Tăng độ chính xác của chiến binh trong một khoảng thời gian nhất định. | เพิ่มความแม่นยำของนักรบเป็นระยะเวลาหนึ่ง | يزيد دقة المحارب لفترة محددة.
Hallucination | Погружает монстра в галлюцинации, заставляя его атаковать любого, кто встретится на пути. | राक्षस को मतिभ्रम में डालता है, जिससे वह रास्ते में मिलने वाले किसी भी व्यक्ति पर आक्रमण करता है। | Membuat monster melihat halusinasi dan menyerang siapa pun yang ditemuinya. | Khiến quái vật chìm trong ảo giác và tấn công bất kỳ ai gặp trên đường. | ทำให้มอนสเตอร์เห็นแต่ภาพหลอนและโจมตีทุกคนที่พบระหว่างทาง | يجعل الوحش يرى هلوسات ويهاجم أي شخص يصادفه في طريقه.
Haste | Повышает скорость атаки. | आक्रमण की गति बढ़ाता है। | Meningkatkan kecepatan serang. | Tăng tốc độ đánh. | เพิ่มความเร็วโจมตี | يزيد سرعة الهجوم.
HealingCircle | Исцеляет дружественные цели в области и наносит врагам магический урон. | क्षेत्र में मित्र लक्ष्यों का उपचार करता है और शत्रुओं को जादुई क्षति पहुँचाता है। | Menyembuhkan sasaran sekutu di area dan menimbulkan kerusakan sihir pada musuh. | Hồi máu cho mục tiêu đồng minh trong khu vực và gây sát thương phép lên kẻ địch. | รักษาเป้าหมายฝ่ายเดียวกันในพื้นที่ และสร้างความเสียหายเวทมนตร์แก่ศัตรู | يشفي الحلفاء داخل المنطقة ويلحق ضررًا سحريًا بالأعداء.
HeavenlySword | Атакует монстров в радиусе 2 шагов. | 2 कदम के दायरे में मौजूद राक्षसों पर आक्रमण करता है। | Menyerang monster dalam radius 2 langkah. | Tấn công quái vật trong bán kính 2 bước. | โจมตีมอนสเตอร์ในรัศมี 2 ก้าว | يهاجم الوحوش ضمن نصف قطر قدره 2 من الخطوات.
HellFire | Выпускает полосу пламени, атакуя монстра перед вами. | सामने के राक्षस पर आक्रमण करने के लिए आग की एक लकीर छोड़ता है। | Menembakkan rentetan api untuk menyerang monster di depan. | Phóng một luồng lửa để tấn công quái vật phía trước. | ยิงแนวเปลวไฟเพื่อโจมตีมอนสเตอร์ด้านหน้า | يطلق شريطًا من اللهب لمهاجمة الوحش أمامك.
Hemorrhage | Даёт шанс нанести критический урон и вызвать кровотечение, наносящее урон. | गंभीर प्रहार की क्षति और रक्तस्राव से होने वाली क्षति पहुँचाने की संभावना देता है। | Memberi peluang menimbulkan kerusakan kritis serta kerusakan pendarahan. | Có cơ hội gây sát thương chí mạng và sát thương chảy máu. | มีโอกาสสร้างความเสียหายคริติคอลและความเสียหายจากเลือดไหล | يمنح فرصة لإحداث ضرر حرج وضرر نزيف.
Hiding | На короткое время скрывает вас от монстров. Они заметят вас, если вы начнёте двигаться. | थोड़े समय के लिए आपको राक्षसों की नज़र से छिपाता है। आपके चलना शुरू करते ही वे आपको देख लेंगे। | Menyembunyikan Anda dari monster untuk waktu singkat. Monster akan menyadari keberadaan Anda jika Anda mulai bergerak. | Trong thời gian ngắn, quái vật không thể phát hiện bạn. Chúng sẽ nhận ra bạn nếu bạn bắt đầu di chuyển. | ทำให้มอนสเตอร์มองไม่เห็นคุณชั่วครู่ หากคุณเริ่มเคลื่อนไหว มอนสเตอร์จะสังเกตเห็นคุณ | يخفيك عن الوحوش لفترة قصيرة. ستلاحظك الوحوش إذا بدأت بالحركة.
IceStorm | Создаёт ледяную бурю в выбранной области, атакуя находящихся в ней монстров. | चुने हुए क्षेत्र में हिम तूफ़ान बनाकर उसके भीतर के राक्षसों पर आक्रमण करता है। | Menciptakan badai es di area yang ditentukan untuk menyerang monster di dalamnya. | Tạo bão băng tại khu vực chỉ định để tấn công quái vật bên trong. | สร้างพายุน้ำแข็งในพื้นที่ที่เลือกเพื่อโจมตีมอนสเตอร์ภายใน | ينشئ عاصفة جليدية في منطقة محددة لمهاجمة الوحوش الموجودة داخلها.
IceThrust | Создаёт ледяной столб, чтобы атаковать монстров. | बर्फ़ का स्तंभ बनाकर राक्षसों पर आक्रमण करता है। | Menciptakan pilar es untuk menyerang monster. | Tạo cột băng để tấn công quái vật. | สร้างเสาน้ำแข็งเพื่อโจมตีมอนสเตอร์ | ينشئ عمودًا جليديًا لمهاجمة الوحوش.
ImmortalSkin | Повышает защиту, уменьшая урон от атак. | रक्षा बढ़ाकर प्रहारों से होने वाली क्षति घटाता है। | Meningkatkan pertahanan untuk mengurangi kerusakan serangan. | Tăng phòng thủ để giảm sát thương từ đòn đánh. | เพิ่มพลังป้องกันเพื่อลดความเสียหายจากการโจมตี | يزيد الدفاع لتقليل الضرر الناتج عن الهجمات.
LightBody | Облегчает ваше тело, позволяя двигаться быстрее. | शरीर हल्का करके आपको अधिक तेज़ी से चलने देता है। | Meringankan tubuh agar dapat bergerak lebih cepat. | Làm cơ thể nhẹ hơn để di chuyển nhanh hơn. | ทำให้ร่างกายเบาลงและเคลื่อนที่เร็วขึ้น | يجعل جسدك أخف لتتحرك بسرعة أكبر.
LionRoar | Парализует врагов вокруг заклинателя. Длительность растёт с уровнем навыка. | मंत्रकर्ता के आसपास के शत्रुओं को लकवाग्रस्त करता है। अवधि कौशल स्तर के साथ बढ़ती है। | Melumpuhkan musuh di sekitar perapal. Durasi meningkat seiring tingkat keterampilan. | Làm tê liệt kẻ địch quanh người thi triển. Thời gian hiệu lực tăng theo cấp kỹ năng. | ทำให้ศัตรูรอบผู้ร่ายเป็นอัมพาต ระยะเวลาเพิ่มขึ้นตามระดับวิชา | يشل الأعداء حول ملقي المهارة، وتزداد المدة مع مستوى المهارة.
MagicBooster | Повышает магический урон, но расходует дополнительные MP. | जादुई क्षति बढ़ाता है, लेकिन अतिरिक्त MP खर्च करता है। | Meningkatkan kerusakan sihir, tetapi menghabiskan MP tambahan. | Tăng sát thương phép nhưng tiêu hao thêm MP. | เพิ่มความเสียหายเวทมนตร์ แต่ใช้ MP เพิ่มเติม | يزيد الضرر السحري، لكنه يستهلك MP إضافيًا.
MassHealing | Окружает маной всех раненых игроков в указанной области, исцеляя их. | चुने हुए क्षेत्र के सभी घायल खिलाड़ियों को माना से घेरकर उनका उपचार करता है। | Menyelimuti semua pemain yang terluka di area tertentu dengan mana untuk menyembuhkan mereka. | Bao bọc tất cả người chơi bị thương trong khu vực chỉ định bằng pháp lực để hồi máu cho họ. | ใช้มานาล้อมผู้เล่นที่บาดเจ็บทั้งหมดในพื้นที่ที่กำหนดเพื่อรักษาพวกเขา | يحيط جميع اللاعبين المصابين في المنطقة المحددة بالمانا لشفائهم.
MassHiding | На короткое время скрывает вас и членов вашей группы от монстров. Когда вы начнёте двигаться, монстры заметят вас и вашу группу. | थोड़े समय के लिए आपको और आपके दल को राक्षसों से छिपाता है। आपके चलना शुरू करने पर राक्षस आपको और आपके दल को देख लेंगे। | Menyembunyikan Anda dan anggota kelompok dari monster untuk waktu singkat. Jika Anda mulai bergerak, monster akan menyadari keberadaan Anda dan kelompok Anda. | Trong thời gian ngắn, quái vật không thể phát hiện bạn và các thành viên tổ đội. Khi bạn bắt đầu di chuyển, chúng sẽ nhận ra bạn và tổ đội của bạn. | ทำให้มอนสเตอร์มองไม่เห็นคุณและสมาชิกปาร์ตี้ชั่วครู่ หากคุณเริ่มเคลื่อนไหว มอนสเตอร์จะสังเกตเห็นคุณและปาร์ตี้ | يخفيك ويخفي أعضاء فريقك عن الوحوش لفترة قصيرة. إذا بدأت بالحركة فستلاحظك الوحوش أنت وفريقك.
Meditation | Позволяет собирать элементы при атаках по монстрам. Всего можно накопить до 4 элементов. | राक्षसों पर आक्रमण करते समय तत्त्व इकट्ठा करने देता है। कुल मिलाकर अधिकतम 4 तत्त्व मिल सकते हैं। | Memungkinkan pengumpulan elemen saat menyerang monster. Dapat mengumpulkan hingga 4 elemen secara total. | Cho phép thu thập nguyên tố khi tấn công quái vật. Có thể tích lũy tối đa 4 nguyên tố. | ทำให้รวบรวมธาตุได้เมื่อโจมตีมอนสเตอร์ สะสมได้สูงสุดรวม 4 ธาตุ | يتيح جمع العناصر عند مهاجمة الوحوش، حتى مجموع أقصاه 4 عناصر.
MeteorStrike | Обрушивает с неба сгустки огня, атакуя всех монстров в квадратной области 5x5. | आकाश से आग के गोले गिराकर 5x5 वर्गाकार क्षेत्र के सभी राक्षसों पर आक्रमण करता है। | Menjatuhkan gumpalan api dari langit untuk menyerang semua monster di area persegi 5x5. | Gọi những khối lửa từ trời rơi xuống, tấn công tất cả quái vật trong vùng vuông 5x5. | ให้ก้อนไฟตกจากท้องฟ้าเพื่อโจมตีมอนสเตอร์ทั้งหมดในพื้นที่สี่เหลี่ยม 5x5 | يسقط كتلًا من النار من السماء لمهاجمة جميع الوحوش داخل منطقة مربعة بمقاس 5x5.
Mirroring | Создаёт вашего зеркального двойника, чтобы вместе атаковать монстров. | अपनी प्रतिबिंब-प्रतिमा बनाता है, जो आपके साथ मिलकर राक्षसों पर आक्रमण करती है। | Menciptakan bayangan cermin diri Anda untuk menyerang monster bersama-sama. | Tạo phân thân phản chiếu của bạn để cùng tấn công quái vật. | สร้างร่างสะท้อนของคุณเพื่อร่วมกันโจมตีมอนสเตอร์ | ينشئ صورة مرآة منك لتهاجما الوحوش معًا.
MoonLight | Делает вас невидимым для монстров. Атаки по монстрам с этим навыком наносят больше урона. | अदृश्य बनाकर आपको राक्षसों से छिपाता है। इस कौशल के साथ राक्षसों पर प्रहार करने से अधिक क्षति होती है। | Membuat Anda tidak terlihat oleh monster. Serangan terhadap monster dengan keterampilan ini menimbulkan kerusakan lebih besar. | Khiến bạn vô hình trước quái vật. Tấn công quái vật khi dùng kỹ năng này sẽ gây sát thương cao hơn. | ทำให้คุณล่องหนจากมอนสเตอร์ เมื่อโจมตีมอนสเตอร์ด้วยวิชานี้จะสร้างความเสียหายมากขึ้น | يجعلك غير مرئي للوحوش. تسبب هجماتك على الوحوش باستخدام هذه المهارة ضررًا أكبر.
MoonMist | Скрывает вас от монстров. Ваша первая атака будет сильнее обычной. | आपको राक्षसों की नज़र से छिपाता है। आपका पहला प्रहार सामान्य से अधिक शक्तिशाली होगा। | Menyembunyikan diri Anda dari monster. Serangan pertama Anda akan lebih kuat daripada biasanya. | Che giấu bạn khỏi quái vật. Đòn đánh đầu tiên của bạn sẽ mạnh hơn bình thường. | ซ่อนคุณจากมอนสเตอร์ การโจมตีครั้งแรกของคุณจะแรงกว่าปกติ | يخفيك عن الوحوش، ويجعل هجومك الأول أقوى من المعتاد.
MPEater | Поглощает MP монстров, восстанавливая MP заклинателя. | राक्षसों का MP सोखकर मंत्रकर्ता का MP बहाल करता है। | Menyerap MP monster untuk memulihkan MP perapal. | Hút MP của quái vật để hồi MP cho người thi triển. | ดูด MP ของมอนสเตอร์เพื่อฟื้นฟู MP ให้ผู้ร่าย | يمتص MP الوحوش لاستعادة MP ملقي المهارة.
`);

reviewedTooltipBodies(`
NapalmShot | Выпускает стрелу, взрывающуюся в области 5×5 вокруг цели. | ऐसा बाण छोड़ता है जो लक्ष्य के आसपास 5×5 क्षेत्र में विस्फोट करता है। | Menembakkan panah yang meledak di area 5×5 di sekitar sasaran. | Bắn mũi tên phát nổ trong vùng 5×5 quanh mục tiêu. | ยิงศรที่ระเบิดในพื้นที่ 5×5 รอบเป้าหมาย | يطلق سهمًا ينفجر في منطقة بمقاس 5×5 حول الهدف.
OneWithNature | Призывает вокруг заклинателя стихийное кольцо, наносящее урон всем целям в области 5×5. | मंत्रकर्ता के चारों ओर तत्त्वों का घेरा बुलाता है, जो 5×5 क्षेत्र के सभी लक्ष्यों को क्षति पहुँचाता है। | Memanggil lingkaran elemen di sekitar perapal yang melukai semua sasaran di area 5×5. | Triệu hồi vòng nguyên tố quanh người thi triển, gây sát thương lên tất cả mục tiêu trong vùng 5×5. | เรียกวงแหวนธาตุรอบผู้ร่าย ซึ่งสร้างความเสียหายแก่ทุกเป้าหมายในพื้นที่ 5×5 | يستدعي حلقة من العناصر حول ملقي المهارة، تلحق الضرر بجميع الأهداف داخل منطقة بمقاس 5×5.
PetEnhancer | Усиливает защиту и мощь питомцев. | साथियों की रक्षा और शक्ति बढ़ाता है। | Memperkuat pertahanan dan kekuatan peliharaan. | Tăng phòng thủ và sức mạnh của thú nuôi. | เพิ่มพลังป้องกันและพลังโจมตีของสัตว์เลี้ยง | يعزز دفاع المرافقين وقوتهم.
Plague | Снижает MP цели и накладывает различные ослабления, например оглушение, проклятие, отравление и замедление. | लक्ष्य का MP घटाता है और कई हानिकारक प्रभाव डालता है, जैसे स्तब्धता, अभिशाप, विष और मंदता। | Mengurangi MP sasaran dan memberikan berbagai efek pelemahan, misalnya pingsan, kutukan, racun, dan perlambatan. | Giảm MP của mục tiêu và gây nhiều hiệu ứng bất lợi, chẳng hạn choáng, nguyền rủa, trúng độc và làm chậm. | ลด MP ของเป้าหมายและทำให้ติดสถานะด้านลบต่าง ๆ เช่น มึนงง คำสาป พิษ และเชื่องช้า | يقلل MP الهدف ويصيبه بتأثيرات إضعاف مختلفة، مثل الصعق واللعنة والتسمم والإبطاء.
PoisonCloud | Бросает талисман, создавая в области очень сильное ядовитое облако. | ताबीज़ फेंकता है, जिससे उस क्षेत्र में अत्यंत शक्तिशाली विष मेघ बनता है। | Melempar jimat untuk memunculkan awan racun yang sangat kuat di area tersebut. | Ném bùa để tạo đám mây độc rất mạnh trong khu vực. | ขว้างเครื่องรางเพื่อสร้างเมฆพิษที่รุนแรงมากในพื้นที่ | يلقي تعويذة لتظهر سحابة سم شديدة القوة في المنطقة.
PoisonShot | Выпускает ядовитую стрелу, отравляющую врага. | विष बाण छोड़ता है जो शत्रु को विषाक्त करता है। | Menembakkan panah beracun yang meracuni musuh. | Bắn mũi tên độc khiến kẻ địch trúng độc. | ยิงศรพิษที่ทำให้ศัตรูติดพิษ | يطلق سهمًا سامًا يصيب العدو بالتسمم.
PoisonSword | Отравляет монстров ударом меча. Яд наносит им урон с течением времени. | तलवार की चोट से राक्षसों को विषाक्त करता है। विष का प्रभाव समय के साथ उन्हें क्षति पहुँचाता है। | Meracuni monster dengan tebasan pedang. Racun menimbulkan kerusakan seiring waktu. | Chém kiếm khiến quái vật trúng độc. Chất độc gây sát thương theo thời gian. | ฟันดาบให้มอนสเตอร์ติดพิษ พิษจะสร้างความเสียหายอย่างต่อเนื่อง | يسمم الوحوش بضربة من سيفك. يسبب السم ضررًا للوحش مع مرور الوقت.
ProtectionField | Сосредотачивает внутреннюю силу и распределяет её по всему телу, усиливая защиту от врагов. Сила защиты и длительность зависят от уровня навыка. После применения нужно подождать, прежде чем использовать навык снова. | आंतरिक शक्ति को एकत्र करके पूरे शरीर में फैलाता है, जिससे शत्रुओं से रक्षा बढ़ती है। रक्षा की शक्ति और अवधि कौशल स्तर पर निर्भर हैं। दोबारा प्रयोग करने से पहले प्रतीक्षा करनी होगी। | Memusatkan tenaga dalam dan menyebarkannya ke seluruh tubuh untuk memperkuat perlindungan dari musuh. Kekuatan pertahanan dan durasi bergantung pada tingkat keterampilan. Setelah digunakan, harus menunggu sebelum dapat menggunakannya lagi. | Tập trung nội lực rồi lan tỏa khắp cơ thể để tăng khả năng phòng vệ trước kẻ địch. Sức phòng thủ và thời gian duy trì phụ thuộc vào cấp kỹ năng. Sau khi dùng phải chờ một thời gian mới có thể dùng lại. | รวมพลังภายในแล้วกระจายทั่วร่างกายเพื่อเพิ่มการป้องกันจากศัตรู พลังป้องกันและระยะเวลาขึ้นอยู่กับระดับวิชา หลังใช้ต้องรอก่อนจึงจะใช้ได้อีกครั้ง | يركز القوة الداخلية وينشرها في أنحاء الجسم لتعزيز الحماية من الأعداء. تعتمد قوة الدفاع ومدته على مستوى المهارة. بعد استخدامها يجب الانتظار قبل استعمالها مجددًا.
Purification | Помогает другим избавиться от отравления и паралича. | दूसरों को विष और लकवे से मुक्त करता है। | Membantu orang lain pulih dari racun dan kelumpuhan. | Giúp người khác thoát khỏi trạng thái trúng độc và tê liệt. | ช่วยผู้อื่นให้หายจากพิษและอัมพาต | يساعد الآخرين على التعافي من التسمم والشلل.
Rage | Усиливает внутреннюю силу, временно повышая мощь атаки. Сила атаки и длительность зависят от уровня навыка. После применения нужно подождать, прежде чем использовать навык снова. | आंतरिक शक्ति बढ़ाकर कुछ समय के लिए प्रहार की शक्ति बढ़ाता है। प्रहार की शक्ति और अवधि कौशल स्तर पर निर्भर हैं। दोबारा प्रयोग करने से पहले प्रतीक्षा करनी होगी। | Memperkuat tenaga dalam untuk meningkatkan kekuatan serangan selama waktu tertentu. Kekuatan serangan dan durasi bergantung pada tingkat keterampilan. Setelah digunakan, harus menunggu sebelum dapat menggunakannya lagi. | Cường hóa nội lực để tăng sức tấn công trong một thời gian. Sức tấn công và thời gian duy trì phụ thuộc vào cấp kỹ năng. Sau khi dùng phải chờ một thời gian mới có thể dùng lại. | เสริมพลังภายในเพื่อเพิ่มพลังโจมตีชั่วคราว พลังโจมตีและระยะเวลาขึ้นอยู่กับระดับวิชา หลังใช้ต้องรอก่อนจึงจะใช้ได้อีกครั้ง | يعزز القوة الداخلية لزيادة قوة الهجوم لفترة معينة. تعتمد قوة الهجوم ومدته على مستوى المهارة. بعد استخدامها يجب الانتظار قبل استعمالها مجددًا.
Reincarnation | Воскрешает погибшего игрока. | मृत खिलाड़ी को पुनर्जीवित करता है। | Menghidupkan kembali pemain yang telah mati. | Hồi sinh một người chơi đã chết. | ชุบชีวิตผู้เล่นที่ตายแล้ว | يحيي لاعبًا ميتًا.
Repulsion | Силой огня отталкивает окружающие цели. | अग्नि की शक्ति से आसपास के लक्ष्यों को दूर धकेलता है। | Mendorong sasaran di sekitar menjauh dengan kekuatan api. | Dùng sức mạnh lửa đẩy lùi các mục tiêu xung quanh. | ใช้พลังไฟผลักเป้าหมายรอบตัวออกไป | يدفع الأهداف المحيطة بعيدًا باستخدام قوة النار.
Revelation | Позволяет видеть HP других. | दूसरों का HP देखने देता है। | Memungkinkan Anda melihat HP pihak lain. | Cho phép bạn xem HP của người khác. | ทำให้คุณมองเห็น HP ของผู้อื่น | يتيح لك رؤية HP الآخرين.
ShoulderDash | Воин бросается на цель плечом, отталкивая её назад. Если цель ударится о препятствие, она получит урон. | योद्धा कंधे से टक्कर मारकर लक्ष्य को पीछे धकेलता है। लक्ष्य किसी बाधा से टकराए तो उसे क्षति होती है। | Prajurit menerjang sasaran dengan bahu untuk mendorongnya mundur. Sasaran menerima kerusakan jika menabrak rintangan. | Chiến binh lao vai vào mục tiêu để đẩy lùi nó. Mục tiêu chịu sát thương nếu va vào chướng ngại vật. | นักรบพุ่งชนเป้าหมายด้วยไหล่ให้ถอยหลัง หากเป้าหมายชนสิ่งกีดขวางจะได้รับความเสียหาย | يندفع المحارب بكتفه نحو الهدف ليدفعه إلى الخلف. يتلقى الهدف ضررًا إذا اصطدم بعائق.
SlashingBurst | Позволяет воину перепрыгнуть через объект или монстра на 1 клетку. | योद्धा को वस्तु या राक्षस के ऊपर से 1 खाने की छलाँग लगाने देता है। | Memungkinkan prajurit melompat 1 petak melewati objek atau monster. | Cho phép chiến binh nhảy 1 ô qua vật thể hoặc quái vật. | ทำให้นักรบกระโดดข้ามวัตถุหรือมอนสเตอร์เป็นระยะ 1 ช่อง | يسمح للمحارب بالقفز مسافة 1 خانة فوق جسم أو وحش.
SoulShield | Благословляет заклинателя и членов группы, повышая их магическую защиту. | मंत्रकर्ता और दल के सदस्यों को आशीर्वाद देकर उनकी जादुई रक्षा बढ़ाता है। | Memberkati perapal dan anggota kelompok untuk meningkatkan pertahanan sihir mereka. | Ban phúc cho người thi triển và các thành viên tổ đội, tăng phòng thủ phép của họ. | อวยพรผู้ร่ายและสมาชิกปาร์ตี้เพื่อเพิ่มพลังป้องกันเวทมนตร์ | يبارك ملقي التعويذة وأعضاء الفريق لتعزيز دفاعهم السحري.
Stonetrap | Устанавливает каменную ловушку. | पाषाण जाल बिछाता है। | Memasang jebakan batu. | Đặt một bẫy đá. | วางกับดักหิน | يضع فخًا حجريًا.
StormEscape | Парализует ближайших врагов и перемещает вас в указанное место. | पास के शत्रुओं को लकवाग्रस्त करके आपको चुने हुए स्थान पर पहुँचाता है। | Melumpuhkan musuh di dekat Anda dan berpindah ke lokasi yang ditentukan. | Làm tê liệt kẻ địch ở gần rồi dịch chuyển bạn đến vị trí chỉ định. | ทำให้ศัตรูใกล้เคียงเป็นอัมพาตแล้ววาร์ปคุณไปยังตำแหน่งที่เลือก | يشل الأعداء القريبين وينقلك إلى الموضع المحدد.
StraightShot | Наполняет стрелу маной, чтобы нанести дополнительный урон. | बाण में माना भरकर अतिरिक्त क्षति पहुँचाता है। | Mengisi panah dengan mana untuk menimbulkan kerusakan tambahan. | Truyền pháp lực vào mũi tên để gây thêm sát thương. | ใส่มานาลงในศรเพื่อสร้างความเสียหายเพิ่มเติม | يشبع السهم بالمانا لإحداث ضرر إضافي.
SummonHolyDeva | Призывает святого духа, который атакует монстров мощными разрядами молнии. | पवित्र आत्मा बुलाता है, जो शक्तिशाली बिजली से राक्षसों पर आक्रमण करती है। | Memanggil roh suci yang menyerang monster dengan petir kuat. | Triệu hồi linh thể thiêng liêng, dùng sấm sét mạnh để tấn công quái vật. | อัญเชิญวิญญาณศักดิ์สิทธิ์ที่จะใช้สายฟ้าอันรุนแรงโจมตีมอนสเตอร์ | يستدعي روحًا مقدسة تهاجم الوحوش ببرق قوي.
SummonShinsu | Призывает собаку, которая сражается рядом с вами. | ऐसा कुत्ता बुलाता है जो आपके साथ मिलकर लड़ता है। | Memanggil anjing yang bertarung di sisi Anda. | Triệu hồi một con chó để chiến đấu bên bạn. | อัญเชิญสุนัขที่จะต่อสู้เคียงข้างคุณ | يستدعي كلبًا يقاتل إلى جانبك.
SummonSnakes | Призывает тотем, порождающий рой змей. Змеи привлекают внимание всех ближайших монстров и атакуют с шансом парализовать цель. | एक टोटम बुलाता है जो साँपों का झुंड पैदा करता है। साँप पास के सभी राक्षसों का ध्यान खींचकर आक्रमण करते हैं और लक्ष्य को लकवाग्रस्त कर सकते हैं। | Memanggil totem yang mengeluarkan kawanan ular. Ular menarik perhatian semua monster di sekitar dan menyerang dengan peluang melumpuhkan sasaran. | Triệu hồi vật tổ sinh ra một đàn rắn. Rắn thu hút tất cả quái vật gần đó và tấn công với cơ hội làm tê liệt mục tiêu. | อัญเชิญโทเทมที่สร้างฝูงงู งูจะดึงความสนใจของมอนสเตอร์ใกล้เคียงทั้งหมดและโจมตี โดยมีโอกาสทำให้เป้าหมายเป็นอัมพาต | يستدعي طوطمًا يطلق سربًا من الأفاعي. تجذب الأفاعي انتباه جميع الوحوش القريبة وتهاجم مع فرصة لشل الهدف.
SummonToad | Призывает жабу, сражающуюся рядом с вами. Жаба не может двигаться и взорвётся, если хозяин выйдет из её поля зрения. | भेक बुलाता है जो आपके साथ लड़ता है। भेक चल नहीं सकता और मालिक के उसकी दृष्टि-सीमा से बाहर जाने पर फट जाता है। | Memanggil katak untuk bertarung di sisi Anda. Katak tidak dapat bergerak dan akan meledak jika tuannya keluar dari jangkauan pandangannya. | Triệu hồi cóc để chiến đấu bên bạn. Cóc không thể di chuyển và sẽ phát nổ nếu chủ nhân ra khỏi tầm nhìn của nó. | อัญเชิญคางคกเพื่อต่อสู้เคียงข้างคุณ คางคกเคลื่อนที่ไม่ได้ และจะระเบิดหากเจ้าของออกจากระยะมองเห็นของมัน | يستدعي علجومًا ليقاتل إلى جانبك. لا يستطيع العلجوم الحركة، وينفجر إذا غادر صاحبه نطاق رؤيته.
SummonVampire | Призывает паука-вампира, сражающегося рядом с вами. Паук похищает HP врагов, исцеляя заклинателя. | पिशाच मकड़ी बुलाता है जो आपके साथ लड़ती है। मकड़ी शत्रु का HP चूसकर मंत्रकर्ता का उपचार करती है। | Memanggil laba-laba vampir untuk bertarung di sisi Anda. Laba-laba menyerap HP musuh untuk menyembuhkan perapal. | Triệu hồi nhện hút máu để chiến đấu bên bạn. Nhện hút HP của kẻ địch để hồi máu cho người thi triển. | อัญเชิญแมงมุมแวมไพร์เพื่อต่อสู้เคียงข้างคุณ แมงมุมจะดูด HP ของศัตรูเพื่อรักษาผู้ร่าย | يستدعي عنكبوتًا مصاصًا للدماء ليقاتل إلى جانبك. يمتص العنكبوت HP الأعداء لشفاء ملقي المهارة.
SwiftFeet | Пока навык активен, повышает скорость бега. | सक्रिय रहने पर दौड़ने की गति बढ़ाता है। | Meningkatkan kecepatan berlari selama aktif. | Tăng tốc độ chạy trong khi kỹ năng có hiệu lực. | เพิ่มความเร็วในการวิ่งขณะวิชามีผล | يزيد سرعة الركض أثناء تفعيل المهارة.
Teleport | Перемещает в случайное место. | किसी यादृच्छिक स्थान पर पहुँचाता है। | Berpindah ke lokasi acak. | Dịch chuyển đến một vị trí ngẫu nhiên. | วาร์ปไปยังตำแหน่งสุ่ม | ينقلك إلى موضع عشوائي.
ThunderStorm | Создаёт вокруг заклинателя грозовую бурю, наносящую урон всем врагам-нежити в её пределах. | मंत्रकर्ता के चारों ओर विद्युत तूफ़ान बनाता है, जो सीमा के भीतर सभी मृतजीव शत्रुओं को क्षति पहुँचाता है। | Menciptakan badai petir di sekitar perapal yang melukai semua musuh mayat hidup dalam jangkauannya. | Tạo bão sét quanh người thi triển, gây sát thương lên tất cả kẻ địch xác sống trong phạm vi. | สร้างพายุสายฟ้ารอบผู้ร่าย ซึ่งสร้างความเสียหายแก่ศัตรูอันเดดทั้งหมดในระยะ | ينشئ عاصفة رعدية حول ملقي التعويذة، تلحق الضرر بكل الأعداء من الموتى الأحياء ضمن نطاقها.
Trap | На короткое время удерживает монстра в ловушке. | थोड़ी देर के लिए राक्षस को जाल में फँसाता है। | Menjebak monster untuk waktu singkat. | Giam giữ quái vật trong một khoảng thời gian ngắn. | กักมอนสเตอร์ไว้ชั่วครู่ | يحبس الوحش في فخ لفترة قصيرة.
TrapHexagon | Магией удерживает монстра на месте. Любой урон от внешнего источника снова позволит монстру двигаться. | जादू से राक्षस को रोकता है ताकि वह हिल न सके। किसी बाहरी स्रोत की क्षति लगते ही राक्षस फिर चल सकता है। | Menahan monster dengan sihir agar tidak dapat bergerak. Kerusakan apa pun dari sumber luar akan membuat monster dapat bergerak lagi. | Dùng phép giữ quái vật tại chỗ. Bất kỳ sát thương nào từ nguồn bên ngoài cũng khiến quái vật có thể di chuyển trở lại. | ใช้เวทมนตร์กักมอนสเตอร์ไม่ให้เคลื่อนไหว ความเสียหายใด ๆ จากภายนอกจะทำให้มอนสเตอร์กลับมาเคลื่อนที่ได้ | يحبس الوحش بقوة سحرية تمنعه من الحركة. أي ضرر من مصدر خارجي يسمح للوحش بالحركة مجددًا.
TwinDrakeBlade | Наносит несколько мощных атак. Есть небольшой шанс ненадолго оглушить цель. Оглушённые монстры получают дополнительно 50% урона. | कई शक्तिशाली प्रहार करता है। लक्ष्य को थोड़े समय के लिए स्तब्ध करने की कम संभावना है। स्तब्ध राक्षसों को 50% अतिरिक्त क्षति होती है। | Melancarkan beberapa serangan kuat. Memiliki peluang kecil membuat sasaran pingsan sesaat. Monster yang pingsan menerima kerusakan tambahan sebesar 50%. | Tung nhiều đòn tấn công mạnh. Có cơ hội nhỏ gây choáng mục tiêu trong thời gian ngắn. Quái vật bị choáng nhận thêm 50% sát thương. | โจมตีอย่างรุนแรงหลายครั้ง มีโอกาสเล็กน้อยทำให้เป้าหมายมึนงงชั่วครู่ มอนสเตอร์ที่มึนงงจะได้รับความเสียหายเพิ่ม 50% | ينفذ عدة هجمات قوية. توجد فرصة منخفضة لصعق الهدف مؤقتًا. تتلقى الوحوش المصعوقة ضررًا إضافيًا بنسبة 50%.
UltimateEnhancer | Поглощает энергию окружения, повышая характеристики. | आसपास की ऊर्जा सोखकर आँकड़े बढ़ाता है। | Menyerap energi dari sekitar untuk meningkatkan atribut. | Hấp thụ năng lượng xung quanh để tăng các chỉ số. | ดูดซับพลังงานรอบตัวเพื่อเพิ่มค่าสถานะ | يمتص الطاقة من المحيط لزيادة الإحصاءات.
VampireShot | Выпускает вампирскую стрелу, похищающую HP врага и исцеляющую заклинателя. | रक्तचूषक बाण छोड़ता है, जो शत्रु का HP चूसकर मंत्रकर्ता का उपचार करता है। | Menembakkan panah vampir yang menyerap HP musuh untuk menyembuhkan perapal. | Bắn mũi tên hút HP của kẻ địch để hồi máu cho người thi triển. | ยิงศรดูดเลือดที่ดูด HP ของศัตรูเพื่อรักษาผู้ร่าย | يطلق سهمًا مصاصًا للدماء يمتص HP العدو لشفاء ملقي المهارة.
`);

const tooltipModes = {
  'passive skill': ['Пассивный навык', 'निष्क्रिय कौशल', 'Keterampilan Pasif', 'Kỹ năng bị động', 'วิชาติดตัว', 'مهارة سلبية'],
  'toggle skill': ['Переключаемый навык', 'चालू/बंद कौशल', 'Keterampilan Aktif/Nonaktif', 'Kỹ năng bật/tắt', 'วิชาเปิด/ปิด', 'مهارة قابلة للتبديل'],
  'active skill': ['Активный навык', 'सक्रिय कौशल', 'Keterampilan Aktif', 'Kỹ năng chủ động', 'วิชาใช้งาน', 'مهارة نشطة'],
  'buff skill': ['Усиливающий навык', 'सुदृढ़ीकरण कौशल', 'Keterampilan Penguatan', 'Kỹ năng cường hóa', 'วิชาเสริมพลัง', 'مهارة تعزيز'],
  'instant casting': ['Мгновенное применение', 'तत्काल प्रयोग', 'Penggunaan Seketika', 'Thi triển tức thì', 'ร่ายทันที', 'إلقاء فوري'],
  'channelling casting': ['Поддерживаемое заклинание', 'निरंतर मंत्र प्रयोग', 'Perapalan Berkelanjutan', 'Thi triển duy trì', 'ร่ายแบบต่อเนื่อง', 'إلقاء مستمر'],
  'trap skill': ['Навык ловушки', 'जाल कौशल', 'Keterampilan Jebakan', 'Kỹ năng đặt bẫy', 'วิชากับดัก', 'مهارة فخ'],
  'summoning skill': ['Навык призыва', 'आह्वान कौशल', 'Keterampilan Pemanggilan', 'Kỹ năng triệu hồi', 'วิชาอัญเชิญ', 'مهارة استدعاء'],
  'lasting effect': ['Продолжительное действие', 'निरंतर प्रभाव', 'Efek Berkelanjutan', 'Hiệu ứng kéo dài', 'ผลต่อเนื่อง', 'تأثير مستمر']
};
const tooltipNameAliases = { CrescentSlash: 'Cresent Slash', UltimateEnhancer: 'Ultimate Enchancer' };
const absentTooltipSkills = ['BattleCry', 'BindingShot', 'CatTongue', 'FireBounce', 'MentalState', 'MeteorShower', 'Portal'];
// Pins all 102 canonical English descriptions and 103 legacy description keys.
// A changed source requires review before old effect prose may be reused.
const reviewedTooltipSourceDigest = 'b2dced6e2bf8446e0e5c68a65859d162e668a0d9ec47d4e07a19826fb7c5ef3c';
const numericLiterals = (text) => (text.replace(/\{[^{}]*\}/g, '').match(/\d+(?:[x×]\d+)?%?/g) ?? []).sort();
const statLiterals = (text) => (text.match(/\b(?:HP|MP|AC|AMC|MAC|DC|MC|SC)\b/gi) ?? []).map((s) => s.toUpperCase()).sort();

function tooltipStructure(source) {
  const lines = source.split('\n').map((s) => s.trim()).filter(Boolean);
  const title = lines.shift();
  const kinds = [];
  const bodyLines = [];
  let mana = false, perAttack = false, cooldown = null, required = null;
  for (const line of lines) {
    const mode = line.toLowerCase() === 'instant castin' ? 'instant casting' : line.toLowerCase();
    if (Object.hasOwn(tooltipModes, mode)) { if (!kinds.includes(mode)) kinds.push(mode); continue; }
    if (line === 'Current Skill Level {0}' || line === 'Next Level {1}') continue;
    const cost = /^Mana Cost:?\s+\{2\}( per attack)?$/.exec(line);
    if (cost) { assert.equal(mana, false, 'duplicate mana line'); mana = true; perAttack = Boolean(cost[1]); continue; }
    const wait = /^Cooldown Time (\d+) secs$/.exec(line);
    if (wait) { assert.equal(cooldown, null, 'duplicate cooldown line'); cooldown = wait[1]; continue; }
    const items = /^Required Items: (.+?)\.?$/.exec(line);
    if (items) { assert.equal(required, null, 'duplicate required-items line'); required = items[1].split(' + '); continue; }
    bodyLines.push(line);
  }
  return { title, kinds, mana, perAttack, cooldown, required, sourceBody: bodyLines.join(' ') };
}

function translatedTooltip(entry, tooltipBySource = null) {
  const skill = /^content\.magic\.([A-Za-z]+)\.description$/.exec(entry.key)?.[1]
    ?? (/^client\..+SkillDescription$/.test(entry.key) ? tooltipBySource?.get(entry.en) : null);
  const body = tooltipBodies.get(skill);
  if (!body) return null;
  const structure = tooltipStructure(entry.en);
  const manaCost = ['Расход MP: {2}', 'MP लागत: {2}', 'Biaya MP: {2}', 'Tiêu hao MP: {2}', 'ใช้ MP: {2}', 'تكلفة MP: {2}'];
  const perAttack = ['за атаку', 'प्रति आक्रमण', 'per serangan', 'mỗi đòn đánh', 'ต่อการโจมตี', 'لكل هجوم'];
  const requires = ['Требуется', 'आवश्यक वस्तु', 'Barang yang Diperlukan', 'Vật phẩm cần có', 'ไอเทมที่ต้องใช้', 'الأداة المطلوبة'];
  const cooldown = ['Время восстановления: {seconds} сек.', 'फिर प्रयोग से पहले प्रतीक्षा: {seconds} सेकंड', 'Waktu Jeda: {seconds} detik', 'Thời gian hồi: {seconds} giây', 'เวลารอใช้ซ้ำ: {seconds} วินาที', 'وقت إعادة الاستخدام: {seconds} ثانية'];
  const currentLevel = ['Текущий уровень навыка {0}', 'वर्तमान कौशल स्तर {0}', 'Tingkat Keterampilan Saat Ini {0}', 'Cấp kỹ năng hiện tại {0}', 'ระดับวิชาปัจจุบัน {0}', 'مستوى المهارة الحالي {0}'];
  const nextLevel = ['Следующий уровень {1}', 'अगला स्तर {1}', 'Tingkat Berikutnya {1}', 'Cấp tiếp theo {1}', 'ระดับถัดไป {1}', 'المستوى التالي {1}'];
  const displayName = term(tooltipNameAliases[skill] ?? skill);
  if (!displayName) throw new Error(`Missing skill-name terminology: ${skill}`);
  const requiredItems = structure.required?.map((name) => {
    const value = term(name);
    if (!value) throw new Error(`Unreviewed skill item requirement: ${skill}/${name}`);
    return value;
  });
  const values = Object.fromEntries(locales.map((locale, i) => {
    const header = structure.kinds.map((kind) => tooltipModes[kind][i]);
    if (structure.cooldown) header.push(cooldown[i].replace('{seconds}', structure.cooldown));
    if (structure.mana) header.push(`${manaCost[i]}${structure.perAttack ? ` ${perAttack[i]}` : ''}`);
    const blocks = [displayName[locale]];
    if (header.length) blocks.push(header.join('\n'));
    if (requiredItems) blocks.push(`${requires[i]}: ${requiredItems.map((item) => item[locale]).join(' + ')}`);
    blocks.push(body[i], `${currentLevel[i]}\n${nextLevel[i]}`);
    const value = blocks.join('\n\n');
    assert.deepEqual(numericLiterals(value), numericLiterals(entry.en), `Changed skill numerals: ${entry.key}/${locale}`);
    assert.deepEqual(statLiterals(body[i]), statLiterals(structure.sourceBody), `Changed effect stat literals: ${entry.key}/${locale}`);
    return [locale, value];
  }));
  return { values, rule: 'reviewed-skill-tooltip', skill };
}

export function terminology() { return Object.fromEntries(terms); }

const compactTerms = new Map([...terms].map(([key, values]) => [nameKey(key), values]));
const compactGroups = new Map([...groups].map(([key, group]) => [nameKey(key), group]));
const copy = (values) => ({ ...values });
const transform = (values, fn) => Object.fromEntries(locales.map((locale, index) => [locale, fn(values[locale], locale, index)]));
const sameEverywhere = (text) => Object.fromEntries(locales.map((locale) => [locale, text]));
const term = (source) => compactTerms.get(nameKey(source));
const levelWords = ['Уровень', 'स्तर', 'Level', 'Cấp', 'ระดับ', 'المستوى'];
const bundleWords = ['набор', 'गठरी', 'bundel', 'bó', 'มัด', 'حزمة'];
const sourceFiles = ['common', 'content', 'npc-menus', 'npc-prose'];
const allCatalogFiles = [...sourceFiles, 'game', 'help', 'quest', 'shell'];
const properNames = new Set(['Oma', 'Shinsu', 'Yimoogi']);
const roleAliases = {
  Merchant: 'Merchant', TravellingMerchant: 'Travelling Merchant',
  MaterialDealer: 'Material Dealer', TrustMerchant: 'Trust Merchant',
  InnKeeper: 'Inn Keeper', Blacksmith: 'Blacksmith', CraftsLady: 'Crafts Lady',
  Master: 'Master', HighPriest: 'High Priest', HighAssassin: 'High Assassin',
  Captain: 'Captain', Alchemist: 'Alchemist', Lottery: 'Lottery',
  MirGuide: 'Mir Guide', Assistant: 'Assistant', Investigator: 'Investigator',
  LeadTrainer: 'Lead Trainer', CaveGuide: 'Cave Guide', Sailor: 'Sailor',
  Transport: 'Transport', Protector: 'Protector', Specialist: 'Specialist',
  Librarian: 'Librarian', General: 'General', Administrator: 'Administrator',
  OldFisherman: 'Old Fisherman', WiseFisherman: 'Wise Fisherman',
  Inspector: 'Inspector', VillageChief: 'Village Chief', MasterMage: 'Master Mage',
  FishMonger: 'Fish Monger', SquadLeader: 'Squad Leader', Hairdresser: 'Hairdresser',
  Wanderer: 'Wanderer', Examiner: 'Examiner', PetMaster: 'Pet Master',
  Commander: 'Commander', Traveller: 'Traveller', Teleport: 'Teleporter',
  Warehouse: 'Warehouse keeper', Storage: 'Warehouse keeper', Drapery: 'Cloth merchant',
  PotionShop: 'Potion merchant', Grocery: 'Grocery merchant', Accessory: 'Accessory merchant',
  Book: 'Book merchant'
};

function translatedStat(entry) {
  // These are notation, not untranslated sentences. The UI retains canonical
  // stat abbreviations; no packet/schema stat is renamed.
  if (/^(?:HP|MP|AC|MAC|DC|MC|SC)(?:\s*[:+]\s*\{\d+\}(?:~\{\d+\})?)?$/.test(entry.en.trim())) {
    return { values: sameEverywhere(entry.en), rule: 'standard-stat-notation' };
  }
  let match = /^(Health|Mana|Accuracy|Agility|Attack Speed|Luck|Durability|Weight):?\s*([+:]?\s*\{\d+\}(?:~\{\d+\})?%?)$/.exec(entry.en.trim());
  if (match && term(match[1])) return { values: transform(term(match[1]), (value) => `${value}: ${match[2]}`), rule: 'stat-label-template' };
  match = /^Adds \+\{0\} (AC|MAC|DC|MC|SC)$/.exec(entry.en.trim());
  if (match) {
    const phrases = ['Добавляет +{0}', '+{0} जोड़ता है:', 'Menambah +{0}', 'Tăng +{0}', 'เพิ่ม +{0}', 'يضيف +{0}'];
    return { values: Object.fromEntries(locales.map((locale, i) => [locale, `${phrases[i]} ${match[1]}`])), rule: 'stat-add-template' };
  }
  match = /^Required (Base )?(AC|MAC|DC|MC|SC)\s*:\s*(\{0\})$/.exec(entry.en.trim());
  if (match) {
    const phrases = match[1]
      ? ['Требование к базовому', 'आवश्यक आधार', 'Dasar yang Diperlukan', 'Chỉ số gốc cần có', 'ค่าพื้นฐานที่ต้องการ', 'القيمة الأساسية المطلوبة']
      : ['Требуется', 'आवश्यक', 'Diperlukan', 'Cần có', 'ค่าที่ต้องการ', 'المطلوب'];
    return { values: Object.fromEntries(locales.map((locale, i) => [locale, `${phrases[i]} ${match[2]}: ${match[3]}`])), rule: 'stat-requirement-template' };
  }
  return null;
}

function curated(entry, catalog, tooltipBySource = null) {
  const common = translatedReviewedCommon(entry, catalog);
  if (common) return common;
  if (Object.hasOwn(descriptions, entry.key)) {
    return { values: Object.fromEntries(locales.map((locale, i) => [locale, descriptions[entry.key][i]])), rule: 'gameplay-description' };
  }
  const tooltip = translatedTooltip(entry, tooltipBySource);
  if (tooltip) return tooltip;
  if (entry.key.startsWith('content.map.') && entry.en === 'Blacksmith') {
    return { values: copy(term('Blacksmith shop')), rule: 'map-context' };
  }
  const exact = terms.get(normalize(entry.en));
  if (exact) return { values: copy(exact), rule: `term:${groups.get(normalize(entry.en))}` };
  const stat = translatedStat(entry);
  if (stat) return stat;

  if (catalog === 'content' && entry.key.endsWith('.name')) {
    // Only catalogue names may use canonical CamelCase aliases. Arbitrary NPC
    // prose and player-controlled values are never split into words.
    for (const candidate of [entry.en, ...(entry.aliases ?? [])]) {
      const compact = term(candidate);
      if (compact) return { values: copy(compact), rule: `name-alias:${compactGroups.get(nameKey(candidate))}` };
    }
    const numberSuffix = /^(.*?)(?:\s*)(\d+)$/.exec(entry.en);
    if (numberSuffix && term(numberSuffix[1])) {
      return { values: transform(term(numberSuffix[1]), (value) => `${value} ${numberSuffix[2]}`), rule: 'numeric-variant', retained: numberSuffix[2] };
    }
    const bundle = /^(.*?)(?:\s*)\(Bundle\)$/.exec(entry.en);
    if (bundle && term(bundle[1])) {
      return { values: transform(term(bundle[1]), (value, _locale, i) => `${value} (${bundleWords[i]})`), rule: 'item-bundle' };
    }
    const sizeCode = /^(.*?)\s+(\([SML]\))$/.exec(entry.en);
    if (sizeCode && term(sizeCode[1])) {
      return { values: transform(term(sizeCode[1]), (value) => `${value} ${sizeCode[2]}`), rule: 'item-size-code', retained: sizeCode[2] };
    }
    if (entry.key.startsWith('content.map.')) {
      const trailingFloor = /^(.*?)\s+(\d+F|B\d+)$/.exec(entry.en);
      const leadingFloor = /^(B\d+|\d+F)(?:of\s*|\s*)(.+)$/.exec(entry.en);
      const direction = /^(.*?)\s+(\([NSEW]\))$/.exec(entry.en);
      if (trailingFloor && term(trailingFloor[1])) {
        return { values: transform(term(trailingFloor[1]), (value) => `${value} ${trailingFloor[2]}`), rule: 'map-floor', retained: trailingFloor[2] };
      }
      if (leadingFloor && term(leadingFloor[2])) {
        return { values: transform(term(leadingFloor[2]), (value) => `${value} ${leadingFloor[1]}`), rule: 'map-floor', retained: leadingFloor[1] };
      }
      if (direction && term(direction[1])) {
        return { values: transform(term(direction[1]), (value) => `${value} ${direction[2]}`), rule: 'map-direction-code', retained: direction[2] };
      }
    }
    if (entry.key.startsWith('content.npc.')) {
      for (const alias of entry.aliases ?? []) {
        const match = /^([^_]+)_(.+)$/.exec(alias);
        if (!match) continue;
        const [, role, name] = match;
        if (Object.hasOwn(roleAliases, role) && /^[A-Za-z][A-Za-z.' -]*$/.test(name)) {
          return { values: transform(term(roleAliases[role]), (value) => `${value} ${name}`), rule: 'npc-job-and-proper-name', retained: name };
        }
        if (['GM', 'GT', 'Gt'].includes(role) && term(name)) {
          return { values: transform(term(name), (value) => `${role} ${value}`), rule: 'npc-standard-prefix', retained: role };
        }
        if (role === 'GTMerchant' && /^[A-Za-z][A-Za-z.' -]*$/.test(name)) {
          return { values: transform(term('Merchant'), (value) => `${value} GT ${name}`), rule: 'npc-job-and-proper-name', retained: name };
        }
      }
    }
  }
  if (catalog === 'npc-prose' || catalog === 'npc-menus') {
    const leading = /^Level\s+(\d+):\s*(.+)$/.exec(entry.en);
    const trailing = /^(.+?)\s*\(Level\s+(\d+)\)$/.exec(entry.en);
    const name = leading?.[2] ?? trailing?.[1];
    const level = leading?.[1] ?? trailing?.[2];
    if (name && compactGroups.get(nameKey(name)) === 'skill') {
      return { values: transform(term(name), (value, _locale, i) => `${value} (${levelWords[i]} ${level})`), rule: 'npc-spell-level', retained: level };
    }
  }
  return null;
}

const slots = (text) => [...text.matchAll(/\{[A-Za-z0-9_]+(?::[^{}]+)?\}/g)].map((match) => match[0]).sort();
const chars = (text) => [...text].length;
function verifyOverride(entry, result) {
  if (Object.keys(result.values).join(',') !== locales.join(',')) throw new Error(`Wrong locale fields: ${entry.key}`);
  for (const locale of locales) {
    const value = result.values[locale];
    if (typeof value !== 'string' || !value.trim()) throw new Error(`Empty value: ${entry.key}/${locale}`);
    if (JSON.stringify(slots(value)) !== JSON.stringify(slots(entry.en))) throw new Error(`Changed placeholders: ${entry.key}/${locale}`);
    if (result.retained && !value.includes(result.retained)) throw new Error(`Changed suffix/proper name: ${entry.key}/${locale}`);
    if (value === entry.en && result.rule !== 'standard-stat-notation') {
      const properBase = entry.en.replace(/\s*\d+$/, '').trim();
      const reviewed = result.rule === 'reviewed-common-identical' && reviewedCommonIdentical.some((item) => item.key === entry.key && item.locale === locale && item.value === value);
      if (!properNames.has(properBase) && !reviewed) throw new Error(`Unclassified unchanged text: ${entry.key}/${locale}`);
    }
  }
}

export function buildContentOverrides() {
  const catalogs = Object.fromEntries(allCatalogFiles.map((name) => [name, JSON.parse(fs.readFileSync(path.join(catalogRoot, `${name}.json`), 'utf8')).entries]));
  assert.equal(reviewedCommon.size, 296, 'reviewed common identical-text key count');
  const commonSource = catalogs.common.filter((entry) => reviewedCommon.has(entry.key)).map(({ key, en }) => [key, en]).sort(([a], [b]) => a.localeCompare(b, 'en'));
  assert.equal(commonSource.length, 296, 'all reviewed common keys must exist in the common catalog');
  assert.equal(createHash('sha256').update(JSON.stringify(commonSource)).digest('hex'), reviewedCommonSourceDigest, 'Common source changed; review before regenerating');
  for (const key of commonKeysOwnedByErrorsReview) assert.ok(!reviewedCommon.has(key), `Reserved errors-review key: ${key}`);
  const magicDescriptions = catalogs.content.filter((entry) => /^content\.magic\..+\.description$/.test(entry.key));
  const commonDescriptions = catalogs.common.filter((entry) => /^client\..+SkillDescription$/.test(entry.key));
  const skillSource = [...magicDescriptions, ...commonDescriptions].map(({ key, en }) => [key, en]).sort(([a], [b]) => a.localeCompare(b, 'en'));
  const sourceDigest = createHash('sha256').update(JSON.stringify(skillSource)).digest('hex');
  assert.equal(sourceDigest, reviewedTooltipSourceDigest, 'Skill-description source changed; review before regenerating');
  assert.equal(magicDescriptions.length, 102, 'canonical source-description count');
  assert.equal(tooltipBodies.size, 102, 'manually authored body count');
  const tooltipBySource = new Map(magicDescriptions.map((entry) => [entry.en, entry.key.split('.')[2]]));
  assert.equal(tooltipBySource.size, 102, 'unambiguous exact English tooltip mapping');
  const overrides = {};
  const evidence = [];
  const byKey = new Map();
  for (const name of sourceFiles) {
    for (const entry of catalogs[name]) {
      if (byKey.has(entry.key)) throw new Error(`Duplicate source key: ${entry.key}`);
      byKey.set(entry.key, entry);
      const result = curated(entry, name, tooltipBySource);
      if (!result) continue;
      verifyOverride(entry, result);
      overrides[entry.key] = result.values;
      evidence.push({ key: entry.key, catalog: name, en: entry.en, rule: result.rule, retained: result.retained, skill: result.skill });
    }
  }
  for (const key of Object.keys(descriptions)) {
    if (!Object.hasOwn(overrides, key)) throw new Error(`Required description source key missing: ${key}`);
  }
  const magic = catalogs.content.filter((entry) => entry.key.startsWith('content.magic.') && entry.key.endsWith('.name'));
  if (magic.some((entry) => !Object.hasOwn(overrides, entry.key))) throw new Error(`Uncovered skill: ${magic.filter((entry) => !Object.hasOwn(overrides, entry.key)).map((entry) => entry.key).join(',')}`);
  for (const entry of magicDescriptions) assert.ok(Object.hasOwn(overrides, entry.key), `Missing complete skill description: ${entry.key}`);
  const absent = magic.filter((entry) => !magicDescriptions.some((d) => d.key === entry.key.replace(/\.name$/, '.description'))).map((entry) => entry.key.split('.')[2]).sort();
  assert.deepEqual(absent, absentTooltipSkills, 'source-missing descriptions remain explicitly classified');
  assert.equal(evidence.filter((entry) => entry.catalog === 'common' && entry.rule === 'reviewed-skill-tooltip').length, 102, 'all exact legacy copies use the reviewed tooltip');
  assert.equal(evidence.filter((entry) => entry.catalog === 'common' && entry.rule === 'reviewed-common-identical').length, 296, 'all scoped common review keys must be emitted');
  return { catalogs, overrides: Object.fromEntries(Object.entries(overrides).sort(([a], [b]) => a.localeCompare(b, 'en'))), evidence };
}

function reviewText({ catalogs, overrides, evidence }) {
  const all = Object.values(catalogs).flat();
  const english = new Set(all.map((entry) => entry.en));
  const count = (test) => evidence.filter(test).length;
  const same = evidence.filter((item) => locales.some((locale) => overrides[item.key][locale] === item.en));
  const lines = [
    '# Native six-language content terminology review', '',
    'This is a curated override layer for `ru`, `hi`, `id`, `vi`, `th`, and `ar`.',
    'It does not add a supported locale by itself and is not a full translation or native-speaker acceptance certificate.',
    'The existing `en`, `zh-TW`, and `pt-BR` catalogs are read-only inputs.', '',
    '## Source size and delivered scope', '',
    `- All eight catalogs: ${all.length} stable keys, ${english.size} distinct English strings.`,
    `- English volume: ${all.reduce((n, e) => n + chars(e.en), 0)} Unicode characters by key; ${[...english].reduce((n, text) => n + chars(text), 0)} after exact text deduplication.`,
    `- Delivered: ${Object.keys(overrides).length} keys × six nonempty values = ${Object.keys(overrides).length * locales.length} values.`,
    `- Curated reusable vocabulary: ${terms.size} English terms; ${count((e) => e.key.startsWith('content.magic.') && e.key.endsWith('.name'))} / ${catalogs.content.filter((e) => e.key.startsWith('content.magic.') && e.key.endsWith('.name')).length} canonical skill names.`,
    `- Context-selected full item descriptions: ${count((e) => e.rule === 'gameplay-description')}.`,
    `- Manually authored complete source skill descriptions: ${count((e) => e.catalog === 'content' && e.rule === 'reviewed-skill-tooltip')} / 102 canonical skills × six languages.`,
    `- Exact legacy copies in common receive the same reviewed values: ${count((e) => e.catalog === 'common' && e.rule === 'reviewed-skill-tooltip')} keys; canonical + legacy = ${count((e) => e.rule === 'reviewed-skill-tooltip')} full tooltip keys. These are not 204 distinct skills.`,
    `- Explicit common-catalog identical-text review: ${count((e) => e.catalog === 'common' && e.rule === 'reviewed-common-identical')} / 296 scoped keys × six languages. The seven overlapping errors-review keys are reserved for the separate reviewer.`,
    '- Seven canonical skills have no source description; none is invented. The additional disabled FastMove legacy text is not counted as a supported canonical skill.',
    `- NPC job + proper-name compositions: ${count((e) => e.rule === 'npc-job-and-proper-name')}; numbered spell menu/prose labels: ${count((e) => e.rule === 'npc-spell-level')}.`,
    `- Other ${all.length - Object.keys(overrides).length} keys still need the full baseline translation/review pipeline; none is assigned English fallback here.`, '',
    '| Catalog | Source keys | Distinct English | Unique English characters | Override keys |',
    '| --- | ---: | ---: | ---: | ---: |',
    ...allCatalogFiles.map((name) => {
      const texts = new Set(catalogs[name].map((e) => e.en));
      return `| ${name} | ${catalogs[name].length} | ${texts.size} | ${[...texts].reduce((n, text) => n + chars(text), 0)} | ${count((e) => e.catalog === name)} |`;
    }), '',
    '## Meaning and reuse rules', '',
    '- IDs, canonical map filenames, script commands and network values remain unchanged; overrides are keyed by existing display keys only.',
    '- Warrior / Wizard / Taoist / Assassin / Archer use one terminology set. The same skill label is reused for spellbook items, the magic catalog and exact NPC skill/level lines.',
    '- `Lightning` is the linear lightning-beam skill (疾光電影); `ThunderBolt` is the separate sky-strike spell (雷電術). They must not collapse to one display name.',
    '- `Fencing` raises accuracy. The Town Teleport description preserves “last saved safe zone”, not “main city”. Red poison is described as reducing defences.',
    '- `Finish` means submit/complete the quest, `Return` means navigate back, and quest `Share` means share the quest, not give away rewards or account access.',
    '- HP / MP / DC / MC / SC / AC / MAC remain stable stat abbreviations. A statement containing other English prose does not qualify for this exception.',
    '- Potion sizes are standalone labels; explicitly authored potion names use grammatical gender/case where required. Do not mechanically paste a nominative size label into Russian, Hindi or Arabic prose.',
    '- Name composition is bounded to reviewed full terms, exact canonical name aliases and known NPC-job prefixes. There is no general word-by-word translation of prose or player input.',
    '- Actual NPC personal-name suffixes (e.g. Kyle, Peter, Jane) stay byte-identical. Unreviewed NPC prefixes are left to the baseline pipeline, never passed through as supposedly translated names.',
    '- Russian Bichon geography uses Бичон consistently with the quest override layer; this display transliteration does not change Bichon map IDs or filenames.',
    '- Map floor/direction codes (`1F`, `B1`, `(N)`), item size codes and monster numeric variants are retained exactly. They identify a variant and are not renumbered.',
    '- English spelling errors such as `Cresent Slash` and `Ultimate Enchancer` are matched as existing display sources; stable keys and aliases are not rewritten.', '',
    '## Common UI/context decisions', '',
    '- The 296 common keys are selected explicitly by stable key. These translations do not become a generic English-word replacement for NPC prose or proper names.',
    '- `AutoRun`, bag/hand/equipment weight, soulbound/trade restrictions, attack modes, refine/repair, item types, HP/MP limits, chat/mail controls and visible runtime status are translated for their actual UI use.',
    '- `ui.home` / `ui.end` are chat-scroll-to-beginning/end controls (`apps/web/app/components/original-client-panels.tsx`), not names of keyboard keys. Their six values express scrolling to the start/end.',
    '- `client.BuffEffect` / `client.ValueByOwnerPercent` receive increase/decrease, stat, numeric amount and suffix arguments (`crystal_ui/status_hud.rs`). The translations preserve that quantity relationship and do not invent an actor/owner or force a positive increase.',
    '- Boundary whitespace, line breaks, numeric slots/formats, stat abbreviations, triple-brace item markup and the literal `/ResetConquest [ConquestID]` / `/StartConquest [ConquestID]` commands are retained. Translating help around a command does not enable or execute it.',
    '- `server.Guild` has the English typo “Guid”; its stable key and UI context identify a guild. The translated display label does not rewrite the source key.',
    `- Reserved for the errors reviewer, with no override emitted here: ${commonKeysOwnedByErrorsReview.map((key) => `\`${key}\``).join(', ')}.`,
    `- The 296 exact common English inputs are pinned by SHA-256: \`${reviewedCommonSourceDigest}\`. Source changes require review.`, '',
    '## Identical text policy', '',
    `- ${same.length} covered keys have at least one target value exactly equal to English. Every case is checked by the generator.`,
    '- General unchanged categories are explicit proper creature names `Oma`, `Shinsu`, `Yimoogi` (and their numeric variants), or stat-only notation with unchanged placeholders.',
    '- Common same-form words are permitted only by the exact key, locale and value below; they are not a blanket proper-name exemption. `reviewedIdenticalText()` exports the list for the final integration audit.',
    '', '| Stable key | Locale | Exact value | Review reason |', '| --- | --- | --- | --- |',
    ...reviewedCommonIdentical.map(({ key, locale, value, reason }) => `| ${key} | ${locale} | ${value} | ${reason} |`), '',
    '- Partial proper-name tokens such as Bichon, Wooma, Zuma, GM/GT or a named NPC are intentional inside an otherwise localized name.',
    '- Untranslated quests, item effects, NPC dialogue, menu sentences, or equipment adjectives are not proper names. Matching English is a missing translation unless separately reviewed and justified.',
    '- Preserving player chat, player names and captured runtime parameters is mandatory and is separate from catalog translation coverage.', '',
    '## Validation and integration', '',
    '- Run `node packages/tooling/scripts/native-i18n-expansion-content.mjs --check`; use without `--check` to regenerate these two artifacts.',
    '- Validation checks source-key existence, exactly six fields, nonempty values, exact placeholder multiset (including numeric formats), retained names/codes, all canonical skill names, and classified identical text.',
    '- Every skill tooltip also checks literal numbers/percentages/dimensions and body stat-token counts. Required materials, casting modes, per-attack MP and cooldown lines are read from the pinned source, not inferred from a skill name.',
    `- The 205 canonical/legacy English description inputs are pinned by SHA-256: \`${reviewedTooltipSourceDigest}\`. Source changes stop regeneration until their meaning is reviewed.`,
    '- Self-tests compare all 102 legacy/canonical pairs in six languages and exercise distinct casting modes, no invented MP, required item combinations, cooldowns and critical effect/trigger clauses.',
    '- Common self-tests require all 296 explicit keys, preserve all boundary whitespace/numbers/slots, verify opaque command/markup tokens, reject changing source numerals, and reject reuse of a same-form exemption at a different key/locale. They do not certify native-speaker fluency.',
    '- The module exports `terminology()` for a baseline translator/glossary. Prefer stable-key overrides after translation; never globally replace terminology inside placeholders, links, chat or names.',
    '- This layer owns only common/content/NPC entries. UI-worker shell/quest/game/help overrides must be merged by stable key with conflict reporting, not silently overwritten.',
    '- Full-locale gates still need all remaining strings, source/parameter parity, Arabic shaping and bidi, Devanagari/Thai shaping, font fallback, clipping/wrapping and actual native screenshots.',
    '- A licensed MT baseline is a draft requiring gameplay/context review. Nonempty fields or key counts alone do not establish a complete playable language.', '',
    '## Skill source ambiguities and preserved limits', '',
    'The six-language bodies are manually authored from the full English text and checked against Traditional context. This is source fidelity, not an independent certification that every legacy tooltip matches combat implementation. No gameplay code or original catalog is changed here.', '',
    '| Source | Evidence and treatment |',
    '| --- | --- |',
    '| No source description | BattleCry, BindingShot, CatTongue, FireBounce, MentalState, MeteorShower and Portal remain without invented description keys. |',
    '| Blizzard | English and Traditional describe body protection, strength/duration by skill level and waiting before reuse, effectively the ProtectionField effect. All those clauses are retained under the source channelling label; the apparent copy error needs source-author resolution. |',
    '| CounterAttack | English says AC and AMC, while Traditional describes physical/magic defence. AMC is preserved verbatim and flagged; the translation does not silently assert it is the MAC stat. |',
    '| PoisonCloud | Required-items line says GreenPoison; effect prose says to throw an amulet. Both are retained. No additional required material is guessed. |',
    '| NapalmShot / OneWithNature | The source says “5×5 radius”, mixing dimensions and radius. Translation says an area of 5×5 and preserves the target/caster centre and all-target scope. This does not certify a tile-radius formula. |',
    '| Trap | Source literal 60-second cooldown is retained. The generated magic manifest has delayBase 60000 and delayReduction 15000; the legacy prose does not explain any level-dependent reduction. Do not interpret the translation as a newly fixed cooldown rule. |',
    '| FastMove | client.FastMoveSkillDescription is the only legacy tooltip without a matching canonical description. Its “rooted skills” meaning is unclear. The magic-manifest test explicitly excludes this commented-out placeholder; it receives no invented gameplay description in this layer. |',
    '| Existing Traditional defects | FireBall has a trailing ThunderboltSkillDescription token and ThunderBolt retains English prose. Neither defect is copied into the new six-language text. |',
    '| Formatting/source typos | Lightning “steak” is rendered as a lightning beam using the unambiguous context; IceStorm “Instant Castin” means instant casting; repeated FatalSword “Passive Skill” is rendered once. CrescentSlash has no casting-mode line, so none is fabricated. |',
    '| Material/scope distinctions | BlessedArmour defence and SoulShield magic defence remain distinct; Curse/Plague require Amulet + Poison; PoisonCloud requires green poison and Poisoning powder. SummonSkeleton keeps the source AOE assertion; SummonShinsu keeps the English dog description rather than inventing extra pet abilities. |', '',
    'Names are linked to the existing stable skill IDs. Warrior/archer wording is used only where the source identifies that class; no new profession eligibility, damage coefficient, duration, MP number or pet behaviour is inferred. The current/next-level and MP slots stay {0}/{1}/{2}.', '',
    '## Retained first-draft findings', '',
    'The first Arabic/Hindi Argos drafts were sampled from `C:/mir2-build/i18n-expansion-drafts/{ar,hi}.json` before integration. They were not accepted as a playable locale.', '',
    '| Source / context | Observed first-draft error | Override action |',
    '| --- | --- | --- |',
    '| Fencing tooltip | Arabic “clapping”; Hindi “erecting a fence” | Canonical fencing name and complete accuracy tooltip |',
    '| Lightning tooltip source typo “steak” | Arabic/Hindi translated a beef steak | Explicit lightning-beam effect; no source gameplay change |',
    '| Poisoning tooltip | Arabic interpreted HP as blood pressure | Preserve HP, distinguish green HP loss from red defence reduction |',
    '| Red Poison item effect | Arabic rendered reduces defence as producing defence | Explicit reduction of defences |',
    '| Town Teleport | Arabic safe-zone became a safe/strongbox; Hindi retained English | Last saved safe zone retained in all six languages |',
    '| Not Enough Mana to cast | Hindi draft lost the insufficient-mana condition | Explicit negative feedback in all six languages |',
    '| Quest-share invitation | Hindi dropped sender and the invitation, leaving only acceptance question | Full invitation with the unchanged `{0}` sender slot |',
    '| Core casting/tooltips | Arabic retained English cast/mana lines and lost level slots | Full core tooltips, required items, MP costs and exact slot parity |', '',
    'This sample does not certify the unsampled descriptions or the other four draft languages. Remaining prose needs gameplay review; the draft `problems` list must remain a release blocker until resolved and revalidated.', ''
  ];
  return lines.join('\n');
}

function selfTest(result) {
  assert.notDeepEqual(result.overrides['content.magic.Lightning.name'], result.overrides['content.magic.ThunderBolt.name']);
  assert.deepEqual(result.overrides['content.item.973.name'], result.overrides['content.magic.Fencing.name']);
  assert.equal(result.overrides['content.item.719.description'].en, undefined);
  const variant = curated({ key: 'content.monster.fixture.name', en: 'Zombie 007', aliases: [] }, 'content');
  assert.ok(locales.every((locale) => variant.values[locale].endsWith(' 007')));
  verifyOverride({ key: 'fixture-variant', en: 'Zombie 007' }, variant);
  const npc = curated({ key: 'content.npc.fixture.name', en: 'Merchant Jane', aliases: ['Merchant_Jane'] }, 'content');
  assert.ok(locales.every((locale) => npc.values[locale].endsWith(' Jane')));
  assert.equal(curated({ key: 'npc.prose.fixture', en: 'Merchant Jane says Fencing is best.', aliases: [] }, 'npc-prose'), null);
  assert.equal(curated({ key: 'content.npc.fixture.name', en: 'Unknown Job Jane', aliases: ['UnknownJob_Jane'] }, 'content'), null);
  const correct = { values: sameEverywhere('HP: {0}'), rule: 'standard-stat-notation' };
  verifyOverride({ key: 'fixture-slots', en: 'HP: {0}' }, correct);
  const wrong = { values: { ...correct.values, ar: 'HP: {1}' }, rule: correct.rule };
  assert.throws(() => verifyOverride({ key: 'fixture-slots', en: 'HP: {0}' }, wrong), /Changed placeholders/);
  assert.throws(() => verifyOverride({ key: 'fixture-prose', en: 'Untranslated sentence.' }, { values: sameEverywhere('Untranslated sentence.'), rule: 'term' }), /Unclassified unchanged text/);
  const fakeExtra = { values: { ...correct.values, en: 'HP: {0}' }, rule: correct.rule };
  assert.throws(() => verifyOverride({ key: 'fixture-fields', en: 'HP: {0}' }, fakeExtra), /Wrong locale fields/);

  const commonSource = new Map(result.catalogs.common.map((entry) => [entry.key, entry]));
  assert.equal(reviewedCommon.size, 296);
  for (const key of reviewedCommon.keys()) {
    const source = commonSource.get(key);
    assert.ok(source, `common key still exists: ${key}`);
    assert.ok(!key.startsWith('content.') && !key.startsWith('npc.'), `bounded common review: ${key}`);
    const translated = translatedReviewedCommon(source, 'common');
    assert.deepEqual(result.overrides[key], translated.values);
    assert.equal(translatedReviewedCommon(source, 'npc-menus'), null, 'key-specific common terms must not leak into NPC translation');
    for (const locale of locales) {
      const value = result.overrides[key][locale];
      assert.deepEqual(slots(value), slots(source.en), `${key}/${locale}: exact slot multiset`);
      assert.deepEqual(numericLiterals(value), numericLiterals(source.en), `${key}/${locale}: literal numbers`);
      assert.equal(value.match(/^\s*/)[0], source.en.match(/^\s*/)[0], `${key}/${locale}: leading whitespace`);
      assert.equal(value.match(/\s*$/)[0], source.en.match(/\s*$/)[0], `${key}/${locale}: trailing whitespace`);
    }
  }
  for (const key of commonKeysOwnedByErrorsReview) assert.equal(result.overrides[key], undefined, `leave separate errors ownership intact: ${key}`);
  assert.equal(translatedReviewedCommon({ key: 'fixture-unreviewed', en: 'Menu' }, 'common'), null, 'no generic alias substitution');
  assert.throws(() => translatedReviewedCommon({ key: 'client.Top20Warriors', en: 'TOP 25 Warriors' }, 'common'), /Changed common numerals/);
  for (const [key, command] of [['server.SyntaxResetConquest', '/ResetConquest [ConquestID]'], ['server.SyntaxStartConquest', '/StartConquest [ConquestID]']]) {
    for (const locale of locales) assert.ok(result.overrides[key][locale].endsWith(command), `${key}/${locale}: keep command and its argument literal`);
  }
  for (const locale of locales) {
    assert.ok(result.overrides['server.PetPickedUp'][locale].includes('{{{0}}}'), 'preserve opaque item markup');
    assert.ok(result.overrides['client.BuffEffect'][locale].includes('{2}{3}'), 'value and unit remain adjacent');
    assert.notEqual(result.overrides['ui.home'][locale], 'Home', 'this is a scroll action, not a keyboard name');
    assert.notEqual(result.overrides['ui.end'][locale], 'End', 'this is a scroll action, not a keyboard name');
    assert.ok(result.overrides['client.SoulboundTo'][locale].endsWith(' '), 'a concatenated opaque owner must remain separated');
    for (const stat of ['AC', 'DC', 'MAC', 'MC', 'SC']) {
      const key = `client.Max${stat[0]}${stat.slice(1).toLowerCase()}PlusPercent`;
      assert.deepEqual(statLiterals(result.overrides[key][locale]), [stat], `stat abbreviation: ${key}/${locale}`);
      assert.ok(result.overrides[key][locale].includes('+ {0}%'), `stat formula: ${key}/${locale}`);
    }
  }
  for (const item of reviewedCommonIdentical) {
    assert.equal(result.overrides[item.key][item.locale], item.value);
    assert.equal(commonSource.get(item.key).en, item.value, 'precise same-form source');
    assert.ok(item.reason.length > 30);
  }
  assert.throws(() => verifyOverride({ key: 'fixture-unreviewed-menu', en: 'Menu' }, { values: sameEverywhere('Menu'), rule: 'reviewed-common-identical' }), /Unclassified unchanged text/);
  const wrongLocale = { ...result.overrides['client.Menu'], ru: 'Menu' };
  assert.throws(() => verifyOverride(commonSource.get('client.Menu'), { values: wrongLocale, rule: 'reviewed-common-identical' }), /Unclassified unchanged text/);

  const canonical = result.catalogs.content.filter((e) => /^content\.magic\..+\.description$/.test(e.key));
  const legacy = result.catalogs.common.filter((e) => /^client\..+SkillDescription$/.test(e.key));
  assert.equal(canonical.length, 102);
  for (const entry of canonical) {
    const copy = legacy.filter((candidate) => candidate.en === entry.en);
    assert.equal(copy.length, 1, `unique legacy copy: ${entry.key}`);
    assert.deepEqual(result.overrides[copy[0].key], result.overrides[entry.key]);
    assert.deepEqual(slots(result.overrides[entry.key].ar), slots(entry.en));
  }
  assert.equal(result.overrides['client.FastMoveSkillDescription'], undefined);
  for (const id of absentTooltipSkills) assert.equal(result.overrides[`content.magic.${id}.description`], undefined);
  const kinds = { Fencing: 'passive skill', Thrusting: 'toggle skill', BackStep: 'active skill', Concentration: 'buff skill', Blink: 'instant casting', Blizzard: 'channelling casting', ExplosiveTrap: 'trap skill', SummonToad: 'summoning skill', MagicBooster: 'lasting effect' };
  for (const [skill, mode] of Object.entries(kinds)) {
    for (const [i, locale] of locales.entries()) assert.ok(result.overrides[`content.magic.${skill}.description`][locale].includes(tooltipModes[mode][i]), `${skill}/${locale}: preserve casting mode`);
  }
  const perHit = ['за атаку', 'प्रति आक्रमण', 'per serangan', 'mỗi đòn đánh', 'ต่อการโจมตี', 'لكل هجوم'];
  for (const [i, locale] of locales.entries()) {
    assert.ok(!result.overrides['content.magic.Thrusting.description'][locale].includes('{2}'), 'do not invent Thrusting MP');
    assert.ok(!result.overrides['content.magic.CrescentSlash.description'][locale].includes(tooltipModes['instant casting'][i]), 'do not invent a missing casting label');
    for (const skill of ['HalfMoon', 'CrossHalfMoon', 'DoubleSlash']) assert.ok(result.overrides[`content.magic.${skill}.description`][locale].includes(`{2} ${perHit[i]}`), `MP per attack: ${skill}/${locale}`);
    assert.deepEqual(numericLiterals(result.overrides['content.magic.Trap.description'][locale]), ['60']);
    assert.ok(result.overrides['content.magic.Curse.description'][locale].includes(`${term('Amulet')[locale]} + ${term('Poison')[locale]}`));
    assert.ok(result.overrides['content.magic.Plague.description'][locale].includes(`${term('Amulet')[locale]} + ${term('Poison')[locale]}`));
    assert.ok(result.overrides['content.magic.PoisonCloud.description'][locale].includes(term('Green Poison')[locale]));
    assert.ok(result.overrides['content.magic.CounterAttack.description'][locale].includes('AMC'), 'flag source AMC; do not silently rewrite its stat');
  }
  // High-risk effects must retain the conditional/cost clauses, not merely the
  // skill name and damage verb. These fragments guard the actual authored copy.
  const clauses = {
    CrippleShot: [['замедляющую', 'дважды'], ['धीमा', 'दो बार'], ['memperlambat', 'dua kali'], ['làm chậm', 'hai lần'], ['ช้าลง', 'สองครั้ง'], ['يبطئ', 'مرتين']],
    ElementalShot: [['Если элементов нет', 'уровень лучника выше'], ['कोई तत्त्व न होने पर', 'स्तर लक्ष्य से ऊँचा'], ['Jika tidak ada elemen', 'level pemanah lebih tinggi'], ['Nếu không có nguyên tố', 'cấp cung thủ cao hơn'], ['หากไม่มีธาตุ', 'นักธนูมีระดับสูงกว่า'], ['إذا لم توجد عناصر', 'مستوى الرامي أعلى']],
    SummonToad: [['не может двигаться', 'взорвётся, если хозяин'], ['चल नहीं सकता', 'बाहर जाने पर फट'], ['tidak dapat bergerak', 'meledak jika tuannya'], ['không thể di chuyển', 'phát nổ nếu chủ nhân'], ['เคลื่อนที่ไม่ได้', 'ระเบิดหากเจ้าของ'], ['لا يستطيع العلجوم الحركة', 'ينفجر إذا غادر صاحبه']],
    TrapHexagon: [['Любой урон от внешнего', 'двигаться'], ['किसी बाहरी स्रोत', 'फिर चल सकता'], ['Kerusakan apa pun dari sumber luar', 'bergerak lagi'], ['Bất kỳ sát thương', 'di chuyển trở lại'], ['ความเสียหายใด ๆ จากภายนอก', 'กลับมาเคลื่อนที่ได้'], ['أي ضرر من مصدر خارجي', 'بالحركة مجددًا']],
    TwinDrakeBlade: [['небольшой шанс', 'дополнительно 50%'], ['कम संभावना', '50% अतिरिक्त'], ['peluang kecil', 'tambahan sebesar 50%'], ['cơ hội nhỏ', 'thêm 50%'], ['โอกาสเล็กน้อย', 'เพิ่ม 50%'], ['فرصة منخفضة', 'إضافيًا بنسبة 50%']],
    MassHiding: [['членов вашей группы', 'начнёте двигаться'], ['आपके दल', 'चलना शुरू'], ['anggota kelompok', 'mulai bergerak'], ['thành viên tổ đội', 'bắt đầu di chuyển'], ['สมาชิกปาร์ตี้', 'เริ่มเคลื่อนไหว'], ['أعضاء فريقك', 'إذا بدأت بالحركة']],
    Plague: [['Снижает MP', 'оглушение', 'проклятие', 'отравление', 'замедление'], ['MP घटाता', 'स्तब्धता', 'अभिशाप', 'विष', 'मंदता'], ['Mengurangi MP', 'pingsan', 'kutukan', 'racun', 'perlambatan'], ['Giảm MP', 'choáng', 'nguyền rủa', 'trúng độc', 'làm chậm'], ['ลด MP', 'มึนงง', 'คำสาป', 'พิษ', 'เชื่องช้า'], ['يقلل MP', 'الصعق', 'اللعنة', 'التسمم', 'الإبطاء']]
  };
  for (const [skill, translations] of Object.entries(clauses)) {
    for (const [i, locale] of locales.entries()) {
      for (const clause of translations[i]) assert.ok(result.overrides[`content.magic.${skill}.description`][locale].includes(clause), `Missing reviewed gameplay clause: ${skill}/${locale}/${clause}`);
    }
  }
  console.log('CONTENT_TERMINOLOGY_SELFTEST=passed (102 full skill descriptions and exact legacy copies; 296 common keys; six locales; modes/MP/items/cooldowns/trigger clauses; source pins; spell distinction; proper names; numeric/stat/slot/layout/command parity; exact same-form classification; unchanged-prose rejection)');
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const result = buildContentOverrides();
  if (process.argv.includes('--self-test')) selfTest(result);
  const artifacts = {
    'content-overrides.json': JSON.stringify(result.overrides, null, 2) + '\n',
    'content-review.md': reviewText(result)
  };
  if (process.argv.includes('--check')) {
    for (const [file, text] of Object.entries(artifacts)) {
      if (fs.readFileSync(path.join(outputRoot, file), 'utf8') !== text) throw new Error(`Stale expansion content artifact: ${file}`);
    }
  } else {
    fs.mkdirSync(outputRoot, { recursive: true });
    for (const [file, text] of Object.entries(artifacts)) fs.writeFileSync(path.join(outputRoot, file), text, 'utf8');
  }
  console.log(JSON.stringify({ terms: terms.size, keys: Object.keys(result.overrides).length, locales, values: Object.keys(result.overrides).length * locales.length, source: 'curated-content-overrides', check: process.argv.includes('--check') }));
}
