// Reviewed display vocabulary. Canonical catalogue IDs and source files are
// inputs only; these rows never change simulation values or script commands.
export function rows(source) {
  return source.trim().split("\n").filter(Boolean).map((line) => {
    const fields = line.split("|");
    if (fields.length !== 3 || fields.some((value) => !value.trim())) throw new Error(`Invalid translation row: ${line}`);
    return fields.map((value) => value.trim());
  });
}
export const commonRows = rows(`
Accept|接受|Aceitar
Finish|完成|Concluir
Cancel|取消|Cancelar
Close|關閉|Fechar
Share|分享|Compartilhar
Track|追蹤|Rastrear
Available|可接取|Disponível
In Progress|進行中|Em andamento
Completed|已完成|Concluída
Complete|完成|Concluir
Ready to turn in|可交付|Pronta para entregar
All|全部|Todas
Gold|金幣|Ouro
Experience|經驗值|Experiência
Equipment|裝備|Equipamento
Skill|技能|Habilidade
Challenge|挑戰|Desafio
Medium|中型|Médio
Small|小型|Pequeno
Large|大型|Grande
Price|價格|Preço
Repair|修理|Reparar
Special repair|特殊修理|Reparo especial
Status|狀態|Status
Chapter|章節|Capítulo
HP potion|生命藥水|Poção de vida
MP potion|魔力藥水|Poção de mana
Amulet|護身符|Amuleto
Poison powder|毒粉|Pó venenoso
Red poison powder|紅毒粉|Pó venenoso vermelho
Green poison powder|綠毒粉|Pó venenoso verde
Town scroll|回城卷|Pergaminho de retorno
Random scroll|隨機卷|Pergaminho de teleporte aleatório
Warrior|戰士|Guerreiro
Wizard|法師|Mago
Taoist|道士|Taoísta
Assassin|刺客|Assassino
Archer|弓箭手|Arqueiro
Male|男|Masculino
Female|女|Feminino
Buy|購買|Comprar
Sell|出售|Vender
Store|存入|Guardar
Retrieve|取回|Retirar
Deposit|存入|Depositar
Withdraw|取出|Retirar
Confirm|確認|Confirmar
Yes|是|Sim
No|否|Não
OK|確定|OK
Back|返回|Voltar
Next|下一頁|Próxima
Previous|上一頁|Anterior
Help|說明|Ajuda
Search|搜尋|Buscar
Sort|排序|Ordenar
Name|名稱|Nome
Level|等級|Nível
Weight|重量|Peso
Durability|耐久度|Durabilidade
Quantity|數量|Quantidade
Inventory|背包|Inventário
Character|角色|Personagem
Skills|技能|Habilidades
Quests|任務|Missões
Options|設定|Opções
Mail|郵件|Correio
Guild|公會|Guilda
Group|隊伍|Grupo
Trade|交易|Troca
Storage|倉庫|Armazém
Shop|商店|Loja
Game Shop|商城|Loja do jogo
Map|地圖|Mapa
World Map|世界地圖|Mapa-múndi
Location|位置|Localização
GO TO|前往|IR ATÉ
SELECT ITEM|選擇物品|SELECIONAR ITEM
Return|交付|Entrega
Reward|獎勵|Recompensa
Tasks|目標|Objetivos
Show All|顯示全部|Mostrar tudo
Online|線上|On-line
Offline|離線|Off-line
System|系統|Sistema
Whisper|私聊|Sussurro
Normal|普通|Normal
Peaceful|和平|Pacífico
Attack|攻擊|Atacar
Follow|跟隨|Seguir
Rest|休息|Descansar
Stop|停止|Parar
Loading...|載入中…|Carregando…
Connecting...|連線中…|Conectando…
Disconnected|連線已中斷|Desconectado
Language|語言|Idioma
English|English|English
Traditional Chinese|繁體中文|繁體中文
Brazilian Portuguese|Português (Brasil)|Português (Brasil)
Exit ({0})|退出 ({0})|Sair ({0})
Fishing ({0})|釣魚 ({0})|Pescar ({0})
Groups ({0})|隊伍 ({0})|Grupos ({0})
Log out ({0})|登出 ({0})|Sair da conta ({0})
Guild ({0})|公會 ({0})|Guilda ({0})
Quests ({0})|任務 ({0})|Missões ({0})
Skills ({0})|技能 ({0})|Habilidades ({0})
Trade ({0})|交易 ({0})|Trocar ({0})
`);

export const commonLegacyAliases = {
  Accept: ["接取", "接受"], Finish: ["完成任務", "完成任务"],
  Available: ["可接取", "可接任务"], "In Progress": ["进行中"],
  Completed: ["已完成"], "Ready to turn in": ["可交付", "可以交付"],
  "HP potion": ["红药", "紅藥", "红药水", "生命药水"],
  "MP potion": ["蓝药", "藍藥", "蓝药水", "魔法药水"],
  Amulet: ["护身符"], "Poison powder": ["毒粉"],
  "Red poison powder": ["红毒粉"], "Green poison powder": ["绿毒粉"],
  "Town scroll": ["回城卷", "回城卷轴"], "Random scroll": ["随机", "随机卷", "隨機卷", "随机传送卷"],
  Medium: ["中型"], Small: ["小型"], Large: ["大型"],
  "Exit ({0})": ["退出 ({0})"], "Fishing ({0})": ["钓鱼 ({0})"],
  "Groups ({0})": ["组队 ({0})"], "Log out ({0})": ["小退 ({0})"],
  "Guild ({0})": ["公会（{0}）"], "Quests ({0})": ["任务 ({0})"],
  "Skills ({0})": ["技能 ({0})"], "Trade ({0})": ["交易 ({0})"],
};

export const npcRoles = rows(`
Teleport|傳送員|Teletransportador
Transport|傳送員|Teletransportador
Teleporter|傳送員|Teletransportador
Assistant|助理|Assistente
CraftsLady|工匠|Artesã
CraftLady|工匠|Artesã
Blacksmith|鐵匠|Ferreiro
Butcher|屠夫|Açougueiro
Merchant|商人|Comerciante
Sailor|水手|Marinheiro
Master|導師|Mestre
HighPriest|大祭司|Sumo sacerdote
HighAssassin|刺客導師|Mestre assassino
Captain|隊長|Capitão
Alchemist|鍊金師|Alquimista
InnKeeper|客棧老闆|Estalajadeiro
Lottery|彩票商|Vendedor de loteria
MaterialDealer|材料商|Comerciante de materiais
MirGuide|傳奇嚮導|Guia de Mir
Examiner|考官|Examinador
WiseFisherman|老漁夫|Pescador sábio
StableGirl|馬廄管理員|Cuidadora do estábulo
Solider|士兵|Soldado
Soldier|士兵|Soldado
TrustMerchant|寄售商|Consignatário
GTMerchant|公會領地商人|Comerciante do território da guilda
SubjagationManager|討伐管理員|Gerente de expedições
SubjugationLead|討伐領隊|Líder de expedição
Premium|特權管理員|Administrador premium
Commander|指揮官|Comandante
TravellingMerchant|旅行商人|Comerciante viajante
Administrator|管理員|Administrador
Emperor|皇帝|Imperador
Specialist|專家|Especialista
Librarian|圖書管理員|Bibliotecário
Hairdresser|理髮師|Cabeleireiro
PetMaster|寵物管理員|Mestre de mascotes
Challange|挑戰者|Desafiante
Sir|爵士|Lorde
MongchonScout|盟重斥候|Batedor de Mongchon
Prison|監獄|Prisão
MasterMage|法師導師|Mestre mago
WickedTrader|黑市商人|Mercador clandestino
Storage|倉庫管理員|Armazenista
Warehouse|倉庫管理員|Armazenista
Mysterious|神祕人|Figura misteriosa
Traveller|旅人|Viajante
General|將軍|General
MasterMK|武術導師|Mestre de artes marciais
Investigator|調查員|Investigador
CaveGuide|洞穴嚮導|Guia da caverna
LeadTrainer|首席教官|Instrutor-chefe
VulnerableSon|虛弱的孩子|Filho debilitado
MongchonDelegate|盟重使者|Representante de Mongchon
Protector|守護者|Protetor
Drapery|布料商|Comerciante de tecidos
BichonInspector|比奇巡察員|Inspetor de Bichon
Inspector|巡察員|Inspetor
VillageChief|村長|Chefe da aldeia
OldFisherman|老漁夫|Velho pescador
LeftEscort|左護衛|Guarda esquerdo
RightEscort|右護衛|Guarda direito
BichonSoldier|比奇士兵|Soldado de Bichon
Excavenger|挖掘者|Escavador
PotionShop|藥水商|Vendedor de poções
Grocery|雜貨商|Merceeiro
TheWatcher|守望者|Vigia
Wanderer|流浪者|Andarilho
Accessory|飾品商|Vendedor de acessórios
FishMonger|魚販|Peixeiro
Book|書商|Livreiro
SquadLeader|小隊長|Líder do esquadrão
GTStore|公會商店|Loja da guilda
`);

