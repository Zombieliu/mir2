import {LOCALES} from "./locales.js";

// Every new player-facing string has the same explicit nine-language contract.
// This module has no network or storage authority; values come from core.js.
const groups = {
  nav: ["portalTitle","activitiesNav","playerNav","mainNav","refreshAccount","selectedCharacter","portalFooter"],
  hero: ["activityEyebrow","activityHeadline","activityDescription","playerEyebrow","playerHeadline","playerDescription"],
  phase: ["activityTitle","activityIntro","phaseClosed","phaseBeta","phasePublic","phaseDisabled","phaseUnknown","closedRules","betaRules","publicRules","testExclusion"],
  progress: ["loginProgress","loadingProgress","activityUnavailable","highestLevel","level","qualifyingLevel","requiredLevel"],
  rewards: ["monthlyReward","titleReward","cosmeticReward","monthlyHint","titleHint","cosmeticHint","publicDelivery","registerReward","registeredReward","levelLocked","phaseLocked","pastLocked","claimHistory","noClaims","registeredAt","claimDone","claimUnknown"],
  player: ["playerTitle","charactersTitle","noCharacters","savedState"],
  carry: ["carryTitle","carryIntro","realPayments","verifiedCredits","monthlyDays","excludedTests","reviewOrders","reconciliation","recorded","noCarryOrders","orderAmount","orderCredits","orderDate","orderState"],
  classes: ["classWarrior","classWizard","classTaoist","classAssassin","classArcher"],
};
const translations = {
  en: {
    nav: ["Player services · Numeron","Activity center","Player center","Player services","Refresh my account","Selected character","Your activities, characters and recharge records share the same game account."],
    hero: ["YOUR JOURNEY, REMEMBERED","Build a legacy.\nCarry it forward.","Reach milestones in the test server and register your rewards for public launch. One game account, one lasting record.","YOUR ADVENTURE, AT A GLANCE","Your character.\nYour next chapter.","Check your characters, game time and registered rewards with the same account you use in the game."],
    phase: ["Level milestones","Eligibility is checked by the game server. Rewards are registered once per account and test phase.","Closed test","Beta test","Public launch","Registration unavailable","Campaign rules","Level 30: a bound 30-day card. Level 40: a permanent title. Level 50: a class outfit. Register now; delivery is at public launch.","Real recharge is recorded at any level. Level 40 earns a beta title; level 50 earns a class outfit. Paid Credits and game time are reconciled before launch.","Registered test rewards are kept for public launch. Recruitment and recharge bonus events are being prepared; no referral payout is active yet.","Test payments never count as real recharge or beta carryover."],
    progress: ["Sign in to see your real progress and register eligible rewards.","Checking your server progress…","Activity progress is temporarily unavailable. Refresh your account to try again.","Highest recorded level","Level","Qualifying level","Required level"],
    rewards: ["30 days of game time","Permanent test title","Class outfit","A bound 30-day monthly card, delivered at public launch. The clock starts when you use it.","A permanent commemorative title. It adds no combat stats.","A class-specific outfit for appearance only. Your equipment and combat stats stay intact.","Public launch delivery · registration does not send mail or equip a reward today.","Register reward","Registered for public launch","Level requirement not met","Future test phase","Registration for this phase is closed","Reward registrations","No rewards registered yet.","Registered","Your reward is registered for public launch.","Registration is not confirmed yet. Refresh or retry the same reward; it can only be registered once."],
    player: ["Your player center","Your characters","Create a character in the Windows game client to begin.","These are saved server values. A character currently playing may have newer progress or balances until its next normal save."],
    carry: ["Beta recharge ledger","Only verified real payments enter this ledger. Recorded Credits are not a settled return or the current wallet balance. Final Credits and purchased game time require reconciliation before public launch.","Verified real payments (USD)","Verified recharge Credits","Reconciled game time (days)","Excluded test orders","Orders under review","Reconciliation required","Recorded","No verified real recharge records yet.","Amount (USD)","Credits","Recorded on","Status"],
    classes: ["Warrior","Wizard","Taoist","Assassin","Archer"],
  },
  "zh-Hant": {
    nav: ["玩家服務 · Numeron","活動中心","玩家中心","玩家服務","重新整理帳號","所選角色","活動、角色與儲值紀錄，皆沿用同一個遊戲帳號。"],
    hero: ["每一步冒險，都留下紀錄","留下你的傳奇，\n帶往下一段旅程。","在測試服達成等級里程碑，登記公測獎勵。沿用遊戲帳號，保留你的成就紀錄。","一眼看見你的冒險","你的角色，\n你的下一段傳奇。","使用遊戲內的同一個帳號，查看角色、遊戲時長與已登記獎勵。"],
    phase: ["等級里程碑","資格由遊戲伺服器核對；每個帳號在各測試階段只能登記一次。","封測","內測","公測","暫不開放登記","活動規則","30 級：綁定 30 天月卡；40 級：永久稱號；50 級：職業時裝。現在登記，公測時發放。","真實儲值不限等級記帳。40 級獲得內測稱號，50 級獲得職業時裝；已付費點券與時長於公測前核對返還。","測試獎勵資格保留至公測。招募與儲值加贈活動仍在準備，目前尚未啟用隊長返還。","測試付款不計入真實儲值，也不會產生內測返還。"],
    progress: ["登入後查看真實進度，並登記符合資格的獎勵。","正在核對伺服器進度…","活動進度暫時無法取得，請重新整理帳號後再試。","最高已記錄等級","等級","資格等級","所需等級"],
    rewards: ["30 天遊戲時長","永久測試稱號","職業時裝","公測時發放綁定 30 天月卡，使用後才開始計時。","永久紀念稱號，不增加戰鬥屬性。","僅改變外觀的職業時裝，保留裝備與戰鬥屬性。","公測時發放・目前登記不會立即寄送郵件或穿戴獎勵。","登記獎勵","已登記公測獎勵","尚未達到等級要求","後續測試階段","此階段登記已結束","獎勵登記紀錄","尚未登記任何獎勵。","登記時間","已成功登記，獎勵將於公測時發放。","登記結果尚未確認。請重新整理或重試同一獎勵；每項獎勵只會登記一次。"],
    player: ["你的玩家中心","你的角色","請先在 Windows 遊戲客戶端建立角色。","這裡顯示伺服器已儲存資料。正在遊戲中的角色，進度與餘額可能要等下次正常儲存後才更新。"],
    carry: ["內測儲值返還帳本","僅記錄已核驗的真實付款。登記點券不代表已返還金額或目前錢包餘額；最終返還點券與已購買時長，須於公測前對帳確認。","已核驗真實付款（美元）","已核驗儲值點券","已結算遊戲時長（天）","已排除測試訂單","待審核訂單","需要核對結算","已記錄","目前沒有已核驗的真實儲值紀錄。","金額（美元）","點券","記錄日期","狀態"],
    classes: ["戰士","法師","道士","刺客","弓箭手"],
  },
  "pt-BR": {
    nav: ["Serviços do jogador · Numeron","Eventos","Central do jogador","Serviços do jogador","Atualizar minha conta","Personagem selecionado","Eventos, personagens e recargas usam a mesma conta do jogo."],
    hero: ["SUA JORNADA, REGISTRADA","Construa seu legado.\nLeve-o adiante.","Alcance os marcos no servidor de testes e registre recompensas para o lançamento público. Uma conta, um histórico permanente.","SUA AVENTURA EM UM OLHAR","Seu personagem.\nSeu próximo capítulo.","Consulte personagens, tempo de jogo e recompensas registradas com a mesma conta usada no jogo."],
    phase: ["Marcos de nível","O servidor verifica a elegibilidade. Cada recompensa pode ser registrada uma vez por conta e fase de testes.","Teste fechado","Teste beta","Lançamento público","Registro indisponível","Regras dos eventos","Nível 30: cartão vinculado de 30 dias. Nível 40: título permanente. Nível 50: traje de classe. Registre agora; entrega no lançamento público.","Recargas reais são registradas em qualquer nível. Nível 40: título beta; nível 50: traje de classe. Créditos pagos e tempo de jogo serão conciliados antes do lançamento.","As recompensas de teste ficam reservadas para o lançamento. Eventos de recrutamento e bônus de recarga estão em preparação; nenhum retorno de indicação está ativo.","Pagamentos de teste nunca contam como recarga real nem como devolução do beta."],
    progress: ["Entre para consultar o progresso real e registrar as recompensas elegíveis.","Consultando seu progresso no servidor…","O progresso está indisponível. Atualize sua conta para tentar novamente.","Maior nível registrado","Nível","Nível elegível","Nível exigido"],
    rewards: ["30 dias de jogo","Título permanente de teste","Traje de classe","Cartão vinculado de 30 dias, entregue no lançamento. O prazo começa quando você o usa.","Título comemorativo permanente, sem atributos de combate.","Traje da sua classe apenas para aparência. Seu equipamento e atributos de combate não mudam.","Entrega no lançamento público · o registro não envia correio nem equipa recompensas hoje.","Registrar recompensa","Registrada para o lançamento","Nível exigido ainda não atingido","Próxima fase de testes","Registro desta fase encerrado","Recompensas registradas","Nenhuma recompensa registrada ainda.","Registrada em","Sua recompensa está registrada para o lançamento público.","O registro ainda não foi confirmado. Atualize ou tente a mesma recompensa; ela só pode ser registrada uma vez."],
    player: ["Sua central do jogador","Seus personagens","Crie um personagem no cliente Windows para começar.","Os dados exibidos foram salvos pelo servidor. Um personagem em jogo pode ter progresso ou saldo mais recente até o próximo salvamento normal."],
    carry: ["Registro de recargas do beta","Este registro inclui somente pagamentos reais verificados. Os Créditos registrados não são uma devolução concluída nem o saldo atual. Os Créditos finais e o tempo comprado exigem conciliação antes do lançamento.","Pagamentos reais verificados (USD)","Créditos de recarga verificados","Tempo conciliado (dias)","Pedidos de teste excluídos","Pedidos em análise","Conciliação necessária","Registrado","Ainda não há registros de recargas reais verificadas.","Valor (USD)","Créditos","Data do registro","Status"],
    classes: ["Guerreiro","Mago","Taoísta","Assassino","Arqueiro"],
  },
  ru: {
    nav: ["Сервисы игрока · Numeron","События","Кабинет игрока","Сервисы игрока","Обновить аккаунт","Выбранный персонаж","События, персонажи и пополнения используют один игровой аккаунт."],
    hero: ["ВАШ ПУТЬ СОХРАНЁН","Создайте свою легенду.\nПродолжите её.","Достигайте уровней на тестовом сервере и регистрируйте награды к публичному запуску. Один аккаунт, общая история.","ВАШЕ ПРИКЛЮЧЕНИЕ В ДЕТАЛЯХ","Ваш персонаж.\nНовая глава.","Проверяйте персонажей, игровое время и зарегистрированные награды через свой игровой аккаунт."],
    phase: ["Награды за уровни","Право на награду проверяет сервер. Регистрация доступна один раз на аккаунт за каждый этап тестирования.","Закрытый тест","Бета-тест","Публичный запуск","Регистрация недоступна","Правила событий","Уровень 30: привязанная карта на 30 дней. Уровень 40: постоянный титул. Уровень 50: костюм класса. Зарегистрируйте сейчас, выдача при запуске.","Реальные пополнения учитываются при любом уровне. Уровень 40: титул бета-теста; уровень 50: костюм. Оплаченные кредиты и время будут сверены до запуска.","Тестовые награды сохраняются к публичному запуску. События приглашения и бонусы готовятся; выплаты за приглашения ещё не действуют.","Тестовые платежи не считаются реальным пополнением и не дают возврата за бета-тест."],
    progress: ["Войдите, чтобы увидеть реальный прогресс и зарегистрировать доступные награды.","Проверяем прогресс на сервере…","Прогресс временно недоступен. Обновите аккаунт и попробуйте снова.","Наивысший сохранённый уровень","Уровень","Уровень для награды","Требуемый уровень"],
    rewards: ["30 дней игрового времени","Постоянный тестовый титул","Костюм класса","Привязанная карта на 30 дней выдаётся при запуске. Срок начинается после использования.","Постоянный памятный титул без боевых характеристик.","Костюм вашего класса меняет только внешность, сохраняя экипировку и боевые характеристики.","Выдача при публичном запуске · сейчас регистрация не отправляет письмо и не экипирует награду.","Зарегистрировать награду","Зарегистрировано к запуску","Нужный уровень не достигнут","Следующий этап тестирования","Регистрация этого этапа закрыта","Регистрации наград","Пока нет зарегистрированных наград.","Дата регистрации","Награда зарегистрирована к публичному запуску.","Регистрация пока не подтверждена. Обновите данные или повторите ту же награду; она регистрируется только один раз."],
    player: ["Ваш кабинет игрока","Ваши персонажи","Создайте персонажа в клиенте Windows.","Это сохранённые сервером значения. Прогресс и баланс персонажа в игре обновятся после следующего обычного сохранения."],
    carry: ["Учёт пополнений бета-теста","В реестр входят только проверенные реальные платежи. Записанные кредиты не являются уже возвращёнными средствами или текущим балансом. Итоговые кредиты и купленное время требуют сверки до запуска.","Проверенные реальные платежи (USD)","Проверенные кредиты пополнения","Сверенное время (дни)","Исключённые тестовые заказы","Заказы на проверке","Требуется сверка","Учтено","Проверенных реальных пополнений пока нет.","Сумма (USD)","Кредиты","Дата учёта","Статус"],
    classes: ["Воин","Маг","Даос","Убийца","Лучник"],
  },
  hi: {
    nav: ["खिलाड़ी सेवाएँ · Numeron","इवेंट केंद्र","खिलाड़ी केंद्र","खिलाड़ी सेवाएँ","मेरा खाता अपडेट करें","चुना गया पात्र","इवेंट, पात्र और रिचार्ज रिकॉर्ड एक ही गेम खाते से जुड़े हैं।"],
    hero: ["आपके सफ़र का रिकॉर्ड","अपनी विरासत बनाएँ।\nउसे आगे ले जाएँ।","टेस्ट सर्वर पर स्तर के लक्ष्य पूरे करें और सार्वजनिक लॉन्च के लिए पुरस्कार दर्ज करें। एक गेम खाता, एक स्थायी रिकॉर्ड।","आपका रोमांच एक नज़र में","आपका पात्र।\nआपका अगला अध्याय।","गेम वाले खाते से अपने पात्र, खेलने का समय और दर्ज पुरस्कार देखें।"],
    phase: ["स्तर के लक्ष्य","पात्रता गेम सर्वर जाँचता है। हर पुरस्कार एक खाते में हर परीक्षण चरण के लिए एक बार दर्ज होता है।","बंद परीक्षण","बीटा परीक्षण","सार्वजनिक लॉन्च","पंजीकरण उपलब्ध नहीं","इवेंट नियम","स्तर 30: खाते से जुड़ा 30 दिन का कार्ड। स्तर 40: स्थायी उपाधि। स्तर 50: वर्ग की पोशाक। अभी दर्ज करें, लॉन्च पर मिलेगा।","असली रिचार्ज किसी भी स्तर पर दर्ज होता है। स्तर 40 पर बीटा उपाधि, स्तर 50 पर वर्ग की पोशाक। खरीदे क्रेडिट और समय लॉन्च से पहले मिलाए जाएँगे।","टेस्ट पुरस्कार लॉन्च के लिए सुरक्षित रहेंगे। भर्ती और रिचार्ज बोनस तैयार हो रहे हैं; रेफ़रल भुगतान अभी सक्रिय नहीं है।","टेस्ट भुगतान असली रिचार्ज या बीटा वापसी में कभी नहीं गिने जाते।"],
    progress: ["अपनी वास्तविक प्रगति देखने और पात्र पुरस्कार दर्ज करने के लिए लॉग इन करें।","सर्वर पर आपकी प्रगति जाँची जा रही है…","इवेंट प्रगति अभी उपलब्ध नहीं है। खाता अपडेट करके फिर कोशिश करें।","सबसे ऊँचा दर्ज स्तर","स्तर","पात्रता स्तर","ज़रूरी स्तर"],
    rewards: ["30 दिन का खेलने का समय","स्थायी टेस्ट उपाधि","वर्ग की पोशाक","जुड़ा हुआ 30 दिन का कार्ड लॉन्च पर मिलेगा। इस्तेमाल करने पर समय शुरू होगा।","स्थायी स्मृति उपाधि, बिना लड़ाई के गुणों में बदलाव।","वर्ग की पोशाक केवल रूप बदलती है। उपकरण और लड़ाई के गुण वही रहते हैं।","सार्वजनिक लॉन्च पर वितरण · अभी पंजीकरण से डाक नहीं भेजी जाती और पुरस्कार पहना नहीं जाता।","पुरस्कार दर्ज करें","लॉन्च के लिए दर्ज","ज़रूरी स्तर अभी पूरा नहीं हुआ","अगला परीक्षण चरण","इस चरण का पंजीकरण बंद है","दर्ज पुरस्कार","अभी कोई पुरस्कार दर्ज नहीं है।","दर्ज करने की तारीख","आपका पुरस्कार सार्वजनिक लॉन्च के लिए दर्ज हो गया है।","पंजीकरण अभी पक्का नहीं है। अपडेट करें या उसी पुरस्कार को फिर आज़माएँ; वह केवल एक बार दर्ज हो सकता है।"],
    player: ["आपका खिलाड़ी केंद्र","आपके पात्र","शुरू करने के लिए Windows गेम क्लाइंट में पात्र बनाएँ।","ये सर्वर में सहेजे हुए आँकड़े हैं। अभी खेल रहे पात्र की नई प्रगति या बैलेंस अगले सामान्य सेव के बाद दिखेगा।"],
    carry: ["बीटा रिचार्ज बही","केवल सत्यापित असली भुगतान इस खाते में दर्ज होते हैं। दर्ज क्रेडिट अंतिम वापसी या मौजूदा वॉलेट शेष नहीं हैं। अंतिम क्रेडिट और खरीदे समय का मिलान सार्वजनिक लॉन्च से पहले आवश्यक है।","सत्यापित असली भुगतान (USD)","सत्यापित रिचार्ज क्रेडिट","मिलाया हुआ खेलने का समय (दिन)","हटाए गए टेस्ट ऑर्डर","जाँच के अधीन ऑर्डर","मिलान ज़रूरी है","दर्ज","अभी कोई सत्यापित असली रिचार्ज रिकॉर्ड नहीं है।","राशि (USD)","क्रेडिट","दर्ज तारीख","स्थिति"],
    classes: ["योद्धा","जादूगर","ताओवादी","हत्यारा","धनुर्धर"],
  },
  id: {
    nav: ["Layanan pemain · Numeron","Pusat acara","Pusat pemain","Layanan pemain","Perbarui akun saya","Karakter terpilih","Acara, karakter, dan catatan isi ulang memakai akun game yang sama."],
    hero: ["PERJALANAN ANDA TERCATAT","Bangun warisan.\nBawa ke depan.","Capai target level di server uji dan daftarkan hadiah untuk peluncuran publik. Satu akun game, satu catatan tetap.","PETUALANGAN ANDA SEKILAS","Karakter Anda.\nBab berikutnya.","Periksa karakter, waktu bermain, dan hadiah terdaftar dengan akun yang sama seperti di game."],
    phase: ["Target level","Kelayakan diperiksa server game. Hadiah didaftarkan sekali per akun dan tahap pengujian.","Uji tertutup","Uji beta","Peluncuran publik","Pendaftaran tidak tersedia","Aturan acara","Level 30: kartu terikat 30 hari. Level 40: gelar permanen. Level 50: kostum kelas. Daftar sekarang; pengiriman saat peluncuran publik.","Isi ulang nyata dicatat pada semua level. Level 40 mendapat gelar beta, level 50 mendapat kostum kelas. Kredit berbayar dan waktu bermain direkonsiliasi sebelum peluncuran.","Hadiah uji disimpan untuk peluncuran publik. Acara rekrutmen dan bonus isi ulang sedang disiapkan; imbalan rujukan belum aktif.","Pembayaran uji tidak pernah dihitung sebagai isi ulang nyata atau pengembalian beta."],
    progress: ["Masuk untuk melihat kemajuan nyata dan mendaftarkan hadiah yang memenuhi syarat.","Memeriksa kemajuan Anda di server…","Kemajuan acara sementara tidak tersedia. Perbarui akun untuk mencoba lagi.","Level tertinggi tercatat","Level","Level yang memenuhi syarat","Level yang dibutuhkan"],
    rewards: ["30 hari waktu bermain","Gelar uji permanen","Kostum kelas","Kartu terikat 30 hari dikirim saat peluncuran publik. Waktu dimulai saat digunakan.","Gelar kenang-kenangan permanen tanpa atribut pertempuran.","Kostum kelas hanya mengubah tampilan. Peralatan dan atribut pertempuran tetap sama.","Pengiriman saat peluncuran publik · pendaftaran tidak mengirim surat atau memakai hadiah sekarang.","Daftarkan hadiah","Terdaftar untuk peluncuran","Level belum memenuhi syarat","Tahap uji mendatang","Pendaftaran tahap ini ditutup","Pendaftaran hadiah","Belum ada hadiah terdaftar.","Terdaftar pada","Hadiah Anda terdaftar untuk peluncuran publik.","Pendaftaran belum dikonfirmasi. Perbarui atau coba hadiah yang sama; hadiah hanya dapat didaftarkan sekali."],
    player: ["Pusat pemain Anda","Karakter Anda","Buat karakter di klien game Windows untuk memulai.","Ini adalah data yang disimpan server. Karakter yang sedang bermain mungkin memiliki kemajuan atau saldo baru hingga penyimpanan normal berikutnya."],
    carry: ["Buku isi ulang beta","Catatan ini hanya mencakup pembayaran nyata terverifikasi. Kredit tercatat bukan pengembalian yang sudah diselesaikan atau saldo dompet saat ini. Kredit akhir dan waktu yang dibeli perlu direkonsiliasi sebelum peluncuran.","Pembayaran nyata terverifikasi (USD)","Kredit isi ulang terverifikasi","Waktu bermain direkonsiliasi (hari)","Pesanan uji dikecualikan","Pesanan ditinjau","Perlu rekonsiliasi","Tercatat","Belum ada catatan isi ulang nyata terverifikasi.","Jumlah (USD)","Kredit","Tanggal pencatatan","Status"],
    classes: ["Prajurit","Penyihir","Taois","Assassin","Pemanah"],
  },
  vi: {
    nav: ["Dịch vụ người chơi · Numeron","Sự kiện","Trung tâm người chơi","Dịch vụ người chơi","Làm mới tài khoản","Nhân vật đã chọn","Sự kiện, nhân vật và lịch sử nạp dùng chung tài khoản game."],
    hero: ["HÀNH TRÌNH ĐƯỢC GHI NHỚ","Tạo nên huyền thoại.\nTiếp bước hành trình.","Đạt mốc cấp trên máy chủ thử nghiệm và đăng ký quà khi ra mắt công khai. Một tài khoản, một hồ sơ lâu dài.","HÀNH TRÌNH TRONG TẦM MẮT","Nhân vật của bạn.\nChương tiếp theo.","Xem nhân vật, thời gian chơi và quà đã đăng ký bằng tài khoản đang dùng trong game."],
    phase: ["Các mốc cấp độ","Máy chủ xác minh điều kiện. Mỗi quà được đăng ký một lần cho mỗi tài khoản và giai đoạn thử nghiệm.","Thử nghiệm kín","Thử nghiệm beta","Ra mắt công khai","Chưa mở đăng ký","Quy tắc sự kiện","Cấp 30: thẻ khóa 30 ngày. Cấp 40: danh hiệu vĩnh viễn. Cấp 50: trang phục nghề. Đăng ký ngay, phát khi ra mắt công khai.","Nạp tiền thật được ghi nhận ở mọi cấp. Cấp 40 nhận danh hiệu beta, cấp 50 nhận trang phục nghề. Điểm và thời gian đã mua được đối soát trước ra mắt.","Quà thử nghiệm được giữ cho ra mắt công khai. Sự kiện tuyển người và thưởng nạp đang chuẩn bị; hoàn thưởng giới thiệu chưa hoạt động.","Thanh toán thử nghiệm không tính là nạp tiền thật hay hoàn trả beta."],
    progress: ["Đăng nhập để xem tiến độ thật và đăng ký quà đủ điều kiện.","Đang kiểm tra tiến độ trên máy chủ…","Chưa lấy được tiến độ sự kiện. Làm mới tài khoản rồi thử lại.","Cấp cao nhất đã ghi nhận","Cấp","Cấp đủ điều kiện","Cấp yêu cầu"],
    rewards: ["30 ngày chơi","Danh hiệu thử nghiệm vĩnh viễn","Trang phục nghề","Thẻ khóa 30 ngày phát khi ra mắt công khai. Thời hạn bắt đầu lúc sử dụng.","Danh hiệu kỷ niệm vĩnh viễn, không thêm chỉ số chiến đấu.","Trang phục nghề chỉ đổi ngoại hình, giữ nguyên trang bị và chỉ số chiến đấu.","Phát khi ra mắt công khai · đăng ký hiện tại không gửi thư hay mặc quà ngay.","Đăng ký quà","Đã đăng ký cho ra mắt","Chưa đạt cấp yêu cầu","Giai đoạn thử nghiệm sắp tới","Đã đóng đăng ký giai đoạn này","Quà đã đăng ký","Chưa đăng ký quà nào.","Đăng ký lúc","Quà đã được đăng ký cho ra mắt công khai.","Chưa xác nhận đăng ký. Làm mới hoặc thử lại cùng một quà; mỗi quà chỉ được đăng ký một lần."],
    player: ["Trung tâm người chơi của bạn","Nhân vật của bạn","Tạo nhân vật trên ứng dụng game Windows để bắt đầu.","Đây là dữ liệu đã lưu trên máy chủ. Nhân vật đang chơi có thể có tiến độ hoặc số dư mới hơn cho đến lần lưu bình thường tiếp theo."],
    carry: ["Sổ nạp tiền beta","Sổ này chỉ ghi thanh toán thật đã xác minh. Điểm đã ghi không phải số đã hoàn trả hay số dư ví hiện tại. Điểm cuối cùng và thời gian đã mua cần được đối soát trước ra mắt.","Thanh toán thật đã xác minh (USD)","Điểm nạp đã xác minh","Thời gian đã đối soát (ngày)","Đơn thử nghiệm bị loại","Đơn đang kiểm tra","Cần đối soát","Đã ghi nhận","Chưa có bản ghi nạp tiền thật đã xác minh.","Số tiền (USD)","Điểm","Ngày ghi nhận","Trạng thái"],
    classes: ["Chiến sĩ","Pháp sư","Đạo sĩ","Thích khách","Cung thủ"],
  },
  th: {
    nav: ["บริการผู้เล่น · Numeron","กิจกรรม","ศูนย์ผู้เล่น","บริการผู้เล่น","รีเฟรชบัญชีของฉัน","ตัวละครที่เลือก","กิจกรรม ตัวละคร และประวัติเติมเงินใช้บัญชีเกมเดียวกัน"],
    hero: ["บันทึกทุกการผจญภัย","สร้างตำนานของคุณ\nและก้าวไปข้างหน้า","บรรลุระดับในเซิร์ฟเวอร์ทดสอบและลงทะเบียนรางวัลสำหรับเปิดบริการ บัญชีเดียว บันทึกความสำเร็จเดียว","การผจญภัยในภาพรวม","ตัวละครของคุณ\nบทต่อไปของคุณ","ตรวจสอบตัวละคร เวลาเล่น และรางวัลที่ลงทะเบียนด้วยบัญชีเดียวกับในเกม"],
    phase: ["เป้าหมายระดับ","เซิร์ฟเวอร์เกมตรวจสอบสิทธิ์ แต่ละรางวัลลงทะเบียนได้ครั้งเดียวต่อบัญชีและช่วงทดสอบ","ทดสอบปิด","ทดสอบเบตา","เปิดบริการ","ยังไม่เปิดลงทะเบียน","กติกากิจกรรม","ระดับ 30: บัตรผูกบัญชี 30 วัน ระดับ 40: ฉายาถาวร ระดับ 50: ชุดอาชีพ ลงทะเบียนตอนนี้ รับเมื่อเปิดบริการ","เติมเงินจริงบันทึกได้ทุกระดับ ระดับ 40 ได้ฉายาเบตา ระดับ 50 ได้ชุดอาชีพ เครดิตและเวลาเล่นที่ซื้อจะตรวจสอบก่อนเปิดบริการ","สิทธิ์รางวัลทดสอบเก็บไว้ถึงวันเปิดบริการ กิจกรรมชวนเพื่อนและโบนัสเติมเงินอยู่ระหว่างเตรียม ยังไม่มีผลตอบแทนผู้แนะนำ","การชำระทดสอบไม่นับเป็นเติมเงินจริงหรือคืนผลประโยชน์เบตา"],
    progress: ["เข้าสู่ระบบเพื่อดูความคืบหน้าจริงและลงทะเบียนรางวัลที่มีสิทธิ์","กำลังตรวจสอบความคืบหน้าจากเซิร์ฟเวอร์…","ยังโหลดความคืบหน้ากิจกรรมไม่ได้ รีเฟรชบัญชีแล้วลองอีกครั้ง","ระดับสูงสุดที่บันทึก","ระดับ","ระดับที่ใช้ตรวจสิทธิ์","ระดับที่ต้องการ"],
    rewards: ["เวลาเล่น 30 วัน","ฉายาทดสอบถาวร","ชุดอาชีพ","บัตรผูกบัญชี 30 วันส่งให้เมื่อเปิดบริการ เริ่มนับเวลาเมื่อใช้","ฉายาที่ระลึกถาวร ไม่เพิ่มค่าสถานะต่อสู้","ชุดอาชีพเปลี่ยนเฉพาะรูปลักษณ์ อุปกรณ์และค่าสถานะต่อสู้ยังคงเดิม","ส่งมอบเมื่อเปิดบริการ · การลงทะเบียนวันนี้ไม่ส่งจดหมายหรือสวมรางวัลทันที","ลงทะเบียนรางวัล","ลงทะเบียนสำหรับเปิดบริการแล้ว","ยังไม่ถึงระดับที่กำหนด","ช่วงทดสอบถัดไป","ปิดลงทะเบียนช่วงนี้แล้ว","รางวัลที่ลงทะเบียน","ยังไม่มีรางวัลที่ลงทะเบียน","เวลาลงทะเบียน","ลงทะเบียนรางวัลสำหรับเปิดบริการแล้ว","ยังยืนยันการลงทะเบียนไม่ได้ รีเฟรชหรือลองรางวัลเดิมอีกครั้ง รางวัลแต่ละอย่างลงทะเบียนได้ครั้งเดียว"],
    player: ["ศูนย์ผู้เล่นของคุณ","ตัวละครของคุณ","สร้างตัวละครในเกมเวอร์ชัน Windows เพื่อเริ่มต้น","นี่คือข้อมูลที่เซิร์ฟเวอร์บันทึกไว้ ตัวละครที่กำลังเล่นอาจมีความคืบหน้าหรือยอดใหม่กว่าจนถึงการบันทึกตามปกติครั้งถัดไป"],
    carry: ["บัญชีเติมเงินเบตา","บันทึกเฉพาะการชำระเงินจริงที่ตรวจสอบแล้ว เครดิตที่บันทึกไม่ใช่ยอดคืนที่เสร็จสิ้นหรือยอดกระเป๋าปัจจุบัน เครดิตสุดท้ายและเวลาเล่นที่ซื้อยังต้องตรวจสอบยอดก่อนเปิดบริการ","เงินจริงที่ตรวจสอบแล้ว (USD)","เครดิตเติมเงินที่ตรวจสอบแล้ว","เวลาเล่นที่ตรวจสอบแล้ว (วัน)","คำสั่งซื้อทดสอบที่ไม่รวม","คำสั่งซื้อที่ตรวจสอบ","ต้องตรวจสอบยอด","บันทึกแล้ว","ยังไม่มีรายการเติมเงินจริงที่ตรวจสอบแล้ว","จำนวนเงิน (USD)","เครดิต","วันที่บันทึก","สถานะ"],
    classes: ["นักรบ","นักเวท","นักพรต","นักฆ่า","นักธนู"],
  },
  ar: {
    nav: ["خدمات اللاعب · Numeron","مركز الفعاليات","مركز اللاعب","خدمات اللاعب","تحديث حسابي","الشخصية المختارة","تستخدم الفعاليات والشخصيات وسجلات الشحن حساب اللعبة نفسه."],
    hero: ["رحلتك محفوظة","اصنع أسطورتك.\nوأكمل رحلتك.","حقق مستويات الاختبار وسجّل مكافآتك للإطلاق العام. حساب لعبة واحد وسجل دائم لإنجازاتك.","مغامرتك في لمحة","شخصيتك.\nالفصل التالي.","تحقّق من شخصياتك ووقت اللعب والمكافآت المسجلة باستخدام حسابك في اللعبة."],
    phase: ["مكافآت المستويات","يتحقق خادم اللعبة من الأهلية. تُسجّل كل مكافأة مرة واحدة لكل حساب ومرحلة اختبار.","الاختبار المغلق","اختبار بيتا","الإطلاق العام","التسجيل غير متاح","قواعد الفعاليات","المستوى 30: بطاقة مرتبطة لمدة 30 يومًا. المستوى 40: لقب دائم. المستوى 50: زي للفئة. سجّل الآن والاستلام عند الإطلاق العام.","يُسجّل الشحن الحقيقي بأي مستوى. المستوى 40 يمنح لقب بيتا والمستوى 50 زي الفئة. تُسوّى النقاط المدفوعة ووقت اللعب قبل الإطلاق.","تُحفظ مكافآت الاختبار للإطلاق العام. فعاليات الدعوة ومكافآت الشحن قيد الإعداد؛ عائد الإحالة غير مفعّل حاليًا.","لا تُحتسب المدفوعات التجريبية شحنًا حقيقيًا ولا تدخل في استرجاع بيتا."],
    progress: ["سجّل الدخول لعرض تقدمك الحقيقي وتسجيل المكافآت المؤهلة.","جارٍ التحقق من تقدمك على الخادم…","تقدم الفعاليات غير متاح مؤقتًا. حدّث حسابك للمحاولة مجددًا.","أعلى مستوى مسجل","المستوى","مستوى التأهل","المستوى المطلوب"],
    rewards: ["30 يومًا من وقت اللعب","لقب اختبار دائم","زي الفئة","بطاقة مرتبطة لمدة 30 يومًا تُسلّم عند الإطلاق العام. يبدأ الوقت عند استخدامها.","لقب تذكاري دائم لا يضيف خصائص قتالية.","زي خاص بالفئة يغيّر المظهر فقط. تبقى المعدات والخصائص القتالية كما هي.","التسليم عند الإطلاق العام · التسجيل اليوم لا يرسل بريدًا ولا يجهّز المكافأة فورًا.","تسجيل المكافأة","مسجلة للإطلاق العام","لم يتحقق المستوى المطلوب","مرحلة اختبار قادمة","انتهى التسجيل لهذه المرحلة","المكافآت المسجلة","لا توجد مكافآت مسجلة بعد.","تاريخ التسجيل","تم تسجيل مكافأتك للإطلاق العام.","لم يُؤكّد التسجيل بعد. حدّث الصفحة أو أعد محاولة المكافأة نفسها؛ يمكن تسجيلها مرة واحدة فقط."],
    player: ["مركز اللاعب الخاص بك","شخصياتك","أنشئ شخصية في عميل اللعبة على Windows للبدء.","هذه بيانات محفوظة على الخادم. قد تكون للشخصية المتصلة بيانات تقدم أو رصيد أحدث حتى الحفظ المعتاد التالي."],
    carry: ["سجل شحن بيتا","يشمل السجل المدفوعات الحقيقية المتحقق منها فقط. النقاط المسجلة ليست استرجاعًا نهائيًا أو رصيد المحفظة الحالي. تتطلب النقاط النهائية ووقت اللعب المشترى تسوية قبل الإطلاق العام.","المدفوعات الحقيقية المتحقق منها (USD)","نقاط الشحن المتحقق منها","وقت اللعب المسوّى (أيام)","طلبات الاختبار المستبعدة","طلبات قيد المراجعة","تحتاج إلى تسوية","مسجل","لا توجد سجلات شحن حقيقي متحقق منها بعد.","المبلغ (USD)","النقاط","تاريخ التسجيل","الحالة"],
    classes: ["محارب","ساحر","طاوي","قاتل","رامي سهام"],
  },
};

