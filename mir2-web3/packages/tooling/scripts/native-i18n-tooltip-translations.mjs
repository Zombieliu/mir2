import { rows } from "./native-i18n-translations.mjs";
export const normalizeProse = (text) => text.replace(/\s+/g, " ").trim();
const descriptions = rows(`
wont|未提供有效說明。|Descrição não fornecida.
Instantly heals player.|立即恢復角色生命。|Restaura a vida do personagem instantaneamente.
Repairs equipped weapons durability to maximum.|將已裝備武器的耐久度修復至最大值。|Restaura ao máximo a durabilidade da arma equipada.
Used in certain Taoist spells to give red poison (reduces defences)|用於部分道士技能，施加紅毒以降低防禦。|Usado em certas habilidades taoístas para aplicar veneno vermelho, que reduz as defesas.
Required amulet for most Taoist spells.|多數道士技能所需的護身符。|Amuleto necessário para a maioria das habilidades taoístas.
Bundle which contains total of 3000 Amulets|內含共 3000 個護身符。|Pacote com um total de 3000 amuletos.
Required amulet for the Taoists resurrection spell.|道士復活技能所需的護身符。|Amuleto necessário para a habilidade de ressurreição do taoísta.
Strange scroll that can take you back to your Guild's Home|能將你傳送回公會家園的神奇卷軸。|Pergaminho que leva você de volta ao lar da sua guilda.
Teleports player to their last saved safezone.|將角色傳送至最近保存的安全區。|Teletransporta o personagem para a última zona segura registrada.
Premium Fishing Rod purchasable from the GameShop. -Duration 6 Months|可於商城購買的高級魚竿。有效時間：6 個月。|Vara de pesca premium da loja do jogo. Duração: 6 meses.
Required for each upgrade.|每次升級所需的材料。|Material necessário para cada melhoria.
Click this egg to obtain a Creature. A Creature can pickup your items for you while you can continue the fight.|點擊此卵可獲得寵物。寵物能在你繼續戰鬥時幫忙拾取物品。|Clique neste ovo para obter um mascote. Ele recolhe seus itens enquanto você continua lutando.
With this mirror you can rename a creature.|使用此鏡可為寵物改名。|Use este espelho para renomear um mascote.
Special creatures produce these rare stones every 3 hours. Breaking this stone will grant you a random item.|特殊寵物每 3 小時產出這種稀有石。打碎後可獲得隨機物品。|Mascotes especiais produzem estas pedras raras a cada 3 horas. Quebre a pedra para receber um item aleatório.
Maintain food level for 10 hours.|維持飽食度 10 小時。|Mantém a saciedade por 10 horas.
When a creature's feeding reaches 0|當寵物的飽食度降至 0 時。|Quando a saciedade de um mascote chega a 0.
Open this box to receive a random wonder. WonderStrength is based on BoxStrength.|開啟此盒可獲得隨機奇蹟效果，效果強度取決於盒子的等級。|Abra a caixa para receber um efeito maravilhoso aleatório. A intensidade depende do nível da caixa.
Hitting Accuracy is increased per spell level.|命中準確度隨技能等級提升。|A precisão dos golpes aumenta com o nível da habilidade.
Your weapon becomes infused with strength|為武器灌注力量。|Infunde força na sua arma.
Increases the distance of your attack|增加攻擊距離。|Aumenta o alcance do ataque.
Curves the swing of the weapon allowing you to hit up to 4 targets at once.|以弧形揮動武器，一次最多命中 4 個目標。|Desfere um golpe em arco, atingindo até 4 alvos de uma vez.
Charge at your target pushing them back|向目標衝撞並將其推退。|Avança contra o alvo e o empurra para trás.
The weapon performs 2 swings in one motion. Target has a chance of becoming stunned allowing more damage.|一次動作揮擊 2 次，有機率使目標暈眩並受到更多傷害。|Executa 2 golpes em um movimento, com chance de atordoar o alvo e causar mais dano.
Paralyses mobs and pulls them towards the caster.|麻痺怪物並將其拉向施法者。|Paralisa monstros e os puxa até o conjurador.
Summons the spirit of fire causing a devastating blow to the target.|召喚火焰之靈，對目標造成猛烈一擊。|Invoca o espírito do fogo para desferir um golpe devastador no alvo.
The warrior lets out a bellowing roar|戰士發出震耳怒吼。|O Guerreiro solta um rugido ensurdecedor.
Perform 2 curving swings with their weapon allowing them to hit up to 8 targets at once.|連續施展 2 次弧形揮擊，一次最多命中 8 個目標。|Executa 2 golpes em arco, atingindo até 8 alvos de uma vez.
Performs a thrust like action in 3 directions.|向 3 個方向施展突刺。|Executa uma estocada em 3 direções.
Warrior enters a hardened state of mind where any physical damage taken is reduced.|戰士進入堅毅狀態，降低受到的物理傷害。|O Guerreiro entra em um estado de firmeza que reduz o dano físico recebido.
Warrior enters a enraged state of mind where each weapon swing hits harder.|戰士進入狂怒狀態，每次武器攻擊更強。|O Guerreiro entra em fúria e cada golpe de arma causa mais dano.
Improves all defenses and has a chance to reflect damage to the attacker.|提升所有防禦，並有機率將傷害反射給攻擊者。|Melhora todas as defesas e pode refletir dano ao atacante.
Warrior charges into his foes parting them like waves while dealing additional damage.|戰士衝入敵群，擊退敵人並造成額外傷害。|O Guerreiro avança entre os inimigos, abrindo caminho e causando dano adicional.
Increases the warriors attack speed for a set period of time.|在一段時間內提升戰士的攻擊速度。|Aumenta a velocidade de ataque do Guerreiro por um período.
Long distance attack by hurling a fireball|投擲火球進行遠距離攻擊。|Lança uma bola de fogo para atacar à distância.
Pushes away monsters and people surrounding you.|推開周圍的怪物和角色。|Empurra monstros e personagens ao redor.
Stuns and confuses monsters. Can also be used to tame monsters.|使怪物暈眩、混亂，也可用於馴服怪物。|Atordoa e confunde monstros. Também pode ser usado para domá-los.
A powerful version of the Fireball spell.|威力更強的火球術。|Uma versão mais poderosa da Bola de Fogo.
Throws a column of flame 5 squares straight ahead of the caster.|向施法者前方直線 5 格釋放火柱。|Lança uma coluna de chamas por 5 casas à frente do conjurador.
Calls a thunderbolt to attack target from the sky.|召喚雷電，從天空攻擊目標。|Invoca um raio do céu para atacar o alvo.
Spell that moves you randomly within a province and used to escape dungeons.|在省內隨機傳送，也可用於逃離地牢。|Teletransporta aleatoriamente dentro da província e permite escapar de calabouços.
Creates an explosion of fire in 3x3 square causing damage in its range.|在 3x3 格範圍製造火焰爆炸並造成傷害。|Cria uma explosão de fogo que causa dano em uma área de 3x3 casas.
Creates a wall of fire on the ground.|在地面製造火牆。|Cria uma muralha de fogo no chão.
The caster creates a bolt of lightning|施法者釋放一道閃電。|O conjurador cria um raio.
Casts a ball of ice which damages the target with a chance of impairing their movement speed or freezing them.|發射冰球造成傷害，並有機率降低目標移速或將其冰凍。|Lança uma esfera de gelo que causa dano e pode reduzir a velocidade do alvo ou congelá-lo.
A spell that creates a thunder storm around the caster causing a shock on monsters into a state of confusion.|在施法者周圍製造雷暴，電擊怪物並使其混亂。|Cria uma tempestade de raios ao redor do conjurador, eletrocutando e confundindo monstros.
Creates a protective field around caster that absorbs any damage taken.|在施法者周圍形成吸收傷害的防護場。|Cria um campo protetor que absorve o dano recebido pelo conjurador.
Chance to kill any undead mob that meets the level requirements in a single cast.|有機率一次消滅符合等級限制的亡靈怪物。|Pode eliminar com uma conjuração um monstro morto-vivo que cumpra o requisito de nível.
Draws health from the victim to replenish the casters own health.|吸取目標生命，恢復施法者的生命。|Drena a vida da vítima para restaurar a do conjurador.
Casts a powerful ice storm that circles the enemy causing damage on rotation.|釋放強力冰風暴，環繞敵人並持續造成傷害。|Cria uma poderosa tempestade de gelo que gira ao redor do inimigo, causando dano.
Casts a powerful column of fire that burns its target.|釋放強力火柱灼燒目標。|Lança uma poderosa coluna de fogo que queima o alvo.
Summons an indentical clone of the caster which will aid with attacks.|召喚與施法者相同的分身協助攻擊。|Invoca um clone idêntico ao conjurador para ajudar nos ataques.
Creates a huge burst of fire damaging anything in its range.|製造巨大的火焰爆發，傷害範圍內的目標。|Cria uma grande explosão de fogo que causa dano a tudo em seu alcance.
The caster will summon a storm from the sky which drops shards of ice upon the target in a 5x5 range.|召喚冰暴，向目標周圍 5x5 格降下冰片。|Invoca uma tempestade que derruba fragmentos de gelo numa área de 5x5 casas ao redor do alvo.
Summons a magical eye allowing the caster to see his enemies weakness's.|召喚魔眼，讓施法者看見敵人的弱點。|Invoca um olho mágico que revela as fraquezas dos inimigos.
Caster summons a meteor storm from the sky which drops molten rock on the enemies in a 5x5 range.|召喚流星風暴，向 5x5 格內的敵人降下熔岩。|Invoca uma tempestade de meteoros que lança rochas fundidas sobre inimigos numa área de 5x5 casas.
Applies a heal over time effect on yourself or other friendly targets.|對自己或友方目標施加持續治療。|Aplica cura gradual em você ou em um alvo aliado.
Infuses your weapon with the power of spirit|將精神力量灌注於武器。|Infunde poder espiritual na sua arma.
Casts harmful poison onto target.|對目標施加毒素。|Aplica veneno nocivo ao alvo.
Casts a flaming amulet at the target inflicting fire damage.|向目標投出燃燒的護身符，造成火焰傷害。|Lança um amuleto em chamas contra o alvo, causando dano de fogo.
Summons a skeleton that will aid your attacks.|召喚骷髏協助攻擊。|Invoca um esqueleto para ajudar nos ataques.
Hide into the shadows making it difficult for mobs to see you.|隱入陰影，使怪物難以發現你。|Oculta você nas sombras, dificultando a detecção por monstros.
Cast in 3x3 square all those effected hide into the shadows making it difficult for mobs to see them.|使 3x3 格範圍內的目標隱入陰影，難以被怪物發現。|Oculta nas sombras os alvos em uma área de 3x3 casas, dificultando a detecção por monstros.
Casts a magical glyph into the sky|向天空施放魔法符文。|Lança um glifo mágico no céu.
Reveals the targets health above their head.|在目標頭頂顯示生命值。|Mostra a vida do alvo acima de sua cabeça.
Creates a wave of energy that repulses foes away from you.|釋放能量波，將敵人推離。|Cria uma onda de energia que afasta os inimigos.
Traps multiple mobs inside a hexagon prism making them immobile.|將多個怪物困在六角結界中，使其無法移動。|Aprisiona vários monstros em um prisma hexagonal, impedindo seu movimento.
Removes harmful effects from self or friendly targets such as poison|移除自己或友方目標的中毒等負面效果。|Remove efeitos nocivos, como veneno, de você ou de alvos aliados.
Applies a heal over time effect to yourself or friendly targets in a 3x3 range.|對 3x3 格範圍內的自己或友方目標施加持續治療。|Aplica cura gradual em você ou em aliados numa área de 3x3 casas.
Confuses mobs and enslaves them to fight for you for a limited time.|迷惑怪物，使其在一段時間內為你作戰。|Confunde monstros e os faz lutar por você por um tempo limitado.
Increases each class's primary offensive stat.|提升各職業的主要攻擊屬性。|Aumenta o atributo ofensivo principal de cada classe.
Summons a fire breathing beast which will aid your attacks.|召喚噴火神獸協助攻擊。|Invoca uma fera que cospe fogo para ajudar nos ataques.
The taoist can bring back a dead player into the living realm.|道士能使死亡的角色復活。|Permite ao Taoísta ressuscitar um personagem morto.
Summons an ancient spirit of thunder to aid their attacks.|召喚遠古雷霆之靈協助攻擊。|Invoca um espírito ancestral do trovão para ajudar nos ataques.
Casts a variation of poisons and debuff affects in a 3x3 square area. Changes affects depending on poison equipped.|在 3x3 格範圍施加毒素與負面效果，效果依裝備的毒粉而變。|Aplica venenos e efeitos negativos em 3x3 casas. Os efeitos dependem do veneno equipado.
Casts a toxic poison cloud in a 3x3 square area|在 3x3 格範圍製造劇毒雲霧。|Cria uma nuvem venenosa em uma área de 3x3 casas.
Castable on self and friendly targets|可對自己與友方目標施放。|Pode ser usado em você e em alvos aliados.
Increases pets defence and attack power for a duration of time.|在一段時間內提升寵物的防禦與攻擊。|Aumenta a defesa e o ataque dos mascotes por um período.
Increases attack damage. Passive Skill.|被動技能，提升攻擊傷害。|Habilidade passiva que aumenta o dano de ataque.
Jack Sparrow|Jack Sparrow|Jack Sparrow
Male Assassin|男性刺客|Assassino masculino
Female Assassin|女性刺客|Assassina feminina
Santa Claus with Candy cane.|持拐杖糖的聖誕老人。|Papai Noel com bengala doce.
Santa's best friend|聖誕老人的摯友。|O melhor amigo do Papai Noel.
Spider(Scythe)|蜘蛛（鐮刀）|Aranha (foice)
Spider(BarbarianScythe)|蜘蛛（野蠻鐮刀）|Aranha (foice bárbara)
RedOverAlls(M)|紅色連身衣（男）|Macacão vermelho (masculino)
Red/Yellow(F)|紅黃服裝（女）|Traje vermelho e amarelo (feminino)
Blue(M)|藍色服裝（男）|Traje azul (masculino)
Red/White(F)|紅白服裝（女）|Traje vermelho e branco (feminino)
Football Red|紅色足球裝|Uniforme de futebol vermelho
White Dress RedTrident(F)|白裙紅三叉戟（女）|Vestido branco e tridente vermelho (feminino)
Black Dress Fan|黑裙摺扇|Vestido preto e leque
Black Sword(M)|黑劍裝（男）|Traje com espada negra (masculino)
Gold Armour + Scythe(M)|金甲鐮刀（男）|Armadura dourada e foice (masculino)
Ice King|冰王|Rei do gelo
Football Player Black uniform|足球員黑色制服|Uniforme de futebol preto
Football Player Red and Blue uniform|足球員紅藍制服|Uniforme de futebol vermelho e azul
Football Player White Blue stripe uniform|足球員白藍條紋制服|Uniforme de futebol branco com listras azuis
Football Player White uniform|足球員白色制服|Uniforme de futebol branco
Football Player Pink and Blue uniform|足球員粉藍制服|Uniforme de futebol rosa e azul
WhiteDress Blue/Red Trident|白裙藍紅三叉戟|Vestido branco e tridente azul e vermelho
Angry Sheep|憤怒公羊|Carneiro furioso
Formal Male|男性禮服|Traje de gala masculino
Scroll which revives you from death. Ressurection HP 100% Ressurection MP 100%|使死亡角色復活的卷軸。復活生命：100%；復活魔力：100%。|Pergaminho que ressuscita o personagem com 100% de vida e 100% de mana.
Scroll which reduces your PK Score. -PK Point reduced by 100 -Useage Once.|降低罪惡值的卷軸。罪惡值減少 100，使用一次。|Pergaminho que reduz os pontos de PK em 100. Uso único.
A mysterious scroll with locations of all the towns. -Useage Once.|記錄所有城鎮位置的神祕卷軸，使用一次。|Pergaminho misterioso com a localização de todas as cidades. Uso único.
A mysterious scroll with locations of all the dungeon entrances. -Useage Once.|記錄所有地牢入口位置的神祕卷軸，使用一次。|Pergaminho misterioso com a localização de todas as entradas de calabouços. Uso único.
A Scroll which allow you to Summon One Guard. -Summons One Guard. -Duration 7 Days. -Command @GuardsHelp|可召喚一名衛兵的卷軸。持續 7 天。指令：@GuardsHelp|Pergaminho que invoca um guarda por 7 dias. Comando: @GuardsHelp
Ancient Horn which ables you to shout across a location. -Shout in Green across a location. -Useage Once.|可在區域內喊話的遠古號角。綠色區域喊話，使用一次。|Corneta ancestral para anunciar na região em verde. Uso único.
Ancient Horn which ables you to shout across the nation. -Shout in Purple across the nation. -Useage Once.|可向全國喊話的遠古號角。紫色全國喊話，使用一次。|Corneta ancestral para anunciar em toda a nação em roxo. Uso único.
Mysterious Scroll. Which changes your Gender. -Useage Once.|改變角色性別的神祕卷軸，使用一次。|Pergaminho misterioso que altera o gênero do personagem. Uso único.
Mysterious Stone. Which Gives you additional Experience. -Additional Experience 30% -None Stackable -Command @MysteriousStoneON -Command @MysteriousStoneOFF|提供額外 30% 經驗的神祕石，不可疊加。指令：@MysteriousStoneON、@MysteriousStoneOFF|Pedra misteriosa que concede 30% de experiência adicional. Não cumulativo. Comandos: @MysteriousStoneON e @MysteriousStoneOFF
Improvement of your Physical Defence for a certain period of time. -Physical Defence + 7 -Duration 1 Day -None Stackable|物理防禦提高 7，持續 1 天，不可疊加。|Aumenta a defesa física em 7 por 1 dia. Não cumulativo.
Improvement of your Accuracy for a certain period of time. -Duration 1 Hour -None Stackable|提升準確度，持續 1 小時，不可疊加。|Aumenta a precisão por 1 hora. Não cumulativo.
Medicine which reduces your PK Point by 10.|降低 10 點罪惡值的藥品。|Remédio que reduz os pontos de PK em 10.
Invite players to your GT.|邀請玩家進入你的公會領地。|Convide jogadores para o território da sua guilda.
In the Kunlun region, it seems to have lost its strength|在崑崙地區，似乎已失去力量。|Parece ter perdido sua força na região de Kunlun.
Additional Drop Rate acquired for a period of time. -Additional Drop Increase by 20% -Duration 1 Day -None Stackable|掉落率額外提高 20%，持續 1 天，不可疊加。|Aumenta a taxa de saque em 20% por 1 dia. Não cumulativo.
Physical Hunting Package consists of. -Experience + 30% -MAXHP + 180 -Destruction + 9 -DCTorch -A.Speed + 8 -Resurrection Scroll *ALL ARE 30 MINS*|物理狩獵禮包：經驗 +30%、最大生命 +180、攻擊力 +9、攻擊火把、攻速 +8、復活卷軸。全部持續 30 分鐘。|Pacote de caça física: experiência +30%, vida máxima +180, ataque físico +9, tocha de ataque físico, velocidade de ataque +8 e pergaminho de ressurreição. Tudo dura 30 minutos.
Magical Hunting Package consists of. -Experience + 30% -MAXHP + 180 -Magical + 7 -MCTorch -Resurrection Scroll *ALL ARE 30 MINS*|魔法狩獵禮包：經驗 +30%、最大生命 +180、魔法力 +7、魔法火把、復活卷軸。全部持續 30 分鐘。|Pacote de caça mágica: experiência +30%, vida máxima +180, magia +7, tocha de magia e pergaminho de ressurreição. Tudo dura 30 minutos.
Soul Hunting Package consists of. -Experience + 30% -MAXHP + 180 -Soul + 7 -SCTorch -Resurrection Scroll *ALL ARE 30 MINS*|道術狩獵禮包：經驗 +30%、最大生命 +180、道術 +7、道術火把、復活卷軸。全部持續 30 分鐘。|Pacote de caça taoísta: experiência +30%, vida máxima +180, tao +7, tocha de tao e pergaminho de ressurreição. Tudo dura 30 minutos.
Quest reward item. A mysterious box with the contents consist of: -ElixirOfMight[1] -ElixirOfDefensive[1] -ElixirOfGinseng[1] -EXP20%[1] -Duration 1 Day|任務獎勵寶箱，內含力量靈藥、防禦靈藥、人參靈藥和 20% 經驗加成各 1 份，持續 1 天。|Caixa de recompensa de missão: 1 elixir de poder, 1 elixir de defesa, 1 elixir de ginseng e 1 bônus de 20% de experiência. Duração: 1 dia.
A Mysterious potion contained from a BoxofHolyWater -Destruction + 7 -Magical + 5 -Soul + 5 -Duration 1 Day -None Stackable|來自聖水寶箱的神祕藥水。攻擊力 +7、魔法力 +5、道術 +5，持續 1 天，不可疊加。|Poção misteriosa da caixa de água sagrada. Ataque físico +7, magia +5 e tao +5 por 1 dia. Não cumulativo.
A Mysterious potion contained from a BoxofHolyWater -Physical Defence + 7 -Magical Defence + 5 -Duration 1 Day -None Stackable|來自聖水寶箱的神祕藥水。物理防禦 +7、魔法防禦 +5，持續 1 天，不可疊加。|Poção misteriosa da caixa de água sagrada. Defesa física +7 e defesa mágica +5 por 1 dia. Não cumulativo.
A Mysterious potion contained from a BoxofHolyWater -Max HP + 50 -Max MP + 70 -Duration 1 Day -None Stackable|來自聖水寶箱的神祕藥水。最大生命 +50、最大魔力 +70，持續 1 天，不可疊加。|Poção misteriosa da caixa de água sagrada. Vida máxima +50 e mana máxima +70 por 1 dia. Não cumulativo.
Premium Fishing Package. Purchased from the GameShop package consists of: :RedFishingRod (6 Months) :PremiumReel :PremiumFloat :PremiumFinder :PremiumBait x7200|商城高級釣魚禮包：紅色魚竿（6 個月）、高級捲線器、高級浮標、高級探魚器、高級魚餌 x7200。|Pacote de pesca premium da loja: vara vermelha (6 meses), carretilha premium, boia premium, localizador premium e isca premium x7200.
OldManFishing|老漁夫|Velho pescador
Sportman|運動員|Esportista
Shoes that allow you to walk quickly as if you have wings, as if you are walking on air without touching the ground. \\ \\When combining each set effect item with a holy relic + divine relic grade, the holy relic set effect is applied.|讓你行走如飛、彷彿踏空而行的鞋子。套裝效果物品混搭聖物與神物等級時，套用聖物套裝效果。|Sapatos que permitem andar depressa, como se tivesse asas e caminhasse no ar. Ao combinar itens de conjunto de relíquia sagrada e divina, aplica-se o efeito do conjunto de relíquia sagrada.
`);
const table = new Map(descriptions.map(([en, tw, pt]) => [normalizeProse(en), { "zh-TW": tw, "pt-BR": pt }]));
const statNames = {
  Destruction: ["攻擊力", "ataque físico"], Magic: ["魔法力", "magia"], "Soul Magic": ["道術", "tao"], "your Health": ["生命", "vida"], "your Mana": ["魔力", "mana"], "your Attack Speed": ["攻擊速度", "velocidade de ataque"],
  Experience: ["經驗", "experiência"], DropRate: ["掉落率", "taxa de saque"], MaxHP: ["最大生命", "vida máxima"], MaxMP: ["最大魔力", "mana máxima"], AC: ["物理防禦", "defesa física"], AMC: ["魔法防禦", "defesa mágica"], BagWeight: ["背包負重", "capacidade de carga"],
};
export function tooltipCopy(source) {
  const text = normalizeProse(source), direct = table.get(text);
  if (direct) return { en: source, ...direct, method: text === "wont" ? "invalid-authored-placeholder" : "reviewed-item-description" };
  let match;
  if ((match = text.match(/^You can give this to your creature\. (25|50|75|100)%$/))) return { en: source, "zh-TW": `可餵給寵物。${match[1]}%`, "pt-BR": `Pode ser dado ao mascote. ${match[1]}%`, method: "reviewed-item-description-template" };
  if ((match = text.match(/^Increase (Experience|DropRate|MaxHP|MaxMP|BagWeight) by (\d+)(%?) for (\d+) hours?\.$/))) {
    const [, stat, amount, percent, hours] = match, [tw, pt] = statNames[stat];
    return { en: source, "zh-TW": `${tw}提升 ${amount}${percent}，持續 ${hours} 小時。`, "pt-BR": `Aumenta ${pt} em ${amount}${percent} por ${hours} ${hours === "1" ? "hora" : "horas"}.`, method: "reviewed-item-description-template" };
  }
  if ((match = text.match(/^Increase (AC|AMC) 1-1 for 1 hour\.$/))) return { en: source, "zh-TW": `${statNames[match[1]][0]}增加 1-1，持續 1 小時。`, "pt-BR": `Aumenta ${statNames[match[1]][1]} em 1-1 por 1 hora.`, method: "reviewed-item-description-template" };
  if (text === "Increase Speed for 30m.") return { en: source, "zh-TW": "提升速度，持續 30 分鐘。", "pt-BR": "Aumenta a velocidade por 30 minutos.", method: "reviewed-item-description" };
  if ((match = text.match(/^Improvement of (Destruction|Magic|Soul Magic|your Health|your Mana|your Attack Speed) for a certain period of time\. -Duratiuon (1 Hour|3 Hours|5 Hours|30 Minuets) -None Stackable$/))) {
    const [tw, pt] = statNames[match[1]], mins = match[2] === "30 Minuets", n = match[2].split(" ")[0];
    return { en: source, "zh-TW": `提升${tw}，持續 ${n} ${mins ? "分鐘" : "小時"}，不可疊加。`, "pt-BR": `Aumenta ${pt} por ${n} ${mins ? "minutos" : n === "1" ? "hora" : "horas"}. Não cumulativo.`, method: "reviewed-item-description-template" };
  }
  if ((match = text.match(/^Premium Torch purchased from the GameShop\. -(Destruction|Magical|Soul) Power \+(\d+) -Duratiuon (\d+) (Hours|Minutes)$/))) {
    const stat = { Destruction: ["攻擊力", "ataque físico"], Magical: ["魔法力", "magia"], Soul: ["道術", "tao"] }[match[1]];
    return { en: source, "zh-TW": `商城高級火把。${stat[0]} +${match[2]}，持續 ${match[3]} ${match[4] === "Hours" ? "小時" : "分鐘"}。`, "pt-BR": `Tocha premium da loja do jogo. ${stat[1]} +${match[2]} por ${match[3]} ${match[4] === "Hours" ? "horas" : "minutos"}.`, method: "reviewed-item-description-template" };
  }
  if ((match = text.match(/^Additional Experience acquired for a period of time\. -Additional Experience (\d+)% -Duration (\d+) (Hour|Hours|Day) -None Stackable$/))) return { en: source, "zh-TW": `額外經驗 +${match[1]}%，持續 ${match[2]} ${match[3] === "Day" ? "天" : "小時"}，不可疊加。`, "pt-BR": `Experiência adicional +${match[1]}% por ${match[2]} ${match[3] === "Day" ? "dia" : match[2] === "1" ? "hora" : "horas"}. Não cumulativo.`, method: "reviewed-item-description-template" };
  if ((match = text.match(/^Limited Item\. Potion which gives the following enhancements\. -Duration (\d+) Hours -None Stackable$/))) return { en: source, "zh-TW": `限定物品。提供強化效果的藥水，持續 ${match[1]} 小時，不可疊加。`, "pt-BR": `Item limitado. Poção que concede melhorias por ${match[1]} horas. Não cumulativo.`, method: "reviewed-item-description-template" };
  if ((match = text.match(/^-Allows Entry to the Premium Dungeon -Duration (\d+) Days?$/))) return { en: source, "zh-TW": `可進入特權地牢，持續 ${match[1]} 天。`, "pt-BR": `Permite entrar no calabouço premium por ${match[1]} ${match[1] === "1" ? "dia" : "dias"}.`, method: "reviewed-item-description-template" };
  return null;
}