export const exactNames = rows(`
SmallHPDrug|小型生命藥水|Poção pequena de vida
MissDo|Do 小姐|Senhorita Do
MissMi|Mi 小姐|Senhorita Mi
MissRe|Re 小姐|Senhorita Re
BichonProvince|比奇省|Província de Bichon
BorderVillage|邊境村|Vila da Fronteira
BichonWall|比奇城牆|Muralha de Bichon
BorderVillage_Board|邊境村公告板|Quadro da Vila da Fronteira
BichonWall_Board|比奇城牆公告板|Quadro da Muralha de Bichon
MudWall_Board|土城公告板|Quadro da Muralha de Barro
CraftingVillage_Portal|工匠村傳送門|Portal da Vila dos Artesãos
StrangeMan|怪人|Homem estranho
MysteriousStone|神祕石|Pedra misteriosa
OldSkeleton|古老骷髏|Esqueleto antigo
Signpost|路標|Placa de sinalização
Pillar|柱子|Pilar
Apprentice|學徒|Aprendiz
TrainerTaoist|道士教官|Instrutor taoísta
BrokenCarriage|損壞的馬車|Carruagem quebrada
CrushedBones|碎骨|Ossos esmagados
BigTaoist|大道士|Grande taoísta
OldSkull|古老頭骨|Crânio antigo
SkeletonPile|骸骨堆|Pilha de esqueletos
SkullPile|頭骨堆|Pilha de crânios
LostSoul|迷失的靈魂|Alma perdida
Ashes|骨灰|Cinzas
WierdPillar|詭異石柱|Pilar estranho
StrangePillar|奇怪石柱|Pilar incomum
MysteriousPillar|神祕石柱|Pilar misterioso
Bones|骨骸|Ossos
OddOldMan|古怪老人|Velho excêntrico
TimeStone|時間石|Pedra do tempo
ChaosNode|混沌節點|Núcleo do caos
Monument|紀念碑|Monumento
Proceeder|接引人|Condutor
GTTransporter|公會領地傳送員|Teletransportador da guilda
GT_BulletinBoard|公會公告板|Quadro da guilda
Gt_BulletinBoard|公會公告板|Quadro da guilda
GT_Peddler|公會小販|Mascate da guilda
GT_Steward|公會管家|Administrador da guilda
Stairs|樓梯|Escadas
Hen|雞|Galinha
Deer|鹿|Cervo
Pig|豬|Porco
Bull|公牛|Touro
Sheep|羊|Ovelha
Wolf|狼|Lobo
Scarecrow|稻草人|Espantalho
HookingCat|鉤貓|Gato com Gancho
RakingCat|釘耙貓|Gato com Ancinho
SpittingSpider|吐絲蜘蛛|Aranha cuspidora
CannibalPlant|食人花|Planta carnívora
Oma|半獸人|Oma
OmaFighter|半獸人勇士|Lutador Oma
OmaWarrior|半獸人戰士|Guerreiro Oma
ForestYeti|森林雪人|Yeti da Floresta
CaveBat|洞穴蝙蝠|Morcego da caverna
CaveMaggot|洞蛆|Larva da caverna
Skeleton|骷髏|Esqueleto
AxeSkeleton|骷髏斧手|Esqueleto com machado
BoneFighter|骷髏勇士|Lutador esqueleto
BoneWarrior|骷髏戰士|Guerreiro esqueleto
BoneElite|骷髏精英|Esqueleto de elite
Zombie|殭屍|Zumbi
Ghoul|屍王|Carniçal
PriestZombie|祭司殭屍|Zumbi sacerdote
ClZombie|爬行殭屍|Zumbi rastejante
NdZombie|腐屍|Zumbi putrefato
RotNdZombie|腐爛殭屍|Zumbi apodrecido
HiGreatGhoul|高階屍王|Carniçal superior
ShamanZombie|薩滿殭屍|Zumbi xamã
Dung|糞蟲|Verme de Esterco
WoomaSoldier|沃瑪戰士|Soldado de Wooma
WoomaFighter|沃瑪勇士|Combatente de Wooma
WoomaWarrior|沃瑪武士|Guerreiro de Wooma
FlamingWooma|火焰沃瑪|Wooma flamejante
WoomaGuardian|沃瑪衛士|Guardião de Wooma
WoomaTaurus|沃瑪教主|Lorde de Wooma
Royal_Guard|皇家衛兵|Guarda real
Royal_Archer|皇家弓箭手|Arqueiro real
Royal_Lieutenant|皇家副官|Tenente real
ArcherGuard|弓箭衛兵|Guarda arqueiro
Guard|衛兵|Guarda
Sentry|哨兵|Sentinela
Trainer|教官|Instrutor
TrainingMob|訓練怪物|Monstro de treino
BoneFamiliar|骷髏僕從|Familiar esqueleto
Shinsu|神獸|Shinsu
HolyDeva|聖靈|Deva sagrado
Clone|分身|Clone
AssassinClone|刺客分身|Clone assassino
VampireSpider|吸血蜘蛛|Aranha vampira
SpittingToad|噴毒蟾蜍|Sapo cuspidor
SnakeTotem|蛇圖騰|Totem de serpentes
CharmedSnake|魅惑之蛇|Serpente encantada
BabyPig|小豬|Porquinho
Chick|小雞|Pintinho
Kitten|小貓|Gatinho
BabySkeleton|小骷髏|Pequeno esqueleto
BlackKitten|黑色小貓|Gatinho preto
BabyDragon|幼龍|Dragão filhote
OlympicFlame|奧運聖火|Chama olímpica
BabySnowMan|小雪人|Boneco de neve pequeno
Currish|惡犬|Cão feroz
Yob|野人|Bárbaro
FrostTiger|冰霜虎|Tigre do gelo
ChestnutTree|栗子樹|Castanheira
EbonyTree|烏木樹|Árvore de ébano
CherryTree|櫻桃樹|Cerejeira
LargeMushroom|大蘑菇|Cogumelo gigante
RedSnake|紅蛇|Cobra vermelha
RedViper|紅色毒蛇|Víbora vermelha
TigerSnake|虎蛇|Cobra-tigre
TigerViper|虎紋毒蛇|Víbora-tigre
Keratoid|角蠅|Mosca cornuda
ShellNipper|鉗蟲|Inseto de pinças
SkyStinger|天刺蟲|Ferrão voador
VisceralWorm|蠕蟲|Verme visceral
SandWorm|沙蟲|Verme da areia
GiantKeratoid|巨型角蠅|Mosca cornuda gigante
Scorpion|蠍子|Escorpião
BoneWhoo|骷髏首領|Chefe esqueleto
CrawlerZombie|爬行殭屍|Zumbi rastejante
WhiteSerpent|白蛇|Serpente branca
BlueViper|藍色毒蛇|Víbora azul
YellowViper|黃色毒蛇|Víbora amarela
GuardianViper|毒蛇守衛|Víbora guardiã
TreasureBox|寶箱|Baú do tesouro
EliteArcher|精英弓箭手|Arqueiro de elite
EliteStatue|精英雕像|Estátua de elite
EliteGuardian|精英守衛|Guardião de elite
EvilSpider|邪惡蜘蛛|Aranha maligna
EvilHog|邪惡野豬|Javali maligno
BlackMaggot|黑蛆|Larva negra
Centipede|蜈蚣|Centopeia
GiantWorm|巨型蠕蟲|Verme gigante
WhimperingBee|鳴叫蜂|Abelha lamentosa
Tongs|鉗蟲|Inseto de pinças
EvilTongs|邪惡鉗蟲|Inseto de pinças maligno
EvilCentipede|邪惡蜈蚣|Centopeia maligna
WedgeMoth|楔蛾|Mariposa-cunha
BugBat|蟲蝠|Morcego-inseto
BugBatMaggot|蟲蝠幼蟲|Larva de morcego-inseto
RedBoar|紅野豬|Javali vermelho
BlackBoar|黑野豬|Javali negro
WhiteBoar|白野豬|Javali branco
SnakeScorpion|蛇蠍|Escorpião-serpente
EvilSnake|邪惡毒蛇|Serpente maligna
KingHog|野豬王|Rei dos javalis
GiantRat|巨鼠|Rato gigante
ZumaArcher|祖瑪弓箭手|Arqueiro de Zuma
ZumaStatue|祖瑪雕像|Estátua de Zuma
ZumaGuardian|祖瑪衛士|Guardião de Zuma
RedThunderZuma|赤雷祖瑪|Zuma do trovão vermelho
ZumaTaurus|祖瑪教主|Lorde de Zuma
RedFoxMan|紅狐人|Homem-raposa vermelho
BlackFoxMan|黑狐人|Homem-raposa negro
WhiteFoxMan|白狐人|Homem-raposa branco
TrapRock|陷阱石|Rocha-armadilha
GuardianRock|守護石|Rocha guardiã
ElectricElement|雷電元素|Elemental elétrico
CloudElement|雲元素|Elemental das nuvens
BrownFrogSpider|棕色蛙蛛|Aranha-sapo marrom
RedFrogSpider|紅色蛙蛛|Aranha-sapo vermelha
GreatFoxSpirit|狐妖王|Grande espírito da raposa
SpiderFrog|蛛蛙|Sapo-aranha
Dark|暗影|Sombra
KingScorpion|蠍王|Rei escorpião
IncarnatedWT|沃瑪化身|Encarnação de Wooma
IncarnatedZT|祖瑪化身|Encarnação de Zuma
DarkDevil|暗黑魔王|Demônio das trevas
BloodPriest|鮮血祭司|Sacerdote do sangue
CursedShaman|詛咒薩滿|Xamã amaldiçoado
CursedPriest|詛咒祭司|Sacerdote amaldiçoado
CursedZombie|詛咒殭屍|Zumbi amaldiçoado
HungryZombie|飢餓殭屍|Zumbi faminto
ShiZombie|屍妖|Zumbi demoníaco
ToxicZombie|劇毒殭屍|Zumbi tóxico
GreatGhoul|巨型屍王|Grande carniçal
BombSpider|爆裂蜘蛛|Aranha explosiva
RootSpider|母蜘蛛|Aranha-mãe
SpiderBat|蛛蝠|Morcego-aranha
VenomSpider|毒蜘蛛|Aranha venenosa
GangSpider|群居蜘蛛|Aranha de bando
GreatSpider|巨型蜘蛛|Aranha gigante
LureSpider|誘惑蜘蛛|Aranha sedutora
BigApe|巨猿|Macaco gigante
EvilApe|邪惡猿猴|Macaco maligno
GreyEvilApe|灰色邪猿|Macaco maligno cinzento
RedEvilApe|紅色邪猿|Macaco maligno vermelho
CrystalSpider|水晶蜘蛛|Aranha de cristal
RedMoonEvil|赤月惡魔|Demônio da lua vermelha
EvilApeSobo|邪猿索波|Sobo, o macaco maligno
EvilBigApe|邪惡巨猿|Macaco gigante maligno
ValeBat|山谷蝙蝠|Morcego do vale
Weaver|織網蜘蛛|Aranha tecelã
VenomWeaver|毒網蜘蛛|Tecelã venenosa
CrackingWeaver|裂網蜘蛛|Tecelã das fendas
ArmingWeaver|裝甲織網蜘蛛|Tecelã blindada
CrystalWeaver|水晶織網蜘蛛|Tecelã de cristal
GreaterWeaver|巨型織網蜘蛛|Grande tecelã
SpiderWarrior|蜘蛛戰士|Guerreiro-aranha
SpiderBarbarian|蜘蛛狂戰士|Bárbaro-aranha
FlyingStatue|飛行雕像|Estátua voadora
StoningStatue|石化雕像|Estátua petrificante
ToxicGhoul|劇毒屍王|Carniçal tóxico
RoninGhoul|浪人屍王|Carniçal ronin
BoneArcher|骷髏弓箭手|Arqueiro esqueleto
BoneBlademan|骷髏刀手|Espadachim esqueleto
BoneSpearman|骷髏槍兵|Lanceiro esqueleto
BoneCaptain|骷髏隊長|Capitão esqueleto
BoneLord|骷髏領主|Lorde esqueleto
Minotaur|牛魔|Minotauro
IceMinotaur|冰牛魔|Minotauro do gelo
ElectricMinotaur|雷牛魔|Minotauro elétrico
WindMinotaur|風牛魔|Minotauro do vento
FireMinotaur|火牛魔|Minotauro do fogo
RightGuard|右護衛|Guarda direito
LeftGuard|左護衛|Guarda esquerdo
MinotaurKing|牛魔王|Rei minotauro
AxeOma|半獸人斧手|Oma com machado
SwordOma|半獸人劍士|Oma espadachim
CrossbowOma|半獸人弩手|Oma besteiro
WingedOma|飛翼半獸人|Oma alado
FlailOma|半獸人鏈錘兵|Oma com mangual
OmaGuard|半獸人護衛|Guarda Oma
YinDevilNode|陰魔節點|Núcleo demoníaco yin
YangDevilNode|陽魔節點|Núcleo demoníaco yang
OmaKing|半獸人王|Rei dos Omas
OmaKingSpirit|半獸人王之魂|Espírito do rei dos Omas
EvilMir|魔龍|Dragão maligno de Mir
MirStatue|傳奇雕像|Estátua de Mir
FrozenDoor|冰封之門|Porta congelada
IcePilar|冰柱|Pilar de gelo
GhastlyLeecher|恐怖吸血怪|Sanguessuga fantasmagórica
MutatedManworm|變異人蟲|Homem-verme mutante
CrazyManworm|瘋狂人蟲|Homem-verme enlouquecido
CyanoGhast|青色幽魂|Espectro ciano
DreamDevourer|吞夢者|Devorador de sonhos
DarkDevourer|暗黑吞噬者|Devorador das trevas
HellSlasher|地獄斬殺者|Ceifador do inferno
HellPirate|地獄海盜|Pirata do inferno
HellCannibal|地獄食人魔|Canibal do inferno
HellBolt|地獄雷魔|Raio do inferno
WitchDoctor|巫醫|Feiticeiro tribal
CaveWitch|洞穴女巫|Bruxa da caverna
HellKeeper|地獄守衛|Guardião do inferno
MubZombie|泥沼殭屍|Zumbi do pântano
MudZombie|泥沼殭屍|Zumbi do pântano
FrozenZombie|冰凍殭屍|Zumbi congelado
DemonWolf|魔狼|Lobo demoníaco
UndeadWolf|亡靈狼|Lobo morto-vivo
WhiteMammoth|白猛獁|Mamute branco
DarkBeast|暗黑巨獸|Fera das trevas
LightBeast|光明巨獸|Fera da luz
BloodBaboon|嗜血狒狒|Babuíno sanguinário
HardenRhino|堅甲犀牛|Rinoceronte encouraçado
DeathCrawler|死亡爬蟲|Rastejante da morte
BurningZombie|燃燒殭屍|Zumbi em chamas
AncBringer|遠古使者|Mensageiro ancestral
BeastKing|獸王|Rei das feras
Hydra|九頭蛇|Hidra
SmallDrake|幼飛龍|Pequeno dragão
Manticore|蠍尾獅|Mantícora
KeelWarrior|龍骨戰士|Guerreiro da quilha
KeelArcher|龍骨弓箭手|Arqueiro da quilha
SandStorm|沙暴|Tempestade de areia
Lamia|蛇妖|Lâmia
Slave|奴隸|Escravo
FineSoul|純淨靈魂|Alma pura
Bear|熊|Urso
TurtleGrass|草龜|Tartaruga da relva
Leopard|豹|Leopardo
Jar|陶罐怪|Vaso animado
FightingCat|格鬥貓|Gato lutador
FireCat|火焰貓|Gato de fogo
CatWidow|寡婦貓|Gata viúva
StainHammerCat|斑紋錘貓|Gato do martelo manchado
BlackHammerCat|黑錘貓|Gato do martelo negro
StrayCat|流浪貓|Gato de rua
CatShaman|薩滿貓|Gato xamã
SeedingsGeneral|幼苗將軍|General dos brotos
RestlessJar|躁動陶罐|Vaso inquieto
GeneralMeowMeow|喵喵將軍|General Miau-Miau
TucsonEgg|圖森之卵|Ovo de Tucson
YoungTucson|幼年圖森|Tucson jovem
Armadillo|犰狳|Tatu
ArmadilloElder|犰狳長老|Tatu ancião
SandSnail|沙蝸牛|Caracol da areia
CannibalTentacles|食人觸手|Tentáculos carnívoros
FrozenSolider|冰封士兵|Soldado congelado
FrozenFighter|冰封勇士|Lutador congelado
FrozenArcher|冰封弓箭手|Arqueiro congelado
FrozenKnight|冰封騎士|Cavaleiro congelado
FrozenGolem|冰封石魔|Golem congelado
IcePhantom|冰霜幻影|Fantasma do gelo
SnowWolf|雪狼|Lobo da neve
FrozenWarewolf|冰封狼人|Lobisomem congelado
FrozenMiner|冰封礦工|Mineiro congelado
FrozenAxeman|冰封斧手|Lenhador congelado
FrozenMagician|冰封魔法師|Mago congelado
SnowYeti|雪原雪人|Yeti da neve
IceCrystalSolider|冰晶士兵|Soldado de cristal de gelo
DarkWraith|暗黑怨靈|Espectro das trevas
DarkSpirit|暗黑靈魂|Espírito das trevas
SinseokMiner|神石礦工|Mineiro de Sinseok
BloodyLureSpider|嗜血誘惑蜘蛛|Aranha sedutora sanguinária
GhostZombie|幽靈殭屍|Zumbi fantasma
DarkPriestZombie|暗黑祭司殭屍|Zumbi sacerdote das trevas
ChainGhoul|鎖鏈屍王|Carniçal das correntes
GameShop_Guard|商城衛兵|Guarda da loja
Football|足球|Bola de futebol
PKSpirit|殺戮之魂|Espírito do combate
Frog|青蛙|Sapo
RedTurtle|紅龜|Tartaruga vermelha
GreenTurtle|綠龜|Tartaruga verde
BlueTurtle|藍龜|Tartaruga azul
TowerTurtle|塔龜|Tartaruga-torre
FinialTurtle|尖塔龜|Tartaruga do pináculo
Hugger|抱面怪|Agarrador
PoisonHugger|劇毒抱面怪|Agarrador venenoso
MutatedHugger|變異抱面怪|Agarrador mutante
HellKnight|地獄騎士|Cavaleiro do inferno
HellBomb|地獄炸彈|Bomba do inferno
HellLord|地獄領主|Lorde do inferno
HornedSorceror|角魔術士|Feiticeiro cornudo
HornedArcher|角魔弓箭手|Arqueiro cornudo
HornedCommander|角魔指揮官|Comandante cornudo
HornedMage|角魔法師|Mago cornudo
HornedWarrior|角魔戰士|Guerreiro cornudo
BoulderSpirit|巨石之靈|Espírito do rochedo
WarriorScroll|戰士卷軸|Pergaminho do guerreiro
TaoistScroll|道士卷軸|Pergaminho do taoísta
WizardScroll|法師卷軸|Pergaminho do mago
AssassinScroll|刺客卷軸|Pergaminho do assassino
SeaTurtle|海龜|Tartaruga marinha
TreeQueen|樹妖女王|Rainha das árvores
StoneTrap|石之陷阱|Armadilha de pedra
SabukGate|沙巴克城門|Portão de Sabuk
PalaceWallLeft|王宮左牆|Muralha esquerda do palácio
PalaceWall|王宮城牆|Muralha do palácio
`);