export const ACTIVITY_LOCALES = Object.freeze(Object.fromEntries(Object.entries(translations).map(([locale, rows]) => {
  const values = {};
  for (const [group, keys] of Object.entries(groups)) {
    if (!Array.isArray(rows[group]) || rows[group].length !== keys.length) throw new Error(`Invalid activity locale ${locale}:${group}`);
    keys.forEach((key, index) => {
      const text = rows[group][index];
      if (typeof text !== "string" || !text.trim()) throw new Error(`Invalid activity text ${locale}:${key}`);
      values[key] = text;
    });
  }
  return [locale, Object.freeze(values)];
})));

export function activityText(locale, key) {
  return (ACTIVITY_LOCALES[locale] ?? ACTIVITY_LOCALES.en)[key] ?? "";
}
export function portalTab(hash) {
  return ["#activities", "#player", "#recharge"].includes(hash) ? hash.slice(1) : "activities";
}
export function sessionEndNotice(code) {
  if (["sessionExpired", "billingAuthenticationRequired", "activitiesAuthenticationRequired"].includes(code)) return "sessionExpired";
  if (["accountBanned", "billingAccountBanned", "activitiesAccountBanned"].includes(code)) return "loginFailed";
  return null;
}
export function currentPortalResult(state, epoch) {
  return state.loggedIn === true && state.epoch === epoch;
}
// Clear private snapshots immediately; the next epoch rejects in-flight results
// even while the old server grant is still being revoked asynchronously.
export function clearedPortalSession(state) {
  return {...state, epoch:state.epoch + 1, loggedIn:false, scope:null,
    status:null, activities:null, activitiesFailed:false, characterIndex:null,
    offerId:null, checkout:null};
}
export function classText(locale, value) {
  const key = {Warrior:"classWarrior", Wizard:"classWizard", Taoist:"classTaoist", Assassin:"classAssassin", Archer:"classArcher"}[value];
  return key ? activityText(locale, key) : "—";
}
const phaseOrder = {closed:0, beta:1, public:2};
export function rewardView(reward, phase, {loggedIn = false, busy = false, characterIndex = null, characters = []} = {}) {
  if (!loggedIn || !reward) return {key:"loginProgress", canRegister:false, progress:null};
  const progress = Math.min(100, Math.max(0, reward.qualifiedLevel / reward.requiredLevel * 100));
  if (reward.claimed) return {key:"registeredReward", canRegister:false, progress};
  if (reward.phase !== phase) {
    return {key:phaseOrder[phase] > phaseOrder[reward.phase] ? "pastLocked" : "phaseLocked", canRegister:false, progress};
  }
  const ownedCharacter = characters.some(character => character.index === characterIndex);
  return {key:reward.eligible ? "registerReward" : "levelLocked",
    canRegister:reward.eligible === true && !busy && ownedCharacter, progress};
}
export function registeredRewards(value) {
  return (value?.rewards ?? []).filter(reward => reward.claimed === true && reward.claimedAtMs !== null)
    .slice().sort((a, b) => b.claimedAtMs - a.claimedAtMs || a.id.localeCompare(b.id));
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
const number = (locale, value) => new Intl.NumberFormat(locale).format(value);
const date = (locale, value) => new Intl.DateTimeFormat(locale, {dateStyle:"medium", timeStyle:"short"}).format(value);
const phaseKey = phase => ({closed:"phaseClosed", beta:"phaseBeta", public:"phasePublic", disabled:"phaseDisabled"}[phase] ?? "phaseUnknown");
const rewardKey = kind => ({monthlyDays:"monthlyReward", title:"titleReward", cosmetic:"cosmeticReward"}[kind]);
const hintKey = kind => ({monthlyDays:"monthlyHint", title:"titleHint", cosmetic:"cosmeticHint"}[kind]);

export function renderClaimHistory(container, value, locale) {
  container.replaceChildren();
  container.append(element("h3", "subheading", activityText(locale, "claimHistory")));
  const claims = registeredRewards(value);
  if (!claims.length) { container.append(element("p", "muted compact", activityText(locale, "noClaims"))); return; }
  const list = element("ul", "claim-list");
  for (const reward of claims) {
    const row = element("li", "claim-row");
    const description = element("div");
    description.append(element("strong", "", activityText(locale, rewardKey(reward.kind))),
      element("small", "", `${activityText(locale, phaseKey(reward.phase))} · ${activityText(locale, "registeredAt")} ${date(locale, reward.claimedAtMs)}`));
    row.append(description, element("span", "registered-tag", activityText(locale, "registeredReward"))); list.append(row);
  }
  container.append(list);
}

export function renderActivities(container, {value, locale, loggedIn, busy, characterIndex, failed = false, onClaim}) {
  container.replaceChildren();
  const text = key => activityText(locale, key);
  const heading = element("div", "section-heading activity-heading");
  heading.append(element("h2", "", text("activityTitle")), element("span", "phase-badge", text(phaseKey(value?.phase))));
  container.append(heading, element("p", "muted activity-intro", text("activityIntro")));
  const exclusion = element("p", "test-exclusion", text("testExclusion"));
  container.append(exclusion);
  if (!loggedIn || !value) {
    container.append(element("p", "activity-placeholder", text(!loggedIn ? "loginProgress" : failed ? "activityUnavailable" : "loadingProgress")));
    if (!loggedIn) {
      // Public rules are information, never synthetic eligibility or progress.
      const previews = element("div", "milestone-grid rules-preview");
      for (const [kind, requiredLevel] of [["monthlyDays",30], ["title",40], ["cosmetic",50]]) {
        const card = element("article", "milestone");
        const requirement = element("div", "milestone-level");
        requirement.append(element("span", "", text("level")), element("strong", "", number(locale, requiredLevel)));
        card.append(requirement, element("h4", "", text(rewardKey(kind))), element("p", "reward-hint", text(hintKey(kind))));
        previews.append(card);
      }
      container.append(previews);
    }
  } else {
    const level = element("div", "level-summary");
    level.append(element("span", "", text("highestLevel")), element("strong", "", number(locale, value.maxLevel)));
    container.append(level);
    if (failed) container.append(element("p", "muted compact", text("activityUnavailable")));
    const phases = ["closed", "beta"];
    for (const phase of phases) {
      const rewards = value.rewards.filter(reward => reward.phase === phase);
      if (!rewards.length) continue;
      const phaseHeading = element("h3", "phase-heading", text(phaseKey(phase)));
      const cards = element("div", "milestone-grid");
      for (const reward of rewards) {
        const view = rewardView(reward, value.phase, {loggedIn, busy:busy || failed, characterIndex, characters:value.characters});
        const card = element("article", `milestone ${reward.claimed ? "registered" : ""}`);
        const requirement = element("div", "milestone-level");
        requirement.append(element("span", "", text("level")), element("strong", "", number(locale, reward.requiredLevel)));
        card.append(requirement, element("h4", "", text(rewardKey(reward.kind))), element("p", "reward-hint", text(hintKey(reward.kind))));
        if (reward.kind === "cosmetic") {
          const selected = value.characters.find(character => character.index === characterIndex);
          if (reward.cosmeticClass || selected) card.append(element("p", "reward-class", classText(locale, reward.cosmeticClass ?? selected.class)));
        }
        const progress = element("div", "reward-progress");
        progress.append(element("span", "", text("qualifyingLevel")), element("b", "", `${number(locale, reward.qualifiedLevel)} / ${number(locale, reward.requiredLevel)}`));
        const bar = element("progress"); bar.max = reward.requiredLevel; bar.value = Math.min(reward.requiredLevel, reward.qualifiedLevel);
        bar.setAttribute("aria-label", `${text("qualifyingLevel")} ${number(locale, reward.qualifiedLevel)}, ${text("requiredLevel")} ${number(locale, reward.requiredLevel)}`);
        progress.append(bar); card.append(progress);
        if (reward.claimed) card.append(element("span", "reward-result", text("registeredReward")));
        else {
          const button = element("button", "secondary register-button", text(view.key)); button.type = "button";
          button.dataset.rewardId = reward.id; button.disabled = !view.canRegister;
          button.addEventListener("click", () => onClaim(reward.id)); card.append(button);
        }
        cards.append(card);
      }
      container.append(phaseHeading, cards);
    }
  }
  container.append(element("p", "delivery-note", text("publicDelivery")));
  const history = element("div", "activity-history");
  if (loggedIn && value) { renderClaimHistory(history, value, locale); container.append(history); }
}

export function renderCampaignRules(container, locale) {
  container.replaceChildren();
  for (const phase of ["closed", "beta", "public"]) {
    const card = element("article", "campaign-rule");
    card.append(element("p", "eyebrow", activityText(locale, phaseKey(phase))), element("p", "", activityText(locale, `${phase}Rules`)));
    container.append(card);
  }
}

export function renderPlayerCenter(container, {value, status, locale, loggedIn, busy, characterIndex, failed = false, onSelect}) {
  container.replaceChildren(); const text = key => activityText(locale, key); const common = LOCALES[locale] ?? LOCALES.en;
  container.append(element("h2", "", text("playerTitle")), element("p", "muted activity-intro", text("savedState")));
  if (!loggedIn || !value) {
    container.append(element("p", "activity-placeholder", text(!loggedIn ? "loginProgress" : failed ? "activityUnavailable" : "loadingProgress"))); return;
  }
  if (failed) container.append(element("p", "muted compact", text("activityUnavailable")));
  container.append(element("h3", "subheading", text("charactersTitle")));
  const roster = element("ul", "character-roster");
  if (!value.characters.length) container.append(element("p", "muted", text("noCharacters")));
  for (const character of value.characters) {
    const row = element("li", "character-row"); row.classList.toggle("selected", character.index === characterIndex);
    const summary = element("div");
    summary.append(element("strong", "character-display-name", character.name), element("small", "", `${classText(locale, character.class)} · ${text("level")} ${number(locale, character.level)}`));
    const button = element("button", "text-button", character.index === characterIndex ? text("selectedCharacter") : common.pay);
    button.type = "button"; button.disabled = busy || character.index === characterIndex;
    button.addEventListener("click", () => onSelect(character.index)); row.append(summary, button); roster.append(row);
  }
  container.append(roster);
  const wallet = element("div", "player-metrics");
  for (const [label, amount] of [[common.balance, status ? number(locale, status.credit) : "—"], [common.pending, status ? number(locale, status.pendingCredit) : "—"]]) {
    const metric = element("div", "player-metric"); metric.append(element("span", "", label), element("strong", "", amount)); wallet.append(metric);
  }
  const monthly = element("div", "player-metric monthly-metric");
  monthly.append(element("span", "", common.monthlyTitle), element("strong", "", status ? (status.monthlyCard.active ? common.monthlyActive : common.monthlyInactive) : "—"));
  if (status?.monthlyCard.active && status.monthlyCard.expiresAtMs) monthly.append(element("small", "", date(locale, status.monthlyCard.expiresAtMs)));
  wallet.append(monthly); container.append(wallet);
  const claims = element("div", "player-claim-history"); renderClaimHistory(claims, value, locale); container.append(claims);
  const carry = value.carryover;
  const ledger = element("section", "carryover-section");
  const title = element("div", "section-heading");
  title.append(element("h3", "subheading", text("carryTitle")), element("span", "settlement-badge", text(carry.settlement === "recorded" ? "recorded" : "reconciliation")));
  ledger.append(title, element("p", "muted compact", text("carryIntro")), element("p", "test-exclusion", text("testExclusion")));
  const metrics = element("dl", "carry-metrics");
  for (const [key, amount] of [["realPayments", new Intl.NumberFormat(locale, {style:"currency",currency:"USD"}).format(carry.realVerifiedUsdMinor / 100)],
    ["verifiedCredits",number(locale, carry.verifiedCredits)], ["monthlyDays",number(locale, carry.monthlyDays)],
    ["excludedTests",number(locale, carry.testOrdersExcluded)], ["reviewOrders",number(locale, carry.reviewOrders)]]) {
    const metric = element("div"); metric.append(element("dt", "", text(key)), element("dd", "", amount)); metrics.append(metric);
  }
  ledger.append(metrics);
  if (!carry.records.length) ledger.append(element("p", "muted compact", text("noCarryOrders")));
  else {
    const records = element("ul", "carry-records");
    for (const record of carry.records) {
      const row = element("li", "carry-record");
      row.append(element("strong", "", new Intl.NumberFormat(locale, {style:"currency",currency:"USD"}).format(record.amountUsdMinor / 100)),
        element("span", "", `${number(locale, record.credits)} ${text("orderCredits")}`), element("small", "", date(locale, record.recordedAtMs)),
        element("span", `order-state ${record.state}`, record.state === "recorded" ? text("recorded") : common.review),
        element("span", "carry-record-id order-id", record.orderId)); records.append(row);
    }
    ledger.append(records);
  }
  container.append(ledger);
}