// Explicitly preserved fictional/personal proper names, not missing prose.
export const properNames = new Set([
  "Baekdon", "Wimaen", "Yimoogi", "KekTal", "Khazard", "Hydrax", "Tucson",
  "Michelangelo", "Raphael", "Donatello", "MissDo", "MissMi", "MissRe",
  "Luke_Thompson", "John_Schwartz", "00", "Yizhili", "HwayoungOkhwan", "Nokyoungokhwan",
]);

exactNames.push(...rows(`
Zombie1|殭屍（一型）|Zumbi (Tipo 1)
Zombie2|殭屍（二型）|Zumbi (Tipo 2)
Zombie3|殭屍（三型）|Zumbi (Tipo 3)
Zombie4|殭屍（四型）|Zumbi (Tipo 4)
Zombie5|殭屍（五型）|Zumbi (Tipo 5)
RotShamanZombie|腐爛薩滿殭屍|Zumbi xamã apodrecido
HedgeKekTal|叢林克塔爾|KekTal da vegetação
BigHedgeKekTal|巨型叢林克塔爾|KekTal gigante da vegetação
HoroBlaster|霍羅炮手|Atirador Horo
BlueHoroBlaster|藍色霍羅炮手|Atirador Horo azul
VioletKekTal|紫色克塔爾|KekTal violeta
ManectricHammer|雷獸錘手|Manectric do martelo
ManectricClub|雷獸棒手|Manectric da clava
ManectricClaw|雷獸爪手|Manectric das garras
ManectricStaff|雷獸杖手|Manectric do cajado
ManectricSlave|雷獸奴僕|Servo Manectric
ManectricBlest|雷獸祭司|Manectric abençoado
ManectricKing|雷獸王|Rei Manectric
Demon|惡魔|Demônio
Minion_Warrior|戰士僕從|Servo guerreiro
Minion_Warlock|術士僕從|Servo bruxo
Minion_Priest|祭司僕從|Servo sacerdote
Minion_Rouge|盜賊僕從|Servo ladino
Chieftain_Warrior|戰士酋長|Chefe guerreiro
Chieftain_Warlock|術士酋長|Chefe bruxo
Chieftain_Priest|祭司酋長|Chefe sacerdote
Chieftain_Rouge|盜賊酋長|Chefe ladino
Chieftain_Archer|弓箭手酋長|Chefe arqueiro
Master_DragonYang|龍陽大師|Mestre Dragon Yang
TucsonFighter|圖森勇士|Combatente Tucson
TucsonMage|圖森法師|Mago Tucson
TucsonWarrior|圖森戰士|Guerreiro Tucson
TucsonGeneral|圖森將軍|General Tucson
RedZuma|赤色祖瑪|Zuma vermelho
_Shinsu(Jude)|朱德的神獸|Shinsu de Jude
GM_Bracelet|管理員：手鐲|GM: braceletes
GM_Necklace|管理員：項鍊|GM: colares
GM_Ring|管理員：戒指|GM: anéis
GM_Armour|管理員：盔甲|GM: armaduras
GM_Boot|管理員：靴子|GM: botas
GM_Belt|管理員：腰帶|GM: cintos
GM_Helmet|管理員：頭盔|GM: elmos
GM_Weapon|管理員：武器|GM: armas
GM_Book|管理員：書籍|GM: livros
GM_Stone|管理員：石頭|GM: pedras
GM_Gem|管理員：寶石|GM: gemas
GM_Meat|管理員：肉類|GM: carnes
GM_Grocery|管理員：雜貨|GM: mercearia
GM_Potion|管理員：藥水|GM: poções
GM_CraftingMaterial|管理員：製作材料|GM: materiais de fabricação
GM_Fishing|管理員：釣魚|GM: pesca
GM_Mount|管理員：坐騎|GM: montarias
GM_Ore|管理員：礦石|GM: minérios
GM_Script|管理員：腳本|GM: scripts
GM_Nothing|管理員：無|GM: nenhum
GM_Quest|管理員：任務|GM: missões
GM_Awakening|管理員：覺醒|GM: despertar
GM_Manager|管理員：管理|GM: administração
GM_Teleporter|管理員：傳送|GM: teletransporte
BattleCry|戰吼|Grito de Guerra
BindingShot|束縛射擊|Disparo Aprisionador
CatTongue|貓舌術|Língua de Gato
CresentSlash|新月斬|Corte Crescente
DoubleSlash|雙重斬擊|Corte Duplo
FatalSword|致命劍術|Espada Fatal
FireBall|火球術|Bola de Fogo
FireBounce|彈射火焰|Chama Ricocheteante
GreatFireBall|大火球|Grande Bola de Fogo
Haste|疾速|Aceleração
HellFire|地獄火|Fogo Infernal
MentalState|精神集中|Estado Mental
MeteorShower|流星雨|Chuva de Meteoros
MoonLight|月光|Luar
Portal|傳送門|Portal
Stonetrap|石之陷阱|Armadilha de Pedra
ThunderBolt|雷電術|Raio
ThunderStorm|地獄雷光|Tempestade de Raios
UltimateEnchancer|無極真氣|Aprimoramento Supremo
UltimateEnhancer|無極真氣|Aprimoramento Supremo
Fencing|基本劍術|Esgrima
Slaying|攻殺劍術|Golpe Mortal
Thrusting|刺殺劍術|Estocada
HalfMoon|半月彎刀|Meia-Lua
SoulFireBall|靈魂火符|Talismã de Fogo
SpiritSword|精神力戰法|Técnica Espiritual da Espada
`));

export const skillTitleOverrides = {
  Slaying: "Golpe Mortal", SoulFireBall: "Talismã de Fogo", SpiritSword: "Técnica Espiritual da Espada",
};

export const itemNouns = rows(`
Sword|劍|Espada
Blade|刀|Lâmina
Blades|雙刃|Lâminas
Knife|短刀|Faca
Rod|杖|Vara
Bayonet|刺刀|Baioneta
Helmet|頭盔|Elmo
Necklace|項鍊|Colar
Bracelet|手鐲|Bracelete
Brace|手鐲|Bracelete
Ring|戒指|Anel
Wheel|輪|Roda
Belt|腰帶|Cinto
Boots|靴|Botas
Shoes|鞋|Sapatos
Armour|盔甲|Armadura
Armor|盔甲|Armadura
Robe|長袍|Manto
Hood|兜帽|Capuz
Glove|手套|Luva
Gauntlet|護手|Manopla
Pendant|墜飾|Pingente
Amulet|護身符|Amuleto
Collar|頸飾|Coleira
Orb|寶珠|Orbe
Gem|寶石|Gema
Stone|石|Pedra
Circle|圓環|Aro
Water|水|Água
Bow|弓|Arco
Dagger|匕首|Adaga
Axe|斧|Machado
Mace|權杖|Maça
Staff|法杖|Cajado
Scythe|鐮刀|Foice
Spear|矛|Lança
Wand|魔杖|Varinha
Sabre|刀|Sabre
Hammer|錘|Martelo
Fan|扇|Leque
Dress|衣|Vestido
Coronet|冠|Diadema
Mask|面具|Máscara
Torch|火把|Tocha
Bell|鈴鐺|Sino
Ribbon|緞帶|Fita
Bridle|韁繩|Rédea
FishingRod|魚竿|Vara de pesca
FishingHook|魚鉤|Anzol
Float|浮標|Boia
Bait|魚餌|Isca
Finder|探魚器|Localizador de peixes
Reel|捲線器|Carretilha
Ore|礦石|Minério
Meat|肉|Carne
Chestnut|栗子|Castanha
Fruit|果實|Fruta
Tea|茶|Chá
Wine|酒|Vinho
Liquor|烈酒|Licor
Potion|藥水|Poção
Drug|藥劑|Remédio
Aid|補劑|Suplemento
Soup|湯|Sopa
Broth|肉湯|Caldo
Dumpling|餃子|Bolinho
Book|書|Livro
Scroll|卷軸|Pergaminho
Token|代幣|Ficha
Egg|卵|Ovo
Pill|藥丸|Pílula
Box|盒|Caixa
Chest|寶箱|Baú
Glyph|銘文|Glifo
Leather|皮革|Couro
Scale|鱗片|Escama
Marble|珠石|Mármore
Oil|油|Óleo
Horn|角|Chifre
Blood|血|Sangue
Heart|心臟|Coração
Tooth|牙|Dente
Teeth|牙齒|Dentes
Leg|腿|Perna
Eye|眼|Olho
Shell|甲殼|Carapaça
Thread|線|Linha
Skull|頭骨|Crânio
Bone|骨|Osso
Letter|信|Carta
Report|報告|Relatório
Rope|繩索|Corda
Strap|帶|Tira
Bangle|手環|Pulseira
Buckle|扣環|Fivela
Bead|珠|Conta
Flute|笛|Flauta
Crystal|水晶|Cristal
Underpants|內褲|Roupa íntima
Marshmallow|棉花糖|Marshmallow
Suit|套裝|Traje
Pike|長槍|Pique
Claw|爪|Garra
Club|棍棒|Clava
Package|禮包|Pacote
Pass|通行證|Passe
`);

export const itemModifiers = rows(`
Spirit|靈魂|Espírito
Recall|記憶|Retorno
RedOrchid|紅蘭|Orquídea Vermelha
RedFlower|紅花|Flor Vermelha
Smash|破擊|Impacto
HwanDevil|幻魔|Demônio Hwan
Purity|純淨|Pureza
FiveString|五弦|Cinco Cordas
Mundane|凡塵|Mundo Terreno
NokChi|綠翠|NokChi
TaoProtect|道護|Proteção Taoísta
Mir|傳奇|Mir
Bone|白骨|Osso
Bug|蟲|Inseto
WhiteGold|白金|Ouro Branco
RedJade|紅玉|Jade Vermelho
Nephrite|翡翠|Nefrita
Mystery|神祕|Mistério
Strength|力量|Força
Tarragon|龍紋|Tarragon
Raiders|掠奪者|Saqueador
Agony|苦痛|Agonia
Sparkling|閃耀|Brilho
Darkness|黑暗|Trevas
Cheongbo|青寶|Cheongbo
Wooden|木|Madeira
Ebony|烏木|Ébano
Short|短|^Curto
Iron|鐵|Ferro
Steel|鋼|Aço
Prince|王子|Príncipe
FireBlood|火血|Sangue de Fogo
HellYama|閻魔|Yama Infernal
Hooked|鉤|^Curvo
Martial|武|Arte Marcial
Power|力量|Poder
Purifier|淨化|Purificação
Great|巨|^Grande
ZumaJudgement|祖瑪裁決|Julgamento de Zuma
Judgement|裁決|Julgamento
Dragon|龍|Dragão
BlackDragon|黑龍|Dragão Negro
WarSpirit|戰魂|Espírito de Guerra
WarGod|戰神|Deus da Guerra
Burst|爆裂|Explosão
Frozen|冰封|Gelo
BlackTiger|黑虎|Tigre Negro
Barbarian|野蠻|Bárbaro
Mage|法師|Mago
BloodStealer|噬血|Roubo de Sangue
ZumaWarMage|祖瑪戰法|Mago de Guerra de Zuma
WarMage|戰法|Mago de Guerra
Magic|魔法|Magia
Sorcery|巫術|Feitiçaria
HolyBlood|聖血|Sangue Sagrado
Conqueror|征服者|Conquistador
Lotus|蓮花|Lótus
Raw|粗製|^Bruto
Kriss|波紋|Kris
Serpent|蛇|Serpente
ZumaSoulSpring|祖瑪靈泉|Fonte Espiritual de Zuma
SoulSpring|靈泉|Fonte Espiritual
BlackDragonSoul|黑龍魂|Alma do Dragão Negro
StoneBamboo|石竹|Bambu de Pedra
Soul|靈魂|Alma
Heaven|天界|Céu
DragonBlood|龍血|Sangue de Dragão
Bastard|混血|Bastardo
Crane|鶴|Garça
BoneCarved|雕骨|Osso Entalhado
Hoa|花|Hoa
ToughHoa|堅韌花|Hoa Resistente
SharpHoa|鋒利花|Hoa Afiada
Velocity|疾速|Velocidade
Double|雙|^Duplo
Zuma|祖瑪|Zuma
Dragon'sRoyal|龍皇|Realeza Dracônica
Royal|皇家|Realeza
Magi|賢者|Magi
Zinc|鋅|Zinco
Butchering|屠宰|Carnificina
MadDragon|狂龍|Dragão Furioso
Compound|複合|^Composto
Long|長|^Longo
Silver|銀|Prata
ZumaFiend|祖瑪魔|Demônio de Zuma
Fiend|惡魔|Demônio
CursedMalefic|詛咒邪惡|Malefício Amaldiçoado
Lithe|輕巧|^Leve
Malefic|邪惡|Malefício
Force|力量|Força
Dread|恐懼|Terror
Grim|陰森|^Sombrio
Lethal|致命|Letalidade
Apus|阿普斯|Apus
Death|死亡|Morte
BloodThirsty|嗜血|Sede de Sangue
Base|布|^Básico
Light|輕型|^Leve
Medium|中型|^Médio
Heavy|重型|^Pesado
LightLeather|輕皮|Couro Leve
Wizard|法師|Mago
Witch|女巫|Bruxa
Pearl|珍珠|Pérola
Ancient|遠古|^Ancestral
Tempered|淬鍊|Têmpera
Titan|泰坦|Titã
Studded|鑲釘|Rebites
Stealth|隱匿|Furtividade
LightTempered|輕淬|Têmpera Leve
BlueDark|藍暗|Trevas Azuis
RedDark|紅暗|Trevas Vermelhas
GreenDark|綠暗|Trevas Verdes
GoldDark|金暗|Trevas Douradas
Scaled|鱗|Escamas
Crystal|水晶|Cristal
OmaKing|半獸王|Rei dos Omas
Paralysis|麻痺|Paralisia
Teleport|傳送|Teletransporte
Clear|透明|Clareza
Protection|防護|Proteção
Revival|復活|Renascimento
Muscle|肌力|Músculo
Flame|火焰|Chama
Recovery|恢復|Recuperação
Hardened|堅固|^Endurecido
Copper|銅|Cobre
Hexagonal|六角|^Hexagonal
Glass|玻璃|Vidro
Horn|角|Chifre
WhiteJade|白玉|Jade Branco
Blue|藍|^Azul
Gale|疾風|Vendaval
Skeleton|骷髏|Esqueleto
Black|黑|^Negro
SerpentEye|蛇眼|Olho de Serpente
Gold|金|Ouro
Golden|黃金|Ouro
Charm|魅力|Encanto
Expel|驅邪|Expulsão
Coral|珊瑚|Coral
Ruby|紅寶石|Rubi
Platinum|白金|Platina
EvilSlayer|斬魔|Caça aos Demônios
JadeSnow|玉雪|Neve de Jade
TwinGold|雙金|Ouro Duplo
Violet|紫羅蘭|Violeta
Boundless|無極|Infinito
Thunder|雷霆|Trovão
TaeGuk|太極|TaeGuk
OmaSpirit|半獸人魂|Espírito Oma
Noble|貴族|Nobreza
Pledge|誓約|Juramento
CrimsonRuby|赤紅寶石|Rubi Carmesim
FiveElement|五行|Cinco Elementos
GoldDragon|金龍|Dragão Dourado
GoldenDragon|金龍|Dragão Dourado
DemonRuby|魔紅寶石|Rubi Demoníaco
EvilExpel|驅魔|Exorcismo
PurpleFox|紫狐|Raposa Roxa
RedFox|紅狐|Raposa Vermelha
BlueFox|藍狐|Raposa Azul
GreatPurpleFox|大紫狐|Grande Raposa Roxa
GreatRedFox|大紅狐|Grande Raposa Vermelha
GreatBlueFox|大藍狐|Grande Raposa Azul
Survival|生存|Sobrevivência
Seal|封印|Selo
Taji|太極|Taiji
Craft|工藝|Artesanato
RedDemon|赤魔|Demônio Vermelho
Cloud|雲|Nuvem
Poison|毒|Veneno
EvilDragon|魔龍|Dragão Maligno
RDragon|赤龍|Dragão Vermelho
Thin|細|^Fino
Leather|皮革|Couro
Large|大型|^Grande
Hard|堅硬|^Duro
Evade|閃避|Esquiva
Monk|僧侶|Monge
Strain|韌性|Resistência
3rdEye|第三眼|Terceiro Olho
Spell|咒術|Feitiço
Knight|騎士|Cavaleiro
BlackIron|黑鐵|Ferro Negro
TaoPower|道力|Poder Taoísta
8Trigram|八卦|Oito Trigramas
HangMa|降魔|HangMa
BokMa|伏魔|BokMa
BaekTa|白塔|BaekTa
HolyTao|聖道|Tao Sagrado
DualTitan|雙泰坦|Titãs Gêmeos
EvilWhisp|邪靈|Espectro Maligno
SacredAngel|聖天使|Anjo Sagrado
WildTitan|狂泰坦|Titã Selvagem
Enchanted|附魔|Encantamento
Lunar|月|Lua
Precision|精準|Precisão
Yellow|黃|^Amarelo
Jade|玉|Jade
Tiger|虎|Tigre
Elusion|閃避|Evasão
Naga|娜迦|Naga
Amber|琥珀|Âmbar
Phoenix|鳳凰|Fênix
Lantern|燈籠|Lanterna
Warrior|戰士|Guerreiro
BlueCrystal|藍水晶|Cristal Azul
BlueJade|藍玉|Jade Azul
BlueThunder|藍雷|Trovão Azul
SpiritPower|靈力|Poder Espiritual
Claw|利爪|Garra
Life|生命|Vida
Amethyst|紫水晶|Ametista
Green|綠|^Verde
Hero|英雄|Herói
Adamantine|精金|Adamantina
Adamant|精金|Adamante
Requiem|安魂|Réquiem
Cuspid|獠牙|Presa
DragonPendant|龍墜|Pingente de Dragão
Probe|探測|Sondagem
Skill|技能|Habilidade
Brass|黃銅|Latão
Shaman|薩滿|Xamã
Wisdom|智慧|Sabedoria
Tao|道|Tao
Kings|王者|Reis
Purified|淨化|Purificação
Chain|鎖鏈|Corrente
Rotten|破舊|^Apodrecido
Low|矮|^Baixo
Silk|絲|Seda
RedScale|赤鱗|Escama Vermelha
GoldPattern|金紋|Desenho Dourado
Health|生命|Vida
DC|攻擊|Ataque Físico
MC|魔法|Magia
SC|道術|Tao
Impact|衝擊|Impacto
Taoist|道術|Tao
Storm|暴風|Tempestade
Stamina|體力|Vigor
Spencer|斯賓塞|Spencer
Beef|牛肉|Carne Bovina
Chicken|雞肉|Frango
Egg|蛋|Ovo
Herbal|草藥|Ervas
Meat|肉|Carne
Zerk|狂暴|Fúria
Repair|修理|Reparo
Benediction|祝福|Bênção
Red|紅|^Vermelho
GreenPoison|綠毒|Veneno Verde
Knowledge|智慧|Conhecimento
Bravery|勇氣|Bravura
Durability|耐久|Durabilidade
Agility|敏捷|Agilidade
Accuracy|準確|Precisão
Freezing|冰凍|Congelamento
Disillusion|破幻|Desilusão
Endurance|耐力|Resistência
Brown|棕|^Marrom
Titanium|鈦|Titânio
Premium|高級|^Premium
Fish|魚|Peixe
Fishing|釣魚|Pesca
Emerald|翡翠|Esmeralda
Olivine|橄欖石|Olivina
Adamintine|精金|Adamantina
Unknown|未知|^Desconhecido
Admission|入場|Admissão
Guardian|守護者|Guardião
Bichon|比奇|Bichon
Venison|鹿肉|Carne de Cervo
Mutton|羊肉|Carne de Carneiro
Deer|鹿|Cervo
Spider|蜘蛛|Aranha
Wooma|沃瑪|Wooma
Maggot|蛆|Larva
Scorpion|蠍|Escorpião
SkyStinger|天刺蟲|Ferrão Voador
RedEye|紅眼|Olho Vermelho
Wind|風|Vento
Evasion|迴避|Evasão
Focus|專注|Concentração
Ice|冰|Gelo
Will|意志|Vontade
Antidote|解毒|Antídoto
Healing|治癒|Cura
Pig|豬|Porco
RedMoon|赤月|Lua Vermelha
EvilApe|邪猿|Macaco Maligno
JadeCrystal|玉晶|Cristal de Jade
Demon|惡魔|Demônio
Undead|亡靈|Morto-vivo
Mammoth|猛獁|Mamute
Beast|巨獸|Fera
Baboon|狒狒|Babuíno
Rhino|犀牛|Rinoceronte
Body|體魄|Corpo
BabyPig|小豬|Porquinho
Chick|小雞|Pintinho
Kitten|小貓|Gatinho
Baekdon|白豚|Baekdon
Wimaen|維曼|Wimaen
BlackKitten|黑色小貓|Gatinho Preto
BabyDragon|幼龍|Dragão Filhote
Olympic|奧運|Olímpico
SnowMan|雪人|Boneco de Neve
Frog|青蛙|Sapo
BlackCreature|黑色寵物|Mascote Negro
Wonder|奇蹟|Maravilha
Woomas|沃瑪|Wooma
Stone|石|Pedra
Prajna|般若|Prajna
Cannibal|食人花|Planta Carnívora
Ginger|薑|Gengibre
Oma|半獸人|Oma
Snake|蛇|Serpente
RedSnake|紅蛇|Serpente Vermelha
CannibalFruit|食人花果|Fruta Carnívora
Bat|蝙蝠|Morcego
Zombie|殭屍|Zumbi
Cave|洞穴|Caverna
Old|舊|^Antigo
RareCopper|稀有銅|Cobre Raro
BluJade|藍玉|Jade Azul
Gathering|採集|Coleta
Mob|怪物|Monstro
Stolen|失竊|^Roubado
Hidden|隱藏|^Oculto
Dead|死亡|Morte
Trainee|學徒|Aprendiz
Zombies|殭屍|Zumbi
Worn|磨損|^Gasto
Skys|天空|Céu
Charley|查理|Charley
Corps|屍體|Cadáver
Clean|潔淨|^Limpo
Relic|遺物|Relíquia
Boar|野豬|Javali
StiffWooden|硬木|Madeira Rígida
OldCopper|舊銅|Cobre Antigo
WornIron|磨損鐵|Ferro Gasto
BronzeWarrior|青銅戰士|Guerreiro de Bronze
StrongHoa|強化花|Hoa Reforçada
StrongWooden|強化木|Madeira Reforçada
BronzeShort|青銅短|Bronze Curto
BronzeHoa|青銅花|Hoa de Bronze
ElkWooden|麋鹿木|Madeira de Alce
StrongLeather|強韌皮革|Couro Resistente
BlackCrystal|黑水晶|Cristal Negro
YellowCrystal|黃水晶|Cristal Amarelo
Solid|堅固|^Sólido
Broken|破損|^Quebrado
SuperiorBronze|優質青銅|Bronze Superior
SolidBronze|堅固青銅|Bronze Sólido
Sharp|鋒利|^Afiado
Tough|堅韌|^Resistente
SuperiorMagic|優質魔法|Magia Superior
OldNaga|古老娜迦|Naga Ancestral
Bronze|青銅|Bronze
HardSteel|堅鋼|Aço Endurecido
BlackPearl|黑珍珠|Pérola Negra
KeenKriss|銳利波紋|Kris Afiado
ToughLong|堅韌長|Longa Resistência
Thick|厚重|^Espesso
FireMagic|火魔法|Magia de Fogo
Fire|火焰|Fogo
Moral|道德|Virtude
SolidBlack|堅固黑|Ébano Sólido
Exorcist|驅魔|Exorcista
SolidGreat|堅固巨|Grande Resistência
SolidSerpent|堅固蛇|Serpente Resistente
SolidTwin|堅固雙|Gêmeos Resistentes
SolidSilver|堅固銀|Prata Sólida
FineIron|精鐵|Ferro Refinado
YingYang|陰陽|Yin-Yang
FlamingRed|烈火紅|Chama Vermelha
HardTempered|堅韌淬鍊|Têmpera Reforçada
Credit|商城點數|Crédito
Destruction|破壞|Destruição
Magical|魔法|Magia
White|白|^Branco
Mongchon|盟重|Mongchon
Blood|鮮血|Sangue
BlueOctagonal|藍八角|Octógono Azul
RedOctagonal|紅八角|Octógono Vermelho
GreenOctagonal|綠八角|Octógono Verde
Snow|雪|Neve
Pink|粉紅|^Rosa
IceDragonSky|冰龍天|Céu do Dragão de Gelo
IronThorn|鐵刺|Espinho de Ferro
BluishGreenBloodSlaughter|碧血屠戮|Carnificina de Sangue Verde-Azulado
Retsuen|烈焰|Retsuen
YellowDragon|黃龍|Dragão Amarelo
InfernalBlood|魔血|Sangue Infernal
SoulEater|噬魂|Devorador de Almas
LeopardTattoo|豹紋|Estampa de Leopardo
GonRyunHolyLight|崑崙聖光|Luz Sagrada de GonRyun
HyeoncheonSuho|玄天守護|Proteção de Hyeoncheon
PoisonDemon|毒魔|Demônio Venenoso
SkyHero|天英雄|Herói Celestial
`);

export const mapNames = rows(`
BichonProvince|比奇省|Província de Bichon
Palace|王宮|Palácio
MeatStore|肉店|Açougue
ReagentStore|試劑店|Loja de reagentes
MedicineRoom|藥房|Sala de remédios
Library|圖書館|Biblioteca
BeautySaloon|美容院|Salão de beleza
Inn|客棧|Estalagem
2FofInn|客棧二樓|Estalagem · 2º andar
WeaponStore|武器店|Loja de armas
AccessoryStore|飾品店|Loja de acessórios
DrapersStore|布莊|Loja de tecidos
DraperyStore|布莊|Loja de tecidos
Tavern|酒館|Taverna
Kitchen|廚房|Cozinha
PrivateHouse|民宅|Residência
Academy|學院|Academia
ContestGround|比武場|Arena de duelos
B1ContestGround|地下比武場|Arena de duelos · subsolo
Barracks|兵營|Quartel
Prison|監獄|Prisão
BorderRestaurant|邊境餐館|Restaurante da fronteira
B1Residence|地下住宅|Residência subterrânea
GroceryStore|雜貨店|Mercearia
Warehouse|倉庫|Armazém
MageHouse|法師之家|Casa do mago
NaturalCave|天然洞穴|Caverna natural
HiddenRoom|隱藏房間|Sala oculta
ConnectionPath|連接通道|Passagem de ligação
OmaCave|半獸人洞穴|Caverna dos Omas
KingsTomb|國王陵墓|Tumba do rei
AncientNaturalCave|遠古天然洞穴|Caverna natural ancestral
DeadMineEntrance|死亡礦區入口|Entrada da Mina Morta
B1ofMine|礦區地下一層|Mina · 1º subsolo
EastofDeadMine|死亡礦區東部|Leste da Mina Morta
B2ofDeadMine|死亡礦區地下二層|Mina Morta · 2º subsolo
OverPass|天橋|Passarela
1FofDeadMine|死亡礦區一層|Mina Morta · 1º andar
OreStoragePlace|礦石倉庫|Depósito de minério
SouthofDeadMine|死亡礦區南部|Sul da Mina Morta
GhoulCave|屍王洞|Caverna do carniçal
WoomyonWoods|沃瑪森林|Floresta de Woomyon
InsectCave|蟲洞|Caverna dos insetos
WoomaTempleEntrance|沃瑪寺廟入口|Entrada do Templo de Wooma
WoomaTemple|沃瑪寺廟|Templo de Wooma
AncientTempleEntrance|遠古神廟入口|Entrada do templo ancestral
AncientTemple|遠古神廟|Templo ancestral
CastleGi-Ryoong|吉龍城|Castelo Gi-Ryoong
InnerCastle|內城|Interior do castelo
PotionStore|藥水店|Loja de poções
ItemDepository|物品保管所|Depósito de itens
BlackDragonDungeon|黑龍地牢|Calabouço do dragão negro
BlackSnakePalace|黑蛇宮|Palácio da serpente negra
PurgatoryHall|煉獄大廳|Salão do purgatório
WoomaPalace|沃瑪宮殿|Palácio de Wooma
HwanMaJin|幻魔陣|HwanMaJin
NobleHogPalace|野豬王宮|Palácio do javali nobre
SoleSpiritHall|孤魂殿|Salão da alma solitária
ZumaPalace|祖瑪宮殿|Palácio de Zuma
PrisonHall|監獄大廳|Salão da prisão
DarkDevilPalace|暗黑魔王宮|Palácio do demônio das trevas
WhiteDragonPassage|白龍通道|Passagem do dragão branco
WhiteDragonHillSide|白龍山坡|Encosta do dragão branco
SeokchoValley|石草山谷|Vale de Seokcho
TrollMine|巨魔礦洞|Mina dos trolls
Armory|軍械庫|Arsenal
Rev.Tao_Office|道士辦公室|Gabinete do reverendo taoísta
Laundry|洗衣房|Lavanderia
Item_Lab|物品實驗室|Laboratório de itens
Drug_Store|藥材店|Loja de remédios
MineralMine|礦洞|Mina de minerais
Treepath|林間小徑|Trilha das árvores
RedValley|赤色山谷|Vale vermelho
Great_TaoTomb|大道士陵墓|Tumba do grande taoísta
RedMoonRoom|赤月密室|Câmara da lua vermelha
Lunar|月宮|Palácio lunar
LunarRoom|月宮密室|Câmara lunar
DarkForest|黑暗森林|Floresta sombria
TownHall|城鎮大廳|Prefeitura
SerpentValley|毒蛇山谷|Vale das Serpentes
S_ValleyTavern|毒蛇山谷酒館|Taverna do Vale das Serpentes
Blacksmith|鐵匠鋪|Ferraria
S_ValleyDeadMine|毒蛇山谷廢礦|Mina Morta do Vale das Serpentes
ViperMaze|毒蛇迷宮|Labirinto das víboras
ViperPath|毒蛇通道|Passagem das víboras
ViperCave|毒蛇洞穴|Caverna das víboras
YimoogiNest|伊穆基巢穴|Ninho de Yimoogi
MysteryCave|神祕洞穴|Caverna misteriosa
MongchonProvince|盟重省|Província de Mongchon
Mill|磨坊|Moinho
S_1FofDungeon|地牢一層南部|Calabouço · 1º andar sul
W_1FofDungeon|地牢一層西部|Calabouço · 1º andar oeste
N_1FofDungeon|地牢一層北部|Calabouço · 1º andar norte
W_2FofDungeon|地牢二層西部|Calabouço · 2º andar oeste
N_2FofDungeon|地牢二層北部|Calabouço · 2º andar norte
LifeDeathCoffin|生死棺|Caixão da vida e da morte
StoneLanternRoom|石燈室|Sala da lanterna de pedra
AmethystRoom|紫晶室|Sala de ametista
StoneCarvedStream|石刻溪流|Riacho esculpido em pedra
StrangeRocksRoom|怪石室|Sala das rochas estranhas
StrangeRocksPath|怪石通道|Passagem das rochas estranhas
LostCave(MainCave)|迷失洞穴主洞|Caverna perdida · câmara principal
LostMiddleCave|迷失洞穴中部|Caverna perdida central
SouthernLostCave|迷失洞穴南部|Caverna perdida sul
WesternLostCave|迷失洞穴西部|Caverna perdida oeste
NorthernLostCave|迷失洞穴北部|Caverna perdida norte
EasternLostCave|迷失洞穴東部|Caverna perdida leste
FatallyPoisonousCave|劇毒洞穴|Caverna do veneno mortal
B1_AngledStoneTomb|石墓地下一層|Tumba de pedra · 1º subsolo
B2_AngledStoneTomb|石墓地下二層|Tumba de pedra · 2º subsolo
B3_AngledStoneTomb|石墓地下三層|Tumba de pedra · 3º subsolo
B4_AngledStoneTomb|石墓地下四層|Tumba de pedra · 4º subsolo
B5_AngledStoneTomb|石墓地下五層|Tumba de pedra · 5º subsolo
B6_AngledStoneTomb|石墓地下六層|Tumba de pedra · 6º subsolo
B7_AngledStoneTomb|石墓地下七層|Tumba de pedra · 7º subsolo
TacticalMaze|戰術迷宮|Labirinto tático
AngledStoneTombEntrance|石墓入口|Entrada da tumba de pedra
AncientSTombEntrance|遠古石墓入口|Entrada da tumba de pedra ancestral
AncientSTomb|遠古石墓|Tumba de pedra ancestral
ConnectionCave|連接洞穴|Caverna de ligação
SabukSecretGate|沙巴克密門|Portão secreto de Sabuk
InnerWall|內城牆|Muralha interna
LobbyofZumaTemple|祖瑪神殿大廳|Saguão do templo de Zuma
ZumaTemple|祖瑪神殿|Templo de Zuma
ZumaMaze|祖瑪迷宮|Labirinto de Zuma
ZumaTempleLobby|祖瑪神殿大廳|Saguão do templo de Zuma
AncientZumaLobby|遠古祖瑪大廳|Saguão ancestral de Zuma
AncientZuma|遠古祖瑪|Zuma ancestral
SealedMaze|封印迷宮|Labirinto selado
SacredFoxHill|聖狐山|Colina da raposa sagrada
SacredFoxTemple|聖狐神殿|Templo da raposa sagrada
SwampCavern|沼澤洞穴|Caverna do pântano
Swamp|沼澤|Pântano
DarkSwamp|黑暗沼澤|Pântano sombrio
ForgottenCity|遺忘之城|Cidade esquecida
PrajnaIsland|般若島|Ilha de Prajna
VillageChiefHouse|村長之家|Casa do chefe da aldeia
FisherManHouse|漁夫之家|Casa do pescador
Shrine|祠堂|Santuário
PrajnaStoneCave|般若石窟|Caverna de pedra de Prajna
PrajnaTempleLobby|般若神殿大廳|Saguão do templo de Prajna
PrajnaTemple|般若神殿|Templo de Prajna
AncientPrajna|遠古般若|Prajna ancestral
PastBichon|昔日比奇|Bichon do passado
GeneralsCamp|將軍營地|Acampamento do general
WestOmaGorge|半獸人峽谷西部|Garganta dos Omas · oeste
WestOmaPass|半獸人隘口西部|Passagem dos Omas · oeste
OmaGorge|半獸人峽谷|Garganta dos Omas
EastOmaPass|半獸人隘口東部|Passagem dos Omas · leste
WestOmaValley|半獸人山谷西部|Vale dos Omas · oeste
OmaValley|半獸人山谷|Vale dos Omas
EastOmaValley|半獸人山谷東部|Vale dos Omas · leste
EastOmaGorge|半獸人峽谷東部|Garganta dos Omas · leste
WestPass|西部隘口|Passagem oeste
EastPass|東部隘口|Passagem leste
BloodGorge|血色峽谷|Garganta de sangue
BloodPass|血色隘口|Passagem de sangue
BloodLand|血色大地|Terra de sangue
LightningCave|雷電洞穴|Caverna do relâmpago
MoltenRockCave|熔岩洞穴|Caverna de rocha fundida
EvilMirPalace|魔龍宮殿|Palácio do dragão maligno
WasteLands|荒原|Terras devastadas
HellCavern|地獄洞穴|Caverna do inferno
HellOverpass|地獄天橋|Passarela do inferno
IceHellCavern|冰獄洞穴|Caverna do inferno gelado
IceHellTemple|冰獄神殿|Templo do inferno gelado
DangerousCavern|險惡洞穴|Caverna perigosa
IceHellPass|冰獄隘口|Passagem do inferno gelado
IceHellTemple_KR|冰獄神殿王室|Templo do inferno gelado · sala do rei
HellFire|地獄火|Fogo infernal
HellFire_KingsRoom|地獄火王室|Fogo infernal · sala do rei
RedCavern|紅色洞穴|Caverna vermelha
RedCavern_KR|紅色洞穴王室|Caverna vermelha · sala do rei
DeadForest|死亡森林|Floresta morta
CastleRuins|城堡廢墟|Ruínas do castelo
Ruins|廢墟|Ruínas
HauntedForest|鬧鬼森林|Floresta assombrada
WhiteValley|白色山谷|Vale branco
GeneralStore|綜合商店|Loja geral
TrainingHall|訓練館|Salão de treinamento
BookStore|書店|Livraria
SnowCavern|雪洞|Caverna de neve
TheSnowLair|雪原巢穴|Covil da neve
WaitingRoom|等候室|Sala de espera
InfiniteBout|無盡試煉|Combate infinito
GameMaster|遊戲管理區|Área do mestre do jogo
PenalCavern|懲戒洞穴|Caverna penal
CraftingVillage|工匠村|Vila dos artesãos
ArchersHideout|弓箭手藏身處|Refúgio dos arqueiros
GTCourt|公會庭院|Pátio da guilda
GT|公會領地|Território da guilda
DogYoArena|道幽競技場|Arena de DogYo
DogYoHyun|道幽縣|Distrito de DogYo
DogYoMineLobby|道幽礦洞大廳|Saguão da mina de DogYo
DY_Pharmacy|道幽藥房|Farmácia de DogYo
DY_Craft|道幽工坊|Oficina de DogYo
DY_Head|道幽總部|Sede de DogYo
DY_MedStorage|道幽藥材倉庫|Depósito medicinal de DogYo
DY_Smithy|道幽鐵匠鋪|Ferraria de DogYo
DY_Weapon|道幽武器店|Loja de armas de DogYo
DY_Wearhouse|道幽倉庫|Armazém de DogYo
DYMine|道幽礦洞|Mina de DogYo
DYMine4Boss|道幽礦洞四層首領室|Mina de DogYo · chefe do 4º andar
DogYoMineLift|道幽礦井升降機|Elevador da mina de DogYo
`);

exactNames.push(...rows(`
SmashPendulum|破擊鐘擺|Pêndulo de impacto
BoneDecapitator|斬骨刀|Decapitador de ossos
BlackDragonSlayer|黑龍屠刀|Matador de dragões negros
DragonSlayer|屠龍刀|Matador de dragões
Trident|三叉戟|Tridente
Scimitar|彎刀|Cimitarra
Stiletto|細刃匕首|Estilete
TheRipper|撕裂者|O Estripador
BracerOfMagic|魔法護腕|Braçadeira de magia
SpiritReformer|靈魂重塑者|Reformador de espíritos
ConvexLens|凸透鏡|Lente convexa
BambooPipe|竹管|Tubo de bambu
DemonicBells|魔鈴|Sinos demoníacos
SorceryAnchor|巫術之錨|Âncora de feitiçaria
PurifiedMirror|淨化之鏡|Espelho purificado
EvilTriangle|邪惡三角|Triângulo maligno
CrossPurified|淨化十字|Cruz purificada
KunroonTear|崑崙之淚|Lágrima de Kunroon
GoldenTiara|金冠|Tiara dourada
DemonShadow|惡魔之影|Sombra demoníaca
FlamingGrasp|火焰之握|Abraço flamejante
(HP)DrugSmall|小型生命藥水|Poção de vida pequena
(MP)DrugSmall|小型魔力藥水|Poção de mana pequena
(HP)DrugMedium|中型生命藥水|Poção de vida média
(MP)DrugMedium|中型魔力藥水|Poção de mana média
(HP)DrugLarge|大型生命藥水|Poção de vida grande
(MP)DrugLarge|大型魔力藥水|Poção de mana grande
(HP)DrugXL|特大型生命藥水|Poção de vida extragrande
(MP)DrugXL|特大型魔力藥水|Poção de mana extragrande
(HP)PotionSmall|小型生命藥劑|Elixir de vida pequeno
(MP)PotionSmall|小型魔力藥劑|Elixir de mana pequeno
(HP)PotionMedium|中型生命藥劑|Elixir de vida médio
(MP)PotionMedium|中型魔力藥劑|Elixir de mana médio
SunPotion|太陽藥水|Poção solar
Ginseng|人參|Ginseng
OldGinseng|老山參|Ginseng antigo
Apple|蘋果|Maçã
GreenPoison|綠色毒粉|Pó venenoso verde
RedPoison|紅色毒粉|Pó venenoso vermelho
Amulet(Bundle)|護身符包|Pacote de amuletos
Invitation|邀請函|Convite
GTTeleport|公會領地傳送卷|Pergaminho do território da guilda
RandomTeleport|隨機傳送卷|Pergaminho de teleporte aleatório
DungeonEscape|地牢逃脫卷|Pergaminho de fuga do calabouço
TownTeleport|回城卷|Pergaminho de retorno
TeleportHome|家園傳送卷|Pergaminho de retorno ao lar
Candle|蠟燭|Vela
EnternalFlame|永恆之火|Chama eterna
PeddlerTorch|商販火把|Tocha do mascate
Lantern|燈籠|Lanterna
SewingGoods|縫紉用品|Material de costura
GreaterSewingGoods|高級縫紉用品|Material de costura superior
GreaterBoneHammer|高級骨錘|Martelo de osso superior
BengalTiger|孟加拉虎|Tigre-de-bengala
BlueTiger|藍虎|Tigre azul
BlackLeopard|黑豹|Leopardo negro
IceLeopard|冰豹|Leopardo do gelo
PantheraTiger|猛虎|Tigre Panthera
RedTiger|紅虎|Tigre vermelho
WhiteTiger|白虎|Tigre branco
BrownWolf|棕狼|Lobo marrom
BlueWolf|藍狼|Lobo azul
BlackWolf|黑狼|Lobo negro
WhiteWolf|白狼|Lobo branco
LeanMeat|瘦肉|Carne magra
FishDetector|探魚器|Detector de peixes
Walleye|玻璃梭鱸|Lucioperca
SwordFish|旗魚|Peixe-espada
RockBass|岩鱸|Robalo-de-rocha
Trout|鱒魚|Truta
Tinker|丁鱥|Tenca
SmallGobby|小蝦虎魚|Góbio pequeno
Mackeral|鯖魚|Cavala
Gobby|蝦虎魚|Góbio
ArmourBook(DC)|盔甲秘笈（攻擊）|Livro de armadura (ataque físico)
ArmourBook(MC)|盔甲秘笈（魔法）|Livro de armadura (magia)
ArmourBook(SC)|盔甲秘笈（道術）|Livro de armadura (tao)
ArmourBook(ACC)|盔甲秘笈（準確）|Livro de armadura (precisão)
RustyArmour|生鏽盔甲|Armadura enferrujada
Translucent|半透明結晶|Cristal translúcido
HavocCrystal|浩劫水晶|Cristal da devastação
BookofMana|魔力之書|Livro de mana
BookofSpirit|靈魂之書|Livro de espírito
ArmourCastTool|盔甲鑄造工具|Ferramenta de fundição de armaduras
MossyBox|苔蘚箱|Caixa musgosa
StrongRope|強韌繩索|Corda resistente
Amethyst|紫水晶|Ametista
Ruby|紅寶石|Rubi
Platinum|白金|Platina
Nephrite|翡翠|Nefrita
PickAxe|鶴嘴鋤|Picareta
Stamp|印章|Selo
GoldBar|金條|Barra de ouro
GoldBarBundle|金條包|Pacote de barras de ouro
GambleChip|賭場籌碼|Ficha de aposta
Dice|骰子|Dado
TimeStonePiece|時間石碎片|Fragmento da pedra do tempo
Chicken|雞肉|Carne de frango
Pork|豬肉|Carne suína
Beef|牛肉|Carne bovina
Venison|鹿肉|Carne de cervo
Mutton|羊肉|Carne de carneiro
Feather|羽毛|Pena
Cherry|櫻桃|Cereja
Mushroom|蘑菇|Cogumelo
Timber|木材|Madeira
CannibalLeaf|食人花葉|Folha de planta carnívora
CannibalLeaves|食人花葉|Folhas de planta carnívora
CannibalStem|食人花莖|Caule de planta carnívora
CannibalSeed|食人花種子|Semente de planta carnívora
CannibalPoison|食人花毒液|Veneno de planta carnívora
SpiderWeb|蜘蛛網|Teia de aranha
ZumaRelic|祖瑪遺物|Relíquia de Zuma
ScorpionTail|蠍尾|Cauda de escorpião
Mandible|顎骨|Mandíbula
String|細繩|Barbante
PigEar|豬耳|Orelha de porco
PigHoof|豬蹄|Pé de porco
RedMoonChip|赤月碎片|Fragmento da lua vermelha
AwakeningSoul|覺醒之魂|Alma do despertar
CreatureMirror|寵物改名鏡|Espelho de mascote
CreatureNuts|寵物堅果|Nozes de mascote
FairyMoss|仙靈苔蘚|Musgo de fada
FreshwaterClam|淡水蛤|Amêijoa de água doce
Mackerel|鯖魚|Cavala
Cherries|櫻桃|Cerejas
WonderDrug(EXP)|奇蹟藥劑（經驗）|Remédio maravilhoso (experiência)
WonderDrug(DROP)|奇蹟藥劑（掉落）|Remédio maravilhoso (saque)
WonderDrug(HP)|奇蹟藥劑（生命）|Remédio maravilhoso (vida)
WonderDrug(MP)|奇蹟藥劑（魔力）|Remédio maravilhoso (mana)
WonderDrug(AC)|奇蹟藥劑（防禦）|Remédio maravilhoso (defesa física)
WonderDrug(MAC)|奇蹟藥劑（魔防）|Remédio maravilhoso (defesa mágica)
WonderDrug(A.SPEED)|奇蹟藥劑（攻速）|Remédio maravilhoso (velocidade de ataque)
Knapsack|背袋|Mochila
DollyMask|娃娃面具|Máscara de boneca
Pirate|海盜裝|Traje de pirata
Ninja|忍者裝|Traje de ninja
Santa|聖誕老人裝|Traje de Papai Noel
Rudolph|魯道夫裝|Traje de Rudolph
FlamingMutant|火焰變異者裝|Traje de mutante flamejante
Formal|禮服|Traje de gala
Footballer|足球員裝|Traje de jogador de futebol
DevilGirl|惡魔女孩裝|Traje de garota demoníaca
HellWarrior|地獄戰士裝|Traje de guerreiro infernal
HeavenWarrior|天界戰士裝|Traje de guerreiro celestial
Ram|公羊裝|Traje de carneiro
PoisonSack|毒囊|Bolsa de veneno
Web|蜘蛛網|Teia
SecretRecipe|祕方|Receita secreta
SnakeBody|蛇身|Corpo de serpente
Ebony(Fruit)|烏木果|Fruto de ébano
OliviasRing|Olivia 的戒指|Anel de Olivia
Relics|遺物|Relíquias
CookBook|食譜|Livro de receitas
CanniTea|食人花茶|Chá carnívoro
BatWings|蝙蝠翅膀|Asas de morcego
SkeletonHead|骷髏頭|Cabeça de esqueleto
Antidote|解毒劑|Antídoto
GatheringTool|採集工具|Ferramenta de coleta
GreenHerb|綠色草藥|Erva verde
StolenGold|失竊金幣|Ouro roubado
AncientTree|古木|Árvore ancestral
BeefRib|牛肋骨|Costela bovina
BugSpecimen|昆蟲標本|Espécime de inseto
BichonTales|比奇傳說|Contos de Bichon
CorpsFlower|屍花|Flor-cadáver
PrajnaHistory|般若史書|História de Prajna
BatPoison|蝙蝠毒液|Veneno de morcego
Algae|藻類|Algas
RelicRock|遺跡石|Rocha das relíquias
OldLoafer|舊便鞋|Mocassim antigo
SharpTrident|鋒利三叉戟|Tridente afiado
SharpScimitar|鋒利彎刀|Cimitarra afiada
WornBeadofPhoenix|磨損鳳凰珠|Conta da fênix desgastada
EyeofGoldenSnake|金蛇眼|Olho da serpente dourada
SpearWithHook|鉤矛|Lança com gancho
BlackIronCrowSwords|黑鐵鴉雙劍|Espadas do corvo de ferro negro
FireConvexLens|火焰凸透鏡|Lente convexa de fogo
StrongBambooFlute|強化竹笛|Flauta de bambu reforçada
ResurrectionScroll|復活卷軸|Pergaminho de ressurreição
PKPointReduction|罪惡值減免卷|Redutor de pontos de PK
MysteriousScroll|神祕卷軸|Pergaminho misterioso
GuardRental|衛兵僱用卷|Contrato de guarda
AncientBanga[Green]|遠古號角（綠）|Corneta ancestral (verde)
AncientBanga[Purple]|遠古號角（紫）|Corneta ancestral (roxa)
SexChange|性別變更卷|Pergaminho de mudança de gênero
ExtraDrop20%|掉落提升 20%|Bônus de saque de 20%
ElixirOfPhysical|物防靈藥|Elixir de defesa física
AncientScyther|遠古鐮刀|Ceifador ancestral
Medicine|藥品|Remédio
BoxofHolyWater|聖水寶箱|Caixa de água sagrada
ElixirOfMight|力量靈藥|Elixir de poder
ElixirOfDefensive|防禦靈藥|Elixir de defesa
ElixirOfGinseng|人參靈藥|Elixir de ginseng
CraftingBook|製作手冊|Livro de fabricação
CostumeGirl|少女時裝|Traje de garota
BlackTiger|黑虎裝|Traje de tigre negro
Bossy|首領時裝|Traje autoritário
GymnasticsQueen|體操女王裝|Traje de rainha da ginástica
OldFisherman|老漁夫裝|Traje de velho pescador
SkiKid|滑雪少年裝|Traje de jovem esquiador
Sportsman|運動員裝|Traje de esportista
CatTognue|貓舌術|Língua de Gato
GtInvite|公會領地邀請函|Convite ao território da guilda
UnstablePearll|不穩定珍珠|Pérola instável
UnstablePearl|不穩定珍珠|Pérola instável
Unstable Pearll|不穩定珍珠|Pérola instável
HyeoncheonSuhoSteel|玄天守護鋼|Aço protetor de Hyeoncheon
Hyeoncheon Maseok|玄天魔石|Pedra mágica de Hyeoncheon
Rabbit|兔裝|Traje de coelho
Carnation|康乃馨|Cravo
BlessCarnation|祝福康乃馨|Cravo abençoado
DoubleFlower|雙花|Flor dupla
GonryunDoubleFlower|崑崙雙花|Flor dupla de Gonryun
Redrecoverywater|紅色恢復藥水|Água de recuperação vermelha
Bluerecoverywater|藍色恢復藥水|Água de recuperação azul
The essence of Doom|末日精華|Essência da perdição
BallOfDoom|末日寶珠|Orbe da perdição
TheFairyRing|仙靈戒指|Anel da fada
StoneMonster|石怪|Monstro de pedra
`));

for (const name of ["GonryunHePublication", "GonryunEunhyung", "GonryunSanggwan", "GonryunYeoseonru", "Gonryunpasackle", "GonryunYeongokhwan", "Gonryunyongdrama"]) properNames.add(name);
