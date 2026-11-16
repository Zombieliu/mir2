// Reviewed NPC prose. This generator never reads or changes a live account.
// Every source entry is accounted for; --check fails while any translation is missing.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const sourcePath = path.join(root, 'packages/tooling/data/native-i18n/npc-prose-source.json');
const outputPath = path.join(root, 'packages/game-data/data/native-i18n/npc-prose.json');
// BEGIN REVIEWED PROSE
const authored = {
  "npc.prose.0111971ffa4acf77": {
    "en": "Hello {npc_arg0}, Are you a beginner in this game?",
    "zh-TW": "你好，{npc_arg0}，你是這個遊戲的新手嗎？",
    "pt-BR": "Olá, {npc_arg0}. Você está começando neste jogo?"
  },
  "npc.prose.0117b777858abf21": {
    "en": "RedValley would be an alternative choice at this level.",
    "zh-TW": "這個等級也可以選擇前往赤月峽谷。",
    "pt-BR": "O Vale da Lua Vermelha também seria uma opção neste nível."
  },
  "npc.prose.0155891d301c8eb1": {
    "en": "Speak with Emperor Far, he is crazy though but also lazy... I would take",
    "zh-TW": "去找法爾皇帝談談吧，他又瘋又懶……我會帶上",
    "pt-BR": "Fale com o Imperador Far. Ele é meio louco e preguiçoso... Eu levaria"
  },
  "npc.prose.01a954c5c79f5e08": {
    "en": "Have you had enough and want to go back?",
    "zh-TW": "玩夠了，想回去了嗎？",
    "pt-BR": "Já teve o bastante e quer voltar?"
  },
  "npc.prose.01ff509814e38c0f": {
    "en": "and strength.",
    "zh-TW": "與力量。",
    "pt-BR": "e força."
  },
  "npc.prose.021d7bbda1aa98a7": {
    "en": "It has a low chance of making the enemy stunned for a while.",
    "zh-TW": "有較低機率使敵人暫時暈眩。",
    "pt-BR": "Tem uma pequena chance de atordoar o inimigo por um tempo."
  },
  "npc.prose.0223f44e8ac9cbc4": {
    "en": "I will summon the members scattered in the Mir continent now.",
    "zh-TW": "現在我會召集散落在瑪法大陸各處的成員。",
    "pt-BR": "Vou convocar agora os membros espalhados pelo continente de Mir."
  },
  "npc.prose.0248219e989b19f8": {
    "en": "Helmets.",
    "zh-TW": "頭盔。",
    "pt-BR": "Elmos."
  },
  "npc.prose.0262474073be0ed8": {
    "en": "You have no GoldBar for me to Exchange...",
    "zh-TW": "你沒有可供兌換的金條……",
    "pt-BR": "Você não tem uma Barra de ouro para trocar..."
  },
  "npc.prose.028013ce81d88504": {
    "en": "Store.",
    "zh-TW": "存放。",
    "pt-BR": "Guardar."
  },
  "npc.prose.02890563bf3e4959": {
    "en": "You know what the best wine is? The best wine is the snakeWine.",
    "zh-TW": "你知道什麼酒最好嗎？最好的就是蛇酒。",
    "pt-BR": "Sabe qual é o melhor vinho? É o vinho de cobra."
  },
  "npc.prose.02ccb755d4086e99": {
    "en": "the target. Can only be cast every 80 seconds.",
    "zh-TW": "目標。每 80 秒只能施放一次。",
    "pt-BR": "o alvo. Só pode ser usada a cada 80 segundos."
  },
  "npc.prose.02d94c5404333698": {
    "en": "Time Limit is 2 hours. I hope you will pass all stages and",
    "zh-TW": "時限是 2 小時。希望你能通過所有關卡，並且",
    "pt-BR": "O limite é de 2 horas. Espero que você passe por todas as etapas e"
  },
  "npc.prose.02dcc589253b0377": {
    "en": "know every kinds of demon and evil for it is holy and reverend.",
    "zh-TW": "辨識各種妖魔，因為它神聖而莊嚴。",
    "pt-BR": "reconhecer todo tipo de demônio e mal, pois é sagrado e venerável."
  },
  "npc.prose.0328c2a81b358730": {
    "en": "Increase attack accuracy.",
    "zh-TW": "提高攻擊準確度。",
    "pt-BR": "Aumenta a precisão dos ataques."
  },
  "npc.prose.03415c02acbdde77": {
    "en": "I see you're wearing: {npc_arg0}",
    "zh-TW": "我看見你正穿著：{npc_arg0}",
    "pt-BR": "Vejo que você está usando: {npc_arg0}"
  },
  "npc.prose.0359ad20dc7c92ed": {
    "en": "You do not have Main Rebirth Effect.",
    "zh-TW": "你沒有主要轉生效果。",
    "pt-BR": "Você não possui o efeito principal de renascimento."
  },
  "npc.prose.038ff7e6b65e25a0": {
    "en": "It is my job to guide newbies.",
    "zh-TW": "引導新手是我的工作。",
    "pt-BR": "Meu trabalho é orientar os iniciantes."
  },
  "npc.prose.03a0d2f15bcac8bb": {
    "en": "was a very strong demon in Redmoon valley and that it seemed to be",
    "zh-TW": "是赤月峽谷中非常強大的惡魔，而且似乎",
    "pt-BR": "era um demônio muito poderoso no Vale da Lua Vermelha e parecia"
  },
  "npc.prose.0442e34486b0637e": {
    "en": "A Dungeonescape scroll is a magic paper that moves you promptly",
    "zh-TW": "地牢逃脫卷是一張魔法卷軸，能立即將你傳送",
    "pt-BR": "Um Pergaminho de fuga do calabouço é um papel mágico que transporta você imediatamente"
  },
  "npc.prose.04630b9dcd01b320": {
    "en": "How can I be of assistance.",
    "zh-TW": "有什麼能為你效勞的？",
    "pt-BR": "Como posso ajudar?"
  },
  "npc.prose.04bb13f99943a488": {
    "en": "Skill training will increase the chance of landing near towns.",
    "zh-TW": "提升技能熟練度會增加落在城鎮附近的機率。",
    "pt-BR": "Treinar a habilidade aumenta a chance de chegar perto de uma cidade."
  },
  "npc.prose.04cb8c8297ae6673": {
    "en": "What a strange pillar.",
    "zh-TW": "真是奇怪的柱子。",
    "pt-BR": "Que pilar estranho."
  },
  "npc.prose.04dbba81e8bfedae": {
    "en": "I want to  my guild territory.",
    "zh-TW": "我想處理我的行會領地。",
    "pt-BR": "Quero cuidar do território da minha guilda."
  },
  "npc.prose.051d086012e742ad": {
    "en": "Assassin:",
    "zh-TW": "刺客：",
    "pt-BR": "Assassino:"
  },
  "npc.prose.0553c860e0e1ce83": {
    "en": "it will be disappeared and success message will appear.",
    "zh-TW": "它就會消失，並顯示成功訊息。",
    "pt-BR": "ele desaparecerá e uma mensagem de sucesso será exibida."
  },
  "npc.prose.05687ff60f4b9d2b": {
    "en": "of TimeStone. This piece of stone will help you to",
    "zh-TW": "的時間石。這塊石頭能幫助你",
    "pt-BR": "da Pedra do tempo. Este fragmento ajudará você a"
  },
  "npc.prose.0595111d69b4e1a2": {
    "en": "//////////////////////////////////////////////////////////////////// Ghoul Cave",
    "zh-TW": "//////////////////////////////////////////////////////////////////// 食屍鬼洞穴",
    "pt-BR": "//////////////////////////////////////////////////////////////////// Caverna dos Ghouls"
  },
  "npc.prose.05bac41c8ec30447": {
    "en": "I'm the Cave researcher in this area.",
    "zh-TW": "我是這一帶的洞穴研究員。",
    "pt-BR": "Sou o pesquisador de cavernas desta região."
  },
  "npc.prose.05c12ba2d6eb5214": {
    "en": "guild information and etc.",
    "zh-TW": "行會資訊等等。",
    "pt-BR": "informações da guilda e assim por diante."
  },
  "npc.prose.05cba78f7aa1bb61": {
    "en": "Infinite Bout?",
    "zh-TW": "無限比武？",
    "pt-BR": "Combate infinito?"
  },
  "npc.prose.05e8f66d8a8176f7": {
    "en": "Welcome to my house..",
    "zh-TW": "歡迎來到我家……",
    "pt-BR": "Bem-vindo à minha casa..."
  },
  "npc.prose.0647e34bf95d0af0": {
    "en": "Hello traveler.. I have been hearing good things about",
    "zh-TW": "你好，旅人……我聽到了不少關於你的好消息",
    "pt-BR": "Olá, viajante... Tenho ouvido coisas boas sobre você"
  },
  "npc.prose.070bb8127e74cfd9": {
    "en": "Then I will teleport you to the territory of you guild.",
    "zh-TW": "那麼，我會把你傳送到你行會的領地。",
    "pt-BR": "Então vou transportar você ao território da sua guilda."
  },
  "npc.prose.07b88bd7b3debb2f": {
    "en": "Province. A party is recommended since soloing here is very difficult.",
    "zh-TW": "省。建議組隊，獨自在這裡冒險很困難。",
    "pt-BR": "Província. Recomendo um grupo, pois enfrentar esta região sozinho é muito difícil."
  },
  "npc.prose.07faa947163d2da2": {
    "en": "Hello there {npc_arg0},",
    "zh-TW": "你好，{npc_arg0}，",
    "pt-BR": "Olá, {npc_arg0},"
  },
  "npc.prose.081394ec3972cda8": {
    "en": "so there is nothing left but its legend...",
    "zh-TW": "因此如今只剩下它的傳說……",
    "pt-BR": "por isso, só restou a lenda..."
  },
  "npc.prose.0839dfaa5145dad0": {
    "en": "OldManInfinity.",
    "zh-TW": "無極老人。",
    "pt-BR": "Sábio do Infinito."
  },
  "npc.prose.085c6e01f0d38d71": {
    "en": "\"The still dont remain still for \"",
    "zh-TW": "「靜止之物並非永遠靜止……」",
    "pt-BR": "“O que está imóvel nem sempre permanece assim...”"
  },
  "npc.prose.089c2079186228ae": {
    "en": "Ask me anytime, I will be glad to guide you.",
    "zh-TW": "隨時問我吧，我很樂意為你指路。",
    "pt-BR": "Pergunte quando quiser. Terei prazer em orientar você."
  },
  "npc.prose.08a68fe628fa8ce3": {
    "en": "A RepairOil makes the durabillity of you hand weapon rise,",
    "zh-TW": "修理油能恢復手中武器的耐久度，",
    "pt-BR": "O Óleo de Reparo restaura a durabilidade da arma em sua mão,"
  },
  "npc.prose.08ce961802856268": {
    "en": "transferred to the buyer 24 hours later.",
    "zh-TW": "在 24 小時後轉交給買家。",
    "pt-BR": "transferido ao comprador 24 horas depois."
  },
  "npc.prose.08f696dc465711f6": {
    "en": "The Evil within are great. But the rewards can be greater.",
    "zh-TW": "裡面的邪惡非常強大，但獎勵可能更加豐厚。",
    "pt-BR": "O mal lá dentro é poderoso, mas as recompensas podem ser ainda maiores."
  },
  "npc.prose.090d666cee7a7f07": {
    "en": "Executes long-distance attacks like a wizards fireball skill,",
    "zh-TW": "能發動遠距離攻擊，就像法師的火球術，",
    "pt-BR": "Executa ataques à distância, como a Bola de Fogo dos magos,"
  },
  "npc.prose.096b2cbe994f795e": {
    "en": "Rebirth 3 cleared.",
    "zh-TW": "已完成第三次轉生。",
    "pt-BR": "Terceiro renascimento concluído."
  },
  "npc.prose.096d0b159645e186": {
    "en": "A DungeonEscape scroll is a magic paper that moves you promptly",
    "zh-TW": "地牢逃脫卷是一張魔法卷軸，能立即將你傳送",
    "pt-BR": "Um Pergaminho de fuga do calabouço é um papel mágico que transporta você imediatamente"
  },
  "npc.prose.09fecd334990cfc3": {
    "en": "You can get 3000 Gold just by collecting and",
    "zh-TW": "只要收集並交出物品，就能得到 3000 金幣",
    "pt-BR": "Você pode ganhar 3000 de ouro apenas coletando e"
  },
  "npc.prose.0a125b1dd230a047": {
    "en": "No MP will be consumed.",
    "zh-TW": "不消耗魔法值。",
    "pt-BR": "Não consome MP."
  },
  "npc.prose.0a2d8143b90cc0ac": {
    "en": "Skill key must be clicked. MP will be consumed.",
    "zh-TW": "必須按下技能鍵，並且會消耗魔法值。",
    "pt-BR": "É preciso pressionar a tecla da habilidade. Ela consome MP."
  },
  "npc.prose.0a4c54ddb720a7aa": {
    "en": "\"The noise rumbles the faint whispers are \"",
    "zh-TW": "「轟鳴聲中，微弱的低語……」",
    "pt-BR": "“O estrondo ressoa, e os sussurros fracos...”"
  },
  "npc.prose.0a9ffdb60d37e0bd": {
    "en": "for I had a bad reputation at that time. So they expelled me from there",
    "zh-TW": "因為我當時名聲很差，所以他們把我趕了出去",
    "pt-BR": "pois eu tinha má fama na época. Então me expulsaram de lá"
  },
  "npc.prose.0aa87e17791a77d8": {
    "en": "\"Only parties weilding the forbidden Orb may pass here.\"",
    "zh-TW": "「只有持有禁忌寶珠的隊伍才能通過。」",
    "pt-BR": "“Somente grupos que carregam o Orbe Proibido podem passar.”"
  },
  "npc.prose.0ac88e0150d023d7": {
    "en": "If you want to extend, talk to the 'GT Steward' in the territory.",
    "zh-TW": "若要延長期限，請找領地中的「行會領地管家」。",
    "pt-BR": "Para prorrogar, fale com o Administrador do Território da Guilda dentro do território."
  },
  "npc.prose.0adafaa9c709390b": {
    "en": "skill is lost.... however....",
    "zh-TW": "技能失傳了……不過……",
    "pt-BR": "a habilidade se perdeu... porém..."
  },
  "npc.prose.0affd1e3cfb2e87e": {
    "en": "The monsters attempt to break the seal endlessly.",
    "zh-TW": "怪物不停地試圖打破封印。",
    "pt-BR": "Os monstros tentam quebrar o selo sem parar."
  },
  "npc.prose.0b0e232392ac0cb4": {
    "en": "TO BE ADDED.",
    "zh-TW": "尚待加入。",
    "pt-BR": "A SER ADICIONADO."
  },
  "npc.prose.0b4881961ed28d83": {
    "en": "here for a long time.",
    "zh-TW": "在這裡待了很久。",
    "pt-BR": "aqui há muito tempo."
  },
  "npc.prose.0b51f400ef9d925b": {
    "en": "TownTeleport scrolls but it could still save your liefe.",
    "zh-TW": "回城卷，但它仍可能救你一命。",
    "pt-BR": "Pergaminhos de retorno, mas ainda pode salvar sua vida."
  },
  "npc.prose.0b585804bca5269c": {
    "en": "and you can return to a village promptly by using it...",
    "zh-TW": "使用它便能立刻回到村莊……",
    "pt-BR": "e usá-lo permite voltar rapidamente a uma vila..."
  },
  "npc.prose.0b8b0ecd7bb75cb1": {
    "en": "to the guild master who owns the territory. And if you wish to",
    "zh-TW": "給擁有領地的行會會長。如果你想要",
    "pt-BR": "ao líder da guilda dona do território. E, se você quiser"
  },
  "npc.prose.0b991574c1f44367": {
    "en": "I'm the Leader of this Town.. Our wall's are strong to keep",
    "zh-TW": "我是這座城鎮的首領……我們堅固的城牆能抵禦",
    "pt-BR": "Sou o líder desta cidade... Nossas muralhas são fortes para deter"
  },
  "npc.prose.0bb687e0a7801a71": {
    "en": "Hey! Jude, don't make it bad... Well customer get here..",
    "zh-TW": "喂！朱德，別把事情弄糟……客人，這邊請……",
    "pt-BR": "Ei, Jude, não estrague tudo... Bem, cliente, venha aqui..."
  },
  "npc.prose.0bd05627fe87fb2e": {
    "en": "Unfortunately you lost, better luck next time.",
    "zh-TW": "很遺憾，你輸了。祝你下次好運。",
    "pt-BR": "Infelizmente você perdeu. Melhor sorte na próxima vez."
  },
  "npc.prose.0bf2cf97b8d78132": {
    "en": "Mount Equipment",
    "zh-TW": "坐騎裝備",
    "pt-BR": "Equipamento de montaria"
  },
  "npc.prose.0c0c28eeebb6cf72": {
    "en": "I will not help an evil person like you...",
    "zh-TW": "我不會幫助你這種惡人……",
    "pt-BR": "Não vou ajudar uma pessoa malvada como você..."
  },
  "npc.prose.0c14c79d609ea427": {
    "en": "I would suggest you leave before you spend some time behind bars.",
    "zh-TW": "建議你在被關進牢房之前離開。",
    "pt-BR": "Sugiro que vá embora antes de passar um tempo atrás das grades."
  },
  "npc.prose.0c3187f3f7d452d4": {
    "en": "I want to show you the way, but I'm exhausted",
    "zh-TW": "我想為你指路，但我已精疲力盡",
    "pt-BR": "Eu gostaria de mostrar o caminho, mas estou exausto"
  },
  "npc.prose.0c8178463b84b538": {
    "en": "may he live on within our community.",
    "zh-TW": "願他永遠活在我們心中。",
    "pt-BR": "que ele continue vivo na memória da nossa comunidade."
  },
  "npc.prose.0c8c6d1c9b6d0fa9": {
    "en": "If you would like a Bichon Token you can purchase one from me.",
    "zh-TW": "如果想要比奇代幣，可以向我購買。",
    "pt-BR": "Se quiser uma Ficha de Bichon, pode comprá-la comigo."
  },
  "npc.prose.0cc41383ec47f1c1": {
    "en": "3. Commission period: After 100 days of item sale registration,",
    "zh-TW": "3. 寄售期限：物品登記出售滿 100 天後，",
    "pt-BR": "3. Prazo da consignação: após 100 dias do registro da venda do item,"
  },
  "npc.prose.0d633adb6423b113": {
    "en": "arrogant for I took a pride in my martial art, consequently it made",
    "zh-TW": "因為自負於武藝而變得傲慢，結果使我",
    "pt-BR": "arrogante por me orgulhar da minha arte marcial; por consequência, isso me fez"
  },
  "npc.prose.0dab1657a89c10c0": {
    "en": "You Dont Have enough Gold to use my Service!",
    "zh-TW": "你的金幣不夠支付我的服務費！",
    "pt-BR": "Você não tem ouro suficiente para usar meu serviço!"
  },
  "npc.prose.0db4b92ac31c127e": {
    "en": "Bichon Province, Mongchon Province, Tao Village, Prajna",
    "zh-TW": "比奇省、盟重省、道士村、般若",
    "pt-BR": "Província de Bichon, Província de Mongchon, Vila Taoísta, Prajna"
  },
  "npc.prose.0df881775841bc23": {
    "en": "for the Armed Transporter at the entrance of the",
    "zh-TW": "尋找入口處的武裝運送員",
    "pt-BR": "procure o Transportador Armado na entrada do"
  },
  "npc.prose.0dfc146e6c0b49bf": {
    "en": "Bichon-Province, WoomyonWoods, Mongchon-Province, TaoVillage,",
    "zh-TW": "比奇省、沃瑪森林、盟重省、道士村，",
    "pt-BR": "Província de Bichon, Floresta de Wooma, Província de Mongchon, Vila Taoísta,"
  },
  "npc.prose.0e6f6e060b631c31": {
    "en": "Right Wall",
    "zh-TW": "右側城牆",
    "pt-BR": "Muralha direita"
  },
  "npc.prose.0e887c375d194d0a": {
    "en": "Fish.",
    "zh-TW": "魚。",
    "pt-BR": "Peixes."
  },
  "npc.prose.0e9102fa2730f254": {
    "en": "Click at dead a body of those (Alt+left click) and acquire meat after slicing action.",
    "zh-TW": "對著牠們的屍體按 Alt 加滑鼠左鍵，切割後便可取得肉。",
    "pt-BR": "Clique no corpo morto com Alt + botão esquerdo para obter carne após a ação de corte."
  },
  "npc.prose.0eb7ecb5599b0a50": {
    "en": "5x5 square range attack with a dreadful flame. (Max. 24 enemies at once)",
    "zh-TW": "以猛烈火焰攻擊 5×5 方格範圍。（最多同時攻擊 24 個敵人）",
    "pt-BR": "Ataca uma área de 5×5 casas com chamas terríveis. (Máximo de 24 inimigos por vez.)"
  },
  "npc.prose.0eb83add13c81916": {
    "en": "awkward and uncomfortable - but it could also",
    "zh-TW": "尷尬又不舒服，但也可能",
    "pt-BR": "estranho e desconfortável, mas também poderia"
  },
  "npc.prose.0ef4119a7df98024": {
    "en": "my spirit from grasp of the huge demonic power and I aquired a",
    "zh-TW": "我的靈魂掙脫了巨大魔力的束縛，而我獲得了",
    "pt-BR": "meu espírito das garras daquele enorme poder demoníaco, e adquiri um"
  },
  "npc.prose.0f4ee998d4178d35": {
    "en": "Ring.",
    "zh-TW": "戒指。",
    "pt-BR": "Anéis."
  },
  "npc.prose.0f5296832e01dbf1": {
    "en": "Please visit Jessica inside the Inn",
    "zh-TW": "請去客棧內找潔西卡",
    "pt-BR": "Visite Jessica dentro da estalagem."
  },
  "npc.prose.0f6eca25c68da254": {
    "en": "I am Perry, the Rev.Taoist of this school. A young hero like you",
    "zh-TW": "我是本門的道長佩里。像你這樣的年輕英雄",
    "pt-BR": "Sou Perry, o reverendo taoísta desta escola. Um jovem herói como você"
  },
  "npc.prose.0f86fc6b891c20b4": {
    "en": "TO BE ADDED",
    "zh-TW": "尚待加入",
    "pt-BR": "A ser adicionado"
  },
  "npc.prose.0f8cafdbdc1121d1": {
    "en": "Hello traveler, What can i do for you?",
    "zh-TW": "旅人，你好。有什麼能為你效勞？",
    "pt-BR": "Olá, viajante. Como posso ajudar?"
  },
  "npc.prose.0fb9aef4b62d82d5": {
    "en": "I want to  members to the territory.",
    "zh-TW": "我想將成員帶到領地。",
    "pt-BR": "Quero levar membros ao território."
  },
  "npc.prose.10059c4ab4d1a2d7": {
    "en": "hit twice and steal HP.",
    "zh-TW": "攻擊兩次並吸取生命。",
    "pt-BR": "atingir duas vezes e roubar PV."
  },
  "npc.prose.10075d86d45fb710": {
    "en": "Greetings {npc_arg0}, I am {npc_arg1} i travel all across",
    "zh-TW": "你好，{npc_arg0}。我是 {npc_arg1}，我遊歷各地",
    "pt-BR": "Saudações, {npc_arg0}. Sou {npc_arg1} e viajo por toda"
  },
  "npc.prose.102a729311b62628": {
    "en": "Hello Mirian, I'm currently protecting the Entrance to the Swamp.",
    "zh-TW": "你好，瑪法人。我正守護著沼澤入口。",
    "pt-BR": "Olá, habitante de Mir. Estou protegendo a entrada do Pântano."
  },
  "npc.prose.109d1f2a7951ab9e": {
    "en": "HALT! You may not pass.",
    "zh-TW": "站住！你不能通過。",
    "pt-BR": "Alto! Você não pode passar."
  },
  "npc.prose.11105eda1b7888ed": {
    "en": "I can't explain more..",
    "zh-TW": "我不能再多說了……",
    "pt-BR": "Não posso explicar mais..."
  },
  "npc.prose.1156753af7d7dde3": {
    "en": "Allows a wizard to move instantly to a random place in a Marian Province.",
    "zh-TW": "讓法師瞬間移動到瑪法省內的隨機位置。",
    "pt-BR": "Permite ao mago se mover instantaneamente para um local aleatório de uma província de Mir."
  },
  "npc.prose.11db2d5f173b7199": {
    "en": "Quick Entry to:",
    "zh-TW": "快速進入：",
    "pt-BR": "Entrada rápida para:"
  },
  "npc.prose.12379136e92c072e": {
    "en": "Rebirth 2 cleared.",
    "zh-TW": "已清除第二次轉生。",
    "pt-BR": "Segundo renascimento removido."
  },
  "npc.prose.125899c7bc92343b": {
    "en": "Actually, due to the number of guilds and struggles between",
    "zh-TW": "其實，由於行會眾多，彼此間又紛爭不斷",
    "pt-BR": "Na verdade, devido ao número de guildas e às disputas entre"
  },
  "npc.prose.12c5604f516ac5b3": {
    "en": "You can learn a special skill from Level 7.",
    "zh-TW": "從 7 級開始，你可以學習特殊技能。",
    "pt-BR": "Você pode aprender uma habilidade especial a partir do nível 7."
  },
  "npc.prose.134f24c5d8781b5d": {
    "en": "damage done. Chance to gain Vampire buff which enhances",
    "zh-TW": "造成的傷害。有機率獲得吸血增益，強化",
    "pt-BR": "o dano causado. Há uma chance de obter o efeito Vampiro, que melhora"
  },
  "npc.prose.134fe05f4a41a925": {
    "en": "I can upgrade weapons for you for a small fee. To make an Upgrade you",
    "zh-TW": "只需支付少許費用，我就能為你升級武器。要升級，你",
    "pt-BR": "Posso melhorar suas armas por uma pequena taxa. Para isso, você"
  },
  "npc.prose.13b409b71ce5bc77": {
    "en": "server develops and adds in more functionality over time.",
    "zh-TW": "伺服器會隨著開發進度逐步加入更多功能。",
    "pt-BR": "o servidor evolui e recebe novas funcionalidades com o tempo."
  },
  "npc.prose.13c209ed6ac189d3": {
    "en": "3) Open skill screen(F11), activate learned skill.",
    "zh-TW": "3）開啟技能畫面（F11），啟用已學會的技能。",
    "pt-BR": "3) Abra a tela de habilidades (F11) e ative uma habilidade aprendida."
  },
  "npc.prose.14705a87f42af02f": {
    "en": "Hello, Do you want to play a game?",
    "zh-TW": "你好，想玩個遊戲嗎？",
    "pt-BR": "Olá, quer jogar um jogo?"
  },
  "npc.prose.1486fa7e4813d307": {
    "en": "Next Conquest Date:",
    "zh-TW": "下次攻城日期：",
    "pt-BR": "Data da próxima conquista:"
  },
  "npc.prose.150d92c40cfd0d1e": {
    "en": "To Do",
    "zh-TW": "待辦事項",
    "pt-BR": "A fazer"
  },
  "npc.prose.15bb869d0a2a05f0": {
    "en": "Congratulations {npc_arg0},",
    "zh-TW": "恭喜你，{npc_arg0}，",
    "pt-BR": "Parabéns, {npc_arg0},"
  },
  "npc.prose.15f4911b4796859f": {
    "en": "PrajnaIsland is the best hunting area at the Level of 30 to 40.",
    "zh-TW": "般若島是 30 至 40 級最適合的狩獵區。",
    "pt-BR": "A Ilha de Prajna é a melhor área de caça para os níveis 30 a 40."
  },
  "npc.prose.162d659790a5e16b": {
    "en": "The chronicle or specification for the HolySword vanished since",
    "zh-TW": "關於聖劍的記載與資料已經消失，自從",
    "pt-BR": "As crônicas e os detalhes da Espada Sagrada desapareceram desde"
  },
  "npc.prose.1669d9cf91f52d8c": {
    "en": "butcher in any town, you can earn money in return.",
    "zh-TW": "任何城鎮的屠夫，就能換取金錢。",
    "pt-BR": "a um açougueiro de qualquer cidade para ganhar dinheiro."
  },
  "npc.prose.16ad80615744df1f": {
    "en": "And go AngledStoneTomb or Treepath at Level 30 to 35.",
    "zh-TW": "30 至 35 級時，可以前往石墓或林間小徑。",
    "pt-BR": "Dos níveis 30 a 35, vá à Tumba de Pedra ou à Trilha das Árvores."
  },
  "npc.prose.1712928fd27f8402": {
    "en": "I tried to use anger against the RedMoonEvil, the terrible monster.",
    "zh-TW": "我曾試圖以憤怒對抗赤月惡魔，那可怕的怪物。",
    "pt-BR": "Tentei usar minha raiva contra o Demônio da Lua Vermelha, aquele monstro terrível."
  },
  "npc.prose.171f792c9464fb97": {
    "en": "The first thing you need to learn is sword technique,",
    "zh-TW": "首先，你需要學習劍術，",
    "pt-BR": "A primeira coisa que você precisa aprender é a técnica da espada,"
  },
  "npc.prose.17de7f24bd298082": {
    "en": "tastes the best.",
    "zh-TW": "味道最好。",
    "pt-BR": "tem o melhor sabor."
  },
  "npc.prose.1818cdfd14a70c59": {
    "en": "Fire a leaching arrow that can heal the archer based on",
    "zh-TW": "射出吸血箭，依據下列數值恢復弓箭手生命：",
    "pt-BR": "Dispara uma flecha drenante que pode curar o arqueiro com base em"
  },
  "npc.prose.1859ac5c0ebbc64a": {
    "en": "In spite of the passage of time, RedMoonEvil has not fully",
    "zh-TW": "儘管時間流逝，赤月惡魔仍未完全",
    "pt-BR": "Apesar do tempo que passou, o Demônio da Lua Vermelha ainda não"
  },
  "npc.prose.187e0584adf7daea": {
    "en": "Have thorough preparation, OK?",
    "zh-TW": "一定要做好充分準備，好嗎？",
    "pt-BR": "Prepare-se bem, está bem?"
  },
  "npc.prose.1899872983ac4e2b": {
    "en": "I sincerely ask you to cooperate with the authorities",
    "zh-TW": "我誠摯地請求你配合當局",
    "pt-BR": "Peço sinceramente que coopere com as autoridades"
  },
  "npc.prose.18be71aff13b97e1": {
    "en": "Thunderbolt (Level 17)",
    "zh-TW": "雷電術（17 級）",
    "pt-BR": "Raio (nível 17)"
  },
  "npc.prose.18d25f00a0605a3e": {
    "en": "*Meat store  *Lottery  *Bookstore  *Warehouse",
    "zh-TW": "＊肉店　＊彩票　＊書店　＊倉庫",
    "pt-BR": "*Açougue  *Loteria  *Livraria  *Armazém"
  },
  "npc.prose.18eeb3848a4d798c": {
    "en": "It is recommended to go DeathValley, WoomaTemple,",
    "zh-TW": "建議前往死亡山谷、沃瑪寺廟，",
    "pt-BR": "Recomendo ir ao Vale da Morte, ao Templo de Wooma,"
  },
  "npc.prose.19210ffd0d58c477": {
    "en": "He was also enquiring about an ancient language.",
    "zh-TW": "他也在打聽一種古代語言。",
    "pt-BR": "Ele também estava investigando uma língua antiga."
  },
  "npc.prose.194318b50cd30076": {
    "en": "Now I am goign to tell you what you want.",
    "zh-TW": "現在，我會告訴你想知道的事。",
    "pt-BR": "Agora vou lhe contar o que deseja saber."
  },
  "npc.prose.1955a1046c900e9f": {
    "en": "the item will be automatically deleted from the list.",
    "zh-TW": "物品將自動從清單中刪除。",
    "pt-BR": "o item será removido da lista automaticamente."
  },
  "npc.prose.197906dd8d97b5c7": {
    "en": "It's my job to guide newbies.",
    "zh-TW": "引導新手是我的工作。",
    "pt-BR": "Meu trabalho é orientar iniciantes."
  },
  "npc.prose.199cbb9f62a54c74": {
    "en": "but if the meat is stained with soil or burned with fire",
    "zh-TW": "但如果肉沾了泥土或被火燒焦",
    "pt-BR": "mas, se a carne estiver suja de terra ou queimada,"
  },
  "npc.prose.19a0d5b2e1901804": {
    "en": "You currently have Rebirth 1.",
    "zh-TW": "你目前已完成第一次轉生。",
    "pt-BR": "Você possui atualmente o primeiro renascimento."
  },
  "npc.prose.19aabd317140d0ad": {
    "en": "for Repairs and Archers.",
    "zh-TW": "用於修理，也適合弓箭手。",
    "pt-BR": "para reparos e arqueiros."
  },
  "npc.prose.19b0dd51cb4f0d9f": {
    "en": "normal condition).",
    "zh-TW": "正常狀態）。",
    "pt-BR": "em condições normais)."
  },
  "npc.prose.19e61668c286aa02": {
    "en": "- (Quest Reward Item).",
    "zh-TW": "－（任務獎勵物品）。",
    "pt-BR": "— (Item de recompensa de missão)."
  },
  "npc.prose.1a1beb5c3fb7809d": {
    "en": "Please select the Recipe you either want to Buy or Sell.",
    "zh-TW": "請選擇你想購買或出售的配方。",
    "pt-BR": "Selecione a receita que deseja comprar ou vender."
  },
  "npc.prose.1a49a5fde5542ecc": {
    "en": "all shared by the five merchants.",
    "zh-TW": "由五位商人共同共享。",
    "pt-BR": "compartilhados entre os cinco comerciantes."
  },
  "npc.prose.1a61813b658d4aba": {
    "en": "I wonder if you can beat them all...",
    "zh-TW": "不知道你能不能打敗它們所有人……",
    "pt-BR": "Será que você consegue derrotar todos eles...?"
  },
  "npc.prose.1ab0767e32dcd23a": {
    "en": "Ah those symbols... Theres not much information about the Language.",
    "zh-TW": "啊，那些符號……關於這種語言的資料不多。",
    "pt-BR": "Ah, aqueles símbolos... Há pouca informação sobre essa língua."
  },
  "npc.prose.1b0b297bcf14bbbb": {
    "en": "What a good attitude you take!",
    "zh-TW": "你的態度真好！",
    "pt-BR": "Que boa atitude!"
  },
  "npc.prose.1b1c9d481bf47ed8": {
    "en": "I want to  to my guild territory.",
    "zh-TW": "我想前往我的行會領地。",
    "pt-BR": "Quero ir ao território da minha guilda."
  },
  "npc.prose.1b55d8f6038efcc0": {
    "en": "Gather more elements for more damage.",
    "zh-TW": "收集更多元素，提升傷害。",
    "pt-BR": "Reúna mais elementos para causar mais dano."
  },
  "npc.prose.1b5d212678e07eee": {
    "en": "Pay 12,000 Gold and Board.",
    "zh-TW": "支付 12,000 金幣並登船。",
    "pt-BR": "Pague 12.000 moedas de ouro e embarque."
  },
  "npc.prose.1b63b5773b38c69e": {
    "en": "You can repair various kinds of Jewellery.",
    "zh-TW": "你可以修理各種首飾。",
    "pt-BR": "Você pode reparar vários tipos de joias."
  },
  "npc.prose.1b9d6158de4f4e18": {
    "en": "Assassins are members of a secret organization and their history is relatively unknown.",
    "zh-TW": "刺客隸屬於祕密組織，他們的歷史鮮為人知。",
    "pt-BR": "Assassinos pertencem a uma organização secreta, e pouco se sabe sobre sua história."
  },
  "npc.prose.1bcb127f6b51285e": {
    "en": "The power of Dungeonescape scroll is weak compared with",
    "zh-TW": "地牢逃脫卷的力量較弱，相較於",
    "pt-BR": "O poder do pergaminho de fuga do calabouço é fraco comparado ao"
  },
  "npc.prose.1c4893519cc89b6f": {
    "en": "4. Number of commission items: Maximum 30 items are allowed",
    "zh-TW": "4. 寄售數量：最多可寄售 30 件物品",
    "pt-BR": "4. Quantidade em consignação: são permitidos no máximo 30 itens."
  },
  "npc.prose.1c5bcaeddb9b945d": {
    "en": "with evil energy are able to kill the monster RedMoonEvil is a",
    "zh-TW": "帶著邪惡能量，能殺死赤月惡魔，這是",
    "pt-BR": "com energia maligna podem matar o Demônio da Lua Vermelha é uma"
  },
  "npc.prose.1c78519f99f77b90": {
    "en": "There are 20 stages altogether.",
    "zh-TW": "總共有 20 個關卡。",
    "pt-BR": "Há 20 etapas ao todo."
  },
  "npc.prose.1c7d11ec98043f79": {
    "en": "You need to be above Level 70 to rebirth.",
    "zh-TW": "你必須超過 70 級才能轉生。",
    "pt-BR": "Você precisa estar acima do nível 70 para renascer."
  },
  "npc.prose.1cc9fc9f16d48698": {
    "en": "You dont have the 3000 Gold entrance fee.",
    "zh-TW": "你沒有足夠的 3,000 金幣入場費。",
    "pt-BR": "Você não tem as 3.000 moedas de ouro da entrada."
  },
  "npc.prose.1ce7c6034a0e636f": {
    "en": "Iceman was a kind and loving gentleman that will be missed by hundreds,",
    "zh-TW": "Iceman 是一位善良有愛的紳士，將被數百人懷念，",
    "pt-BR": "Iceman era um homem gentil e amoroso, de quem centenas de pessoas sentirão falta,"
  },
  "npc.prose.1d1ef78a8646570a": {
    "en": "*Item root: Left click to root the item in ground. If inventory is full or heavy",
    "zh-TW": "＊撿取物品：左鍵點擊地上的物品。若背包已滿或負重過高",
    "pt-BR": "*Coletar itens: clique com o botão esquerdo no item no chão. Se a bolsa estiver cheia ou pesada,"
  },
  "npc.prose.1d6eeb806b782a96": {
    "en": "from the throne the better. Its time for the free folk to",
    "zh-TW": "越早將其逐下王位越好。自由之民該是時候",
    "pt-BR": "quanto antes cair do trono, melhor. É hora de o povo livre"
  },
  "npc.prose.1d9fb688c88eab04": {
    "en": "We know talking about suicide can seem",
    "zh-TW": "我們知道，談論自殺似乎會讓人",
    "pt-BR": "Sabemos que falar sobre suicídio pode parecer"
  },
  "npc.prose.1db9b400b7ecf060": {
    "en": "Since I became a demon of a man due to the blood of RedMoonEvil...",
    "zh-TW": "自從赤月惡魔的血讓我變成了人形惡魔……",
    "pt-BR": "Desde que o sangue do Demônio da Lua Vermelha me tornou um demônio em forma humana..."
  },
  "npc.prose.1de41f04498ac407": {
    "en": "Lasts maximum of 15 seconds. Chance to gain Poison",
    "zh-TW": "最多持續 15 秒。有機率獲得毒素",
    "pt-BR": "Dura no máximo 15 segundos. Há uma chance de obter Veneno"
  },
  "npc.prose.1e53128514a7747f": {
    "en": "Ah! What is the Infinite Bout?",
    "zh-TW": "啊！什麼是無限挑戰？",
    "pt-BR": "Ah! O que é o Desafio Infinito?"
  },
  "npc.prose.1e64a376116bb643": {
    "en": "Heal himself or other players.",
    "zh-TW": "治療自己或其他玩家。",
    "pt-BR": "Cura a si mesmo ou a outros jogadores."
  },
  "npc.prose.1e6dd52bbb5e8175": {
    "en": "researching it. Go speak to Emperor Far in the Palace he knows",
    "zh-TW": "研究它。去皇宮找 Far 皇帝談談，他知道",
    "pt-BR": "pesquisando isso. Fale com o imperador Far no palácio; ele sabe"
  },
  "npc.prose.1e98fd1726d1e5c2": {
    "en": "Sorry traveller I don't trust you.",
    "zh-TW": "抱歉，旅人，我不信任你。",
    "pt-BR": "Desculpe, viajante, não confio em você."
  },
  "npc.prose.1e9e5b937a041181": {
    "en": "Rental days remaining: {npc_arg0}.",
    "zh-TW": "剩餘租用天數：{npc_arg0}。",
    "pt-BR": "Dias restantes de aluguel: {npc_arg0}."
  },
  "npc.prose.1ede669c78c6490e": {
    "en": "{npc_arg0}, You are currently in a guild.",
    "zh-TW": "{npc_arg0}，你目前已加入行會。",
    "pt-BR": "{npc_arg0}, você já pertence a uma guilda."
  },
  "npc.prose.1f0ffa0aeb1f9ecb": {
    "en": "So don't lose your valuable items by being reckless.",
    "zh-TW": "所以，別因為魯莽而失去貴重物品。",
    "pt-BR": "Por isso, não perca seus itens valiosos por imprudência."
  },
  "npc.prose.1f23780d3b1dc8c9": {
    "en": "Select any skill you wish to know.",
    "zh-TW": "請選擇想了解的技能。",
    "pt-BR": "Selecione a habilidade sobre a qual deseja saber mais."
  },
  "npc.prose.1f2c42d07477ff84": {
    "en": "skill to let my soul remain even after my body Died. Now I would",
    "zh-TW": "能讓我的肉身死去後，靈魂依然存在的技能。如今我想",
    "pt-BR": "uma técnica para que minha alma sobrevivesse à morte do corpo. Agora eu gostaria"
  },
  "npc.prose.1f3a5d064d0f3335": {
    "en": "Show me the Material's..",
    "zh-TW": "讓我看看材料……",
    "pt-BR": "Mostre-me os materiais..."
  },
  "npc.prose.1f4896a280e9d1ee": {
    "en": "They do not attack until you hit them.",
    "zh-TW": "除非你先攻擊，否則牠們不會攻擊你。",
    "pt-BR": "Eles só atacam se você os atingir primeiro."
  },
  "npc.prose.1fc71448433a881c": {
    "en": "HItting accuracy and destructive power will be increased",
    "zh-TW": "命中率與破壞力都會提升",
    "pt-BR": "A precisão dos golpes e o poder destrutivo aumentarão"
  },
  "npc.prose.2019e306b7b80ec1": {
    "en": "Hello i'm Kyle, the wandering warrior.",
    "zh-TW": "你好，我是凱爾，一名流浪戰士。",
    "pt-BR": "Olá, sou Kyle, um guerreiro errante."
  },
  "npc.prose.2046b5e3f79552bf": {
    "en": "Summons a HolyDeva.",
    "zh-TW": "召喚聖獸。",
    "pt-BR": "Invoca uma Deva Sagrada."
  },
  "npc.prose.205c3575d5171442": {
    "en": "An Monument which read's",
    "zh-TW": "紀念碑上寫著",
    "pt-BR": "Um monumento onde está escrito"
  },
  "npc.prose.205c82daa0d1bfa3": {
    "en": "Thunderstorm (Level 30)",
    "zh-TW": "地獄雷光（30 級）",
    "pt-BR": "Tempestade de Raios (nível 30)"
  },
  "npc.prose.20678ee22282e348": {
    "en": "But this is what the Stone says.",
    "zh-TW": "但石頭上是這麼寫的。",
    "pt-BR": "Mas é isso que está escrito na pedra."
  },
  "npc.prose.2083977d47d1e026": {
    "en": "Ah that Relic, such a beautiful piece of work.",
    "zh-TW": "啊，那件遺物，真是一件精美的作品。",
    "pt-BR": "Ah, aquela relíquia, uma obra tão bela."
  },
  "npc.prose.20be566e6a2d82f3": {
    "en": "Managing the affairs of a Game Master like you is a pleasure.",
    "zh-TW": "能為你這樣的遊戲管理員處理事務，是我的榮幸。",
    "pt-BR": "É um prazer cuidar dos assuntos de um mestre do jogo como você."
  },
  "npc.prose.20d02827524db14c": {
    "en": "You can sell all your loot to him.",
    "zh-TW": "你可以把所有戰利品賣給他。",
    "pt-BR": "Você pode vender todo o seu espólio para ele."
  },
  "npc.prose.20d69d6a4016f088": {
    "en": "may be able to help you.",
    "zh-TW": "或許能幫助你。",
    "pt-BR": "talvez possa ajudar você."
  },
  "npc.prose.211813863ee22324": {
    "en": "Caves:",
    "zh-TW": "洞穴：",
    "pt-BR": "Cavernas:"
  },
  "npc.prose.21814bdc8e33572a": {
    "en": "Archers are a class of great accuracy and strength,",
    "zh-TW": "弓箭手是擅長精準射擊與力量的職業，",
    "pt-BR": "Arqueiros são uma classe de grande precisão e força,"
  },
  "npc.prose.21905f8e1d80add3": {
    "en": "Rev.taoist of this school thought he was pitiful and permitted",
    "zh-TW": "本門道長憐憫他的處境，便准許",
    "pt-BR": "O reverendo taoísta desta escola teve pena dele e permitiu"
  },
  "npc.prose.21c9d5f710ccc2fc": {
    "en": "L: {npc_arg0} R: {npc_arg1}",
    "zh-TW": "左：{npc_arg0}　右：{npc_arg1}",
    "pt-BR": "E: {npc_arg0} D: {npc_arg1}"
  },
  "npc.prose.2204dbd8f4ebe772": {
    "en": "VamnpireShot (Level 26)",
    "zh-TW": "吸血箭（26 級）",
    "pt-BR": "Disparo Vampírico (nível 26)"
  },
  "npc.prose.220d86dbcb038a58": {
    "en": "of commission sales. We are located at five places of",
    "zh-TW": "寄售服務。我們在五個地方設有據點",
    "pt-BR": "de vendas em consignação. Estamos em cinco locais de"
  },
  "npc.prose.22a7ab46b6c68b7c": {
    "en": "It was our very best sword a long time ago. But we don't",
    "zh-TW": "很久以前，這曾是我們最好的劍。但我們不",
    "pt-BR": "Era nossa melhor espada há muito tempo. Mas não"
  },
  "npc.prose.22c5414372711d16": {
    "en": "Max. 9 people can be invisible if all are in 3x3 square area.",
    "zh-TW": "若都站在 3×3 的區域內，最多可讓 9 人隱身。",
    "pt-BR": "Até 9 pessoas podem ficar invisíveis se estiverem em uma área de 3×3 casas."
  },
  "npc.prose.22c549efab848b70": {
    "en": "poverty and can't afford to get a hair cut.",
    "zh-TW": "生活貧困，連剪頭髮都負擔不起。",
    "pt-BR": "na pobreza, sem poder pagar um corte de cabelo."
  },
  "npc.prose.22d3043f789b49d0": {
    "en": "open the closed door. I can let you in, but you will not be able to",
    "zh-TW": "打開封閉的門。我可以讓你進去，但你將無法",
    "pt-BR": "abrir a porta fechada. Posso deixar você entrar, mas não poderá"
  },
  "npc.prose.230046a7c0b6cf01": {
    "en": "Welcome to the Mir world.",
    "zh-TW": "歡迎來到傳奇世界。",
    "pt-BR": "Boas-vindas ao mundo de Mir."
  },
  "npc.prose.231bbabc38577059": {
    "en": "\"Only party leaders with their parties may pass here.\"",
    "zh-TW": "「只有帶著隊伍的隊長才能通過此處。」",
    "pt-BR": "“Somente líderes acompanhados de seus grupos podem passar.”"
  },
  "npc.prose.2335048bff742ae8": {
    "en": "I will not speak to such a person like you.",
    "zh-TW": "我不會和你這樣的人說話。",
    "pt-BR": "Não vou falar com alguém como você."
  },
  "npc.prose.23611603d3b302d4": {
    "en": "the taoist school located below the mountain. But they didn't trust me",
    "zh-TW": "山下的道門。但他們不信任我",
    "pt-BR": "a escola taoísta ao pé da montanha. Mas eles não confiaram em mim"
  },
  "npc.prose.23646a72a01118d4": {
    "en": "If you wish for a safe hunt, try Hen, Deer, and Sheep near town.",
    "zh-TW": "若想安全地狩獵，可挑戰城鎮附近的雞、鹿和羊。",
    "pt-BR": "Para uma caçada segura, procure galinhas, cervos e ovelhas perto da cidade."
  },
  "npc.prose.236911228cc37645": {
    "en": "JadeCrystal holding. Some special taoistic power to pass there, Only a",
    "zh-TW": "翡翠水晶所蘊含的力量。通過那裡需要特殊道術，只有",
    "pt-BR": "o poder do Cristal de Jade. É preciso um poder taoísta especial para passar; somente um"
  },
  "npc.prose.23bd947600550d0f": {
    "en": "What haircut would you like?",
    "zh-TW": "你想剪什麼髮型？",
    "pt-BR": "Que corte de cabelo você gostaria?"
  },
  "npc.prose.243e475c7f455e87": {
    "en": "(*You hear a slight whisper in the wind:*)",
    "zh-TW": "（＊你聽見風中傳來輕聲低語：＊）",
    "pt-BR": "(*Você ouve um leve sussurro no vento:*)"
  },
  "npc.prose.246077e55cd8fe91": {
    "en": "You should then see a piece of meat appear in your inventory.",
    "zh-TW": "接著，你應該會看到背包中出現一塊肉。",
    "pt-BR": "Então você deverá ver um pedaço de carne na bolsa."
  },
  "npc.prose.247f0cd60ce8d916": {
    "en": "A Mysterious Stone with Ancient symbols.",
    "zh-TW": "一塊刻有古老符號的神祕石頭。",
    "pt-BR": "Uma pedra misteriosa com símbolos antigos."
  },
  "npc.prose.24dcc006324f7fda": {
    "en": "teleport to the village stores",
    "zh-TW": "傳送到村莊商店",
    "pt-BR": "teleportar para as lojas da vila"
  },
  "npc.prose.250f9a29ffaa17d3": {
    "en": "Hello I'm Harrison, the wandering warrior.",
    "zh-TW": "你好，我是哈里森，一名流浪戰士。",
    "pt-BR": "Olá, sou Harrison, um guerreiro errante."
  },
  "npc.prose.2541a348c461cadd": {
    "en": "Hello Traveller. You found me hiding away from",
    "zh-TW": "你好，旅人。你找到了正在躲避的我",
    "pt-BR": "Olá, viajante. Você me encontrou escondido de"
  },
  "npc.prose.256d1545a2c2a2da": {
    "en": "sword, But I had murdered several people already by that time.",
    "zh-TW": "劍，但那時我已經殺了好幾個人。",
    "pt-BR": "a espada, mas a essa altura eu já havia matado várias pessoas."
  },
  "npc.prose.258fcfb4551d2042": {
    "en": "What would like to purchase?",
    "zh-TW": "你想購買什麼？",
    "pt-BR": "O que gostaria de comprar?"
  },
  "npc.prose.25ccb9c0b57bfe15": {
    "en": "\"\" Level10~60.",
    "zh-TW": "「」10～60 級。",
    "pt-BR": "“” Níveis 10–60."
  },
  "npc.prose.25d656ca35a7722b": {
    "en": "Welcome Traveller, my name is Dean, I'm a Master Artisan..",
    "zh-TW": "歡迎，旅人。我叫迪恩，是一位工藝大師……",
    "pt-BR": "Boas-vindas, viajante. Meu nome é Dean, sou um mestre artesão..."
  },
  "npc.prose.25dc41cc2ca3e69f": {
    "en": "clicked, click once more will remove image.",
    "zh-TW": "點擊後，再點一次就會移除圖像。",
    "pt-BR": "clicada; clique novamente para remover a imagem."
  },
  "npc.prose.261e82729f56ce32": {
    "en": "Hello, I am Miss Do. I will be guiding the people of Level 1~30.",
    "zh-TW": "你好，我是杜小姐。我會引導 1～30 級的玩家。",
    "pt-BR": "Olá, sou a senhorita Do. Vou orientar os jogadores dos níveis 1 a 30."
  },
  "npc.prose.2623a7648b282a4f": {
    "en": "return.",
    "zh-TW": "返回。",
    "pt-BR": "retornar."
  },
  "npc.prose.267849868bb99543": {
    "en": "Rebirth 1 cleared.",
    "zh-TW": "已清除第一次轉生。",
    "pt-BR": "Primeiro renascimento removido."
  },
  "npc.prose.268bc1833e83306f": {
    "en": "Thank you",
    "zh-TW": "謝謝你",
    "pt-BR": "Obrigado."
  },
  "npc.prose.26abb9b155f41e44": {
    "en": "poverty. So, I would like to help them with my making skills.",
    "zh-TW": "貧困。所以，我想用自己的製作技術幫助他們。",
    "pt-BR": "na pobreza. Por isso, quero ajudá-los com minhas habilidades de fabricação."
  },
  "npc.prose.26e84eb238b86b9e": {
    "en": "to extend. If no fee is paid even within the additional 7 days,",
    "zh-TW": "來延長期限。若額外 7 天內仍未付費，",
    "pt-BR": "para prorrogar. Se a taxa não for paga nos 7 dias adicionais,"
  },
  "npc.prose.2710810efd935a0d": {
    "en": "Fireball (Level 7)",
    "zh-TW": "火球術（7 級）",
    "pt-BR": "Bola de Fogo (nível 7)"
  },
  "npc.prose.271219cdceb2c137": {
    "en": "You must be a tough fighter to survive in Mir world. The first requirement is",
    "zh-TW": "要在傳奇世界生存，你必須成為強悍的鬥士。首要條件是",
    "pt-BR": "Para sobreviver no mundo de Mir, você precisa ser um lutador forte. O primeiro requisito é"
  },
  "npc.prose.2728a5031508fdf3": {
    "en": "Infuses your arrow with mana to deal extra damage.",
    "zh-TW": "為箭矢注入魔力，造成額外傷害。",
    "pt-BR": "Infunde mana na flecha para causar dano adicional."
  },
  "npc.prose.275dd985116d14bf": {
    "en": "what he is investigating",
    "zh-TW": "他正在調查的事",
    "pt-BR": "o que ele está investigando"
  },
  "npc.prose.27e0d68e129f8e65": {
    "en": "CaveMaggot, and Skeletons appear.",
    "zh-TW": "會出現洞蛆與骷髏。",
    "pt-BR": "aparecem vermes de caverna e esqueletos."
  },
  "npc.prose.28015c212cb94273": {
    "en": "Where shall I teleport you?",
    "zh-TW": "你想傳送到哪裡？",
    "pt-BR": "Para onde devo teleportar você?"
  },
  "npc.prose.282479f648e90a91": {
    "en": "Oh, if you fall down during the fight,",
    "zh-TW": "啊，如果你在戰鬥中倒下，",
    "pt-BR": "Ah, se você cair durante a luta,"
  },
  "npc.prose.282c691b93ec234d": {
    "en": "monster beat within it... Its power was so strong that I was frightened",
    "zh-TW": "怪物在其中搏動……那股力量強大得令我恐懼",
    "pt-BR": "o monstro pulsava em seu interior... Seu poder era tão forte que fiquei com medo"
  },
  "npc.prose.286d3074201d3c01": {
    "en": "the scary monsters.",
    "zh-TW": "那些可怕的怪物。",
    "pt-BR": "os monstros assustadores."
  },
  "npc.prose.287c22b700ffd359": {
    "en": "Main Rebirth Flag granted.",
    "zh-TW": "已授予主轉生標記。",
    "pt-BR": "Marca principal de renascimento concedida."
  },
  "npc.prose.28a83badf49ba4a4": {
    "en": "Which item would you like to Buy or Sell?",
    "zh-TW": "你想購買或出售哪件物品？",
    "pt-BR": "Qual item deseja comprar ou vender?"
  },
  "npc.prose.28b2bff0d8f98151": {
    "en": "I will start & cancel the sale of guild territory.",
    "zh-TW": "我可以開始或取消行會領地的出售。",
    "pt-BR": "Posso iniciar ou cancelar a venda de um território de guilda."
  },
  "npc.prose.28e784d3086b5656": {
    "en": "*Item dispose: Open the inventory and left click the item",
    "zh-TW": "＊丟棄物品：開啟背包，左鍵點擊物品",
    "pt-BR": "*Descartar itens: abra a bolsa e clique no item com o botão esquerdo"
  },
  "npc.prose.29753e4c30c4c111": {
    "en": "Please select the item you want to buy.",
    "zh-TW": "請選擇你想購買的物品。",
    "pt-BR": "Selecione o item que deseja comprar."
  },
  "npc.prose.298635dec9df178b": {
    "en": "speed drops significantly.",
    "zh-TW": "速度會明顯下降。",
    "pt-BR": "a velocidade cai significativamente."
  },
  "npc.prose.29ad936e331533b0": {
    "en": "in accordance with practice level.",
    "zh-TW": "隨著熟練程度提高。",
    "pt-BR": "de acordo com o nível de prática."
  },
  "npc.prose.29c3c72e8e846caf": {
    "en": "to help you out of a tight situation.",
    "zh-TW": "幫助你脫離困境。",
    "pt-BR": "para ajudar você a sair de uma situação difícil."
  },
  "npc.prose.29dc8ea36006658e": {
    "en": "*Private chat          *Group shout          *Guild shout",
    "zh-TW": "＊私聊　　＊隊伍喊話　　＊行會喊話",
    "pt-BR": "*Conversa privada  *Mensagem de grupo  *Mensagem de guilda"
  },
  "npc.prose.2a0a840b8142e170": {
    "en": "explainshow to enter hell. It is said the patch to hell was discovered",
    "zh-TW": "說明如何進入地獄。據說通往地獄的道路被發現於",
    "pt-BR": "explica como entrar no inferno. Dizem que o caminho foi descoberto"
  },
  "npc.prose.2a2325c2592f32fe": {
    "en": "But be specially aware of poison from Spitting Spider.",
    "zh-TW": "但要特別小心噴毒蜘蛛的毒。",
    "pt-BR": "Mas tome cuidado especial com o veneno da Aranha Cuspidora."
  },
  "npc.prose.2a447b5a81c5bfc0": {
    "en": "Will you continue the challenge?",
    "zh-TW": "你要繼續挑戰嗎？",
    "pt-BR": "Deseja continuar o desafio?"
  },
  "npc.prose.2a77287a3c524283": {
    "en": "The art of making multiple power attacks that have the energy equivalent to",
    "zh-TW": "使出多次強力攻擊的技術，其能量相當於",
    "pt-BR": "A técnica de desferir vários ataques poderosos com energia equivalente a"
  },
  "npc.prose.2b1f0ee9022ada9f": {
    "en": "at 8o'clock direction reeling backwards.",
    "zh-TW": "朝八點鐘方向向後退。",
    "pt-BR": "recuando na direção das oito horas."
  },
  "npc.prose.2b454e85aa08d764": {
    "en": "You'll have to pay {npc_arg0} Gold as request fee.",
    "zh-TW": "你需要支付 {npc_arg0} 金幣的委託費。",
    "pt-BR": "Você terá de pagar {npc_arg0} moedas de ouro como taxa do pedido."
  },
  "npc.prose.2b5e518c2be13969": {
    "en": "In the Infinite Bout, you have to slay all the",
    "zh-TW": "在無限挑戰中，你必須消滅所有",
    "pt-BR": "No Desafio Infinito, você precisa derrotar todos os"
  },
  "npc.prose.2b625e792d140cd5": {
    "en": "Before the relief comes I can not leave the spot freely",
    "zh-TW": "在援軍抵達前，我不能擅自離開崗位",
    "pt-BR": "Não posso abandonar meu posto antes da chegada dos reforços"
  },
  "npc.prose.2ba62c2b69f5445f": {
    "en": "Clothes",
    "zh-TW": "服裝",
    "pt-BR": "Roupas"
  },
  "npc.prose.2ba8222f3460597e": {
    "en": "to ask the oldman for the date and time of the war.",
    "zh-TW": "向那位老人詢問戰爭的日期與時間。",
    "pt-BR": "para perguntar ao ancião a data e a hora da guerra."
  },
  "npc.prose.2c1ac9bb619bb88e": {
    "en": "You may not pass.All Must be within 5 spots of npc.\"",
    "zh-TW": "你們不能通過。所有人都必須在 NPC 周圍 5 格內。",
    "pt-BR": "Vocês não podem passar. Todos devem estar a até 5 casas do NPC."
  },
  "npc.prose.2c53d9b559f740b3": {
    "en": "A RepairOil makes the durability of your hand weapon rise.",
    "zh-TW": "修理油能提高手持武器的耐久度。",
    "pt-BR": "O Óleo de Reparo aumenta a durabilidade da arma que você empunha."
  },
  "npc.prose.2c565b9dc541ab87": {
    "en": "A Mysterious Stone with Unknown symbols.",
    "zh-TW": "一塊刻有未知符號的神祕石頭。",
    "pt-BR": "Uma pedra misteriosa com símbolos desconhecidos."
  },
  "npc.prose.137f5beb20c9629c": {
    "en": "{npc_arg0}    -    {npc_arg1}",
    "zh-TW": "{npc_arg0}    -    {npc_arg1}",
    "pt-BR": "{npc_arg0}    -    {npc_arg1}",
    "sameValueReason": "Only opaque parameters and layout punctuation; no translatable prose."
  },
  "npc.prose.29671bdf780f4a41": {
    "en": "-------------------------------------------------------------------------------------------------------",
    "zh-TW": "-------------------------------------------------------------------------------------------------------",
    "pt-BR": "-------------------------------------------------------------------------------------------------------",
    "sameValueReason": "Only opaque parameters and layout punctuation; no translatable prose."
  },
  "npc.prose.2c59acb8cbd041da": {
    "en": "What Item would you like to buy or sell?",
    "zh-TW": "你想購買或出售什麼物品？",
    "pt-BR": "Que item deseja comprar ou vender?"
  },
  "npc.prose.2c5ab531e605cb13": {
    "en": "That was about ... 20... 48... years ago....",
    "zh-TW": "那大約是……20……48……年前的事了……",
    "pt-BR": "Isso foi há uns... 20... 48... anos..."
  },
  "npc.prose.2c65f5d6debdb806": {
    "en": "guild territory.",
    "zh-TW": "行會領地。",
    "pt-BR": "território de guilda."
  },
  "npc.prose.2c924b9f38f98917": {
    "en": "Welcome {npc_arg0}, I am the Awakening Master.",
    "zh-TW": "歡迎，{npc_arg0}。我是覺醒大師。",
    "pt-BR": "Boas-vindas, {npc_arg0}. Sou o mestre do despertar."
  },
  "npc.prose.2c98c83f6380fe3c": {
    "en": "Have you heard the rumours of the beast",
    "zh-TW": "你聽過關於那頭野獸的傳聞嗎",
    "pt-BR": "Você ouviu os rumores sobre a fera"
  },
  "npc.prose.2cc31d7b6e4f619b": {
    "en": "Makes invisible to mobs by hiding traces. Hiding will allow player not to be",
    "zh-TW": "隱去蹤跡，使怪物無法看見你。隱身能讓玩家不被",
    "pt-BR": "Oculta os rastros e torna você invisível aos monstros. A ocultação permite que o jogador não seja"
  },
  "npc.prose.2cf04cc58937c6e9": {
    "en": "and Portal Scroll are considered as essential items.",
    "zh-TW": "與傳送卷軸都被視為必備物品。",
    "pt-BR": "e pergaminhos de portal são considerados itens essenciais."
  },
  "npc.prose.2d390e31fb653b5a": {
    "en": "Sell.",
    "zh-TW": "出售。",
    "pt-BR": "Vender."
  },
  "npc.prose.2d600a6bc07bcda4": {
    "en": "It can be executed out of certain ranges from the",
    "zh-TW": "可在距離一定範圍之外施放",
    "pt-BR": "Pode ser executada fora de determinados alcances do"
  },
  "npc.prose.2d6d1a3668139dc4": {
    "en": "make you randonly move to another place in the same map.",
    "zh-TW": "讓你隨機移動到同一張地圖上的其他位置。",
    "pt-BR": "move você para outro lugar aleatório do mesmo mapa."
  },
  "npc.prose.2d74610fd86172c7": {
    "en": "Parcels",
    "zh-TW": "包裹",
    "pt-BR": "Encomendas"
  },
  "npc.prose.2dcb68a1a27e9428": {
    "en": "Warrior:",
    "zh-TW": "戰士：",
    "pt-BR": "Guerreiro:"
  },
  "npc.prose.2e1d428447152836": {
    "en": "Guilds made by using alt codes or spaces in the name",
    "zh-TW": "若行會名稱使用特殊字元或空格",
    "pt-BR": "Guildas cujos nomes usam códigos Alt ou espaços"
  },
  "npc.prose.2e4a30fc0706619b": {
    "en": "Throw the amulet and a very strong poison cloud will appear in the area.",
    "zh-TW": "投出護身符，在範圍內形成強力毒霧。",
    "pt-BR": "Lança um talismã e cria uma nuvem de veneno muito forte na área."
  },
  "npc.prose.2e9c4a721daf986d": {
    "en": "*Ban shout",
    "zh-TW": "＊屏蔽喊話",
    "pt-BR": "*Bloquear gritos"
  },
  "npc.prose.2ed7a88a0e73527f": {
    "en": "Once you go far away from town, hunt HookingCat, RakingCat, Yob, Wolf,",
    "zh-TW": "離開城鎮較遠後，可狩獵鉤貓、釘耙貓、多鉤貓、狼，",
    "pt-BR": "Longe da cidade, cace Gatos com Gancho, Gatos com Ancinho, Yobs, Lobos,"
  },
  "npc.prose.2ee7ed1dc2835808": {
    "en": "\"Banished Creatures \"",
    "zh-TW": "「被放逐的生物」",
    "pt-BR": "“Criaturas Banidas”"
  },
  "npc.prose.2f301b45fb67f0da": {
    "en": "Hello again traveler.. How are you on this fine day?",
    "zh-TW": "又見面了，旅人……今天過得如何？",
    "pt-BR": "Olá de novo, viajante... Como vai neste belo dia?"
  },
  "npc.prose.2f3312a1d2c143dc": {
    "en": "A strong flame around targeted area with 3x3 square range.",
    "zh-TW": "在目標周圍 3×3 格範圍內燃起猛烈火焰。",
    "pt-BR": "Cria chamas intensas em uma área de 3×3 casas ao redor do alvo."
  },
  "npc.prose.2f3f132426138d3e": {
    "en": "*Ban private chat                            *Withdraw guild",
    "zh-TW": "＊屏蔽私聊　　＊退出行會",
    "pt-BR": "*Bloquear conversa privada  *Sair da guilda"
  },
  "npc.prose.2f7cb8adcb500169": {
    "en": "After the rental period expiration, the guild is given 7 days",
    "zh-TW": "租期屆滿後，行會還有 7 天",
    "pt-BR": "Após o término do aluguel, a guilda recebe 7 dias"
  },
  "npc.prose.3016983dcd22097b": {
    "en": "Can gain up to 4 elements.",
    "zh-TW": "最多可獲得 4 個元素。",
    "pt-BR": "É possível obter até 4 elementos."
  },
  "npc.prose.307bd0104d8f0857": {
    "en": "you're brave enough.",
    "zh-TW": "只要你夠勇敢。",
    "pt-BR": "se tiver coragem suficiente."
  },
  "npc.prose.30cd6f4e701f4753": {
    "en": "leave themselves open to frontal attacks. However,",
    "zh-TW": "讓自己暴露在正面攻擊之下。不過，",
    "pt-BR": "ficam vulneráveis a ataques frontais. No entanto,"
  },
  "npc.prose.311840dd38a8fa01": {
    "en": "Lay down your fishies to be sold",
    "zh-TW": "把要出售的魚放下吧",
    "pt-BR": "Coloque aqui os peixes que deseja vender."
  },
  "npc.prose.3150e178891d6a50": {
    "en": "I see you're wearing: L: {npc_arg0} R: {npc_arg1}",
    "zh-TW": "我看見你戴著：左：{npc_arg0}　右：{npc_arg1}",
    "pt-BR": "Vejo que você usa: E: {npc_arg0} D: {npc_arg1}"
  },
  "npc.prose.3163bdaf2ef70e07": {
    "en": "Welcome, What can I do for you?",
    "zh-TW": "歡迎，有什麼能為你效勞？",
    "pt-BR": "Boas-vindas. Como posso ajudar?"
  },
  "npc.prose.316ccb9742736f9c": {
    "en": "I think i have seen you before...",
    "zh-TW": "我好像以前見過你……",
    "pt-BR": "Acho que já vi você antes..."
  },
  "npc.prose.31c6423453082209": {
    "en": "Brings snow storm in a targetted area (5x5 square).",
    "zh-TW": "在目標區域引發暴風雪（5×5 格）。",
    "pt-BR": "Provoca uma nevasca na área escolhida (5×5 casas)."
  },
  "npc.prose.31d5b4ea6e29e75d": {
    "en": "You know what makes my life harder?",
    "zh-TW": "你知道什麼讓我的生活更辛苦嗎？",
    "pt-BR": "Sabe o que torna minha vida mais difícil?"
  },
  "npc.prose.31e56a63ff89fea3": {
    "en": "This will make hunts last longer.",
    "zh-TW": "這能讓你狩獵更久。",
    "pt-BR": "Isso permitirá caçadas mais longas."
  },
  "npc.prose.31f3bdceddab02e0": {
    "en": "All rebirth flags applied.",
    "zh-TW": "已套用所有轉生標記。",
    "pt-BR": "Todas as marcas de renascimento foram aplicadas."
  },
  "npc.prose.3219b164557ed27a": {
    "en": "In order to go to restricted ares, you are required to have a",
    "zh-TW": "若想前往限制區域，你必須擁有",
    "pt-BR": "Para entrar nas áreas restritas, você precisa ter um"
  },
  "npc.prose.324cd7ebe9537289": {
    "en": "//////////////////////////////////////////////////////////////////// WhiteDragonPassage *Movements missing*",
    "zh-TW": "//////////////////////////////////////////////////////////////////// 白龍通道　＊缺少移動設定＊",
    "pt-BR": "//////////////////////////////////////////////////////////////////// Passagem do Dragão Branco *Movimentos ausentes*"
  },
  "npc.prose.3284e9fb7355fdc5": {
    "en": "Holy Sword...",
    "zh-TW": "聖劍……",
    "pt-BR": "Espada Sagrada..."
  },
  "npc.prose.32a05555fbc56b1b": {
    "en": "the omas.",
    "zh-TW": "那些半獸人。",
    "pt-BR": "os Omas."
  },
  "npc.prose.32b3dab90bc2829c": {
    "en": "Here you can Move your Cash from Sabuk to your Guild Bank.",
    "zh-TW": "你可以在此將沙巴克的資金轉入行會銀行。",
    "pt-BR": "Aqui você pode transferir o dinheiro de Sabuk para o banco da sua guilda."
  },
  "npc.prose.32c5f2f27ab510aa": {
    "en": "I'm just hoping we can return home soon",
    "zh-TW": "我只希望能早點回家",
    "pt-BR": "Só espero que possamos voltar para casa logo."
  },
  "npc.prose.32f62648133f3486": {
    "en": "I am the local hairdresser, specialized in the cutting hair.",
    "zh-TW": "我是本地的理髮師，專門替人剪髮。",
    "pt-BR": "Sou o cabeleireiro local, especializado em cortes de cabelo."
  },
  "npc.prose.330edfe58c6f6e9d": {
    "en": "Characteristic of Wizard skills are all long range attack and some does",
    "zh-TW": "法師的技能特色是遠距攻擊，其中有些會",
    "pt-BR": "As habilidades de mago são de longo alcance, e algumas"
  },
  "npc.prose.333fc83df49acb39": {
    "en": "Hello {npc_arg0}, my name is {npc_arg1}.",
    "zh-TW": "你好，{npc_arg0}，我叫 {npc_arg1}。",
    "pt-BR": "Olá, {npc_arg0}. Meu nome é {npc_arg1}."
  },
  "npc.prose.33646490a660b196": {
    "en": "A skill that paralyses or confuses enemies by emitting a strong electric shock",
    "zh-TW": "釋放強烈電擊，使敵人麻痺或混亂的技能",
    "pt-BR": "Uma habilidade que paralisa ou confunde inimigos ao emitir uma forte descarga elétrica"
  },
  "npc.prose.3365c2deee2d5bb7": {
    "en": "Which frontier do you want to talk about?",
    "zh-TW": "你想了解哪個邊境地區？",
    "pt-BR": "Sobre qual fronteira deseja conversar?"
  },
  "npc.prose.3388c99156108750": {
    "en": "You should get yourself prepared here",
    "zh-TW": "你應該先在這裡做好準備",
    "pt-BR": "Você deve se preparar aqui."
  },
  "npc.prose.34364c9e815eb1d8": {
    "en": "Archer Two                         Archer Five",
    "zh-TW": "弓箭手二　　弓箭手五",
    "pt-BR": "Arqueiro Dois  Arqueiro Cinco"
  },
  "npc.prose.34412509569c6a04": {
    "en": "the owner of this school knows more details about it",
    "zh-TW": "本門掌門知道更詳細的情況",
    "pt-BR": "o mestre desta escola sabe mais detalhes sobre isso"
  },
  "npc.prose.3450079d6abc01c3": {
    "en": "depending on your level after rebirth.",
    "zh-TW": "取決於你轉生後的等級。",
    "pt-BR": "dependendo do seu nível após renascer."
  },
  "npc.prose.346d870e0382317e": {
    "en": "Yes I researched a Ancient Language... I didn't have much luck",
    "zh-TW": "是的，我研究過一種古代語言……但進展不大",
    "pt-BR": "Sim, pesquisei uma língua antiga... Não tive muita sorte."
  },
  "npc.prose.34744f65b1f3e709": {
    "en": "\"From another being who endlessly wanders this place,\"",
    "zh-TW": "「來自另一個無盡徘徊於此的存在，」",
    "pt-BR": "“De outro ser que vagueia sem fim por este lugar,”"
  },
  "npc.prose.34b6af5028403777": {
    "en": "I can take you there",
    "zh-TW": "我可以帶你去那裡",
    "pt-BR": "Posso levar você até lá."
  },
  "npc.prose.34b7fb2d82b20a1a": {
    "en": "The fee is only '3000 Gold'. What? Too expensive?",
    "zh-TW": "費用只要 3,000 金幣。什麼？太貴了？",
    "pt-BR": "A taxa é de apenas 3.000 moedas de ouro. O quê? Muito caro?"
  },
  "npc.prose.352efdcaf621557a": {
    "en": "Protects the caster with an elemental barrier.",
    "zh-TW": "以元素屏障保護施法者。",
    "pt-BR": "Protege o conjurador com uma barreira elemental."
  },
  "npc.prose.35465731c07e2ebc": {
    "en": "in my mind The HolySword also became a demon sword, a",
    "zh-TW": "在我心中，聖劍也變成了魔劍，一把",
    "pt-BR": "em minha mente, a Espada Sagrada também se tornou uma espada demoníaca, uma"
  },
  "npc.prose.355f97c0d8fb8c3b": {
    "en": "I will get to the bottom of it.",
    "zh-TW": "我會查個水落石出。",
    "pt-BR": "Vou descobrir a verdade."
  },
  "npc.prose.35e5e15089dd4ac2": {
    "en": "The art of freezing that uses the force of ice.",
    "zh-TW": "運用冰之力量的凍結法術。",
    "pt-BR": "A arte de congelar usando a força do gelo."
  },
  "npc.prose.36532d14f61064a4": {
    "en": "{npc_arg0} You cannot leave just yet.",
    "zh-TW": "{npc_arg0}，你現在還不能離開。",
    "pt-BR": "{npc_arg0}, você ainda não pode sair."
  },
  "npc.prose.3691b07c933d6ffb": {
    "en": "If you wish for higher exp. Go OmaCave northwest",
    "zh-TW": "若想獲得更多經驗，可前往西北方的半獸人洞穴",
    "pt-BR": "Para ganhar mais experiência, vá à Caverna dos Omas, a noroeste"
  },
  "npc.prose.369aacd94f40066b": {
    "en": "What function would you like me to perform today {npc_arg0}",
    "zh-TW": "{npc_arg0}，今天想讓我執行什麼功能？",
    "pt-BR": "Que função deseja que eu execute hoje, {npc_arg0}?"
  },
  "npc.prose.37394bb1d487b2a8": {
    "en": "like to ask you, get back the RedMoonChip and bring it to me",
    "zh-TW": "請求你找回赤月碎片，並帶給我",
    "pt-BR": "pedir que recupere o Fragmento da Lua Vermelha e o traga para mim"
  },
  "npc.prose.37f3ec59f056eebb": {
    "en": "Cripple Shot and OneWithNature spells.",
    "zh-TW": "致殘射擊與天人合一技能。",
    "pt-BR": "as habilidades Disparo Debilitante e União com a Natureza."
  },
  "npc.prose.38307faad3f99a0b": {
    "en": "Mount Loyalty: {npc_arg0}",
    "zh-TW": "坐騎忠誠度：{npc_arg0}",
    "pt-BR": "Lealdade da montaria: {npc_arg0}"
  },
  "npc.prose.384395d40eaa7c3f": {
    "en": "Rebirth 2 -",
    "zh-TW": "第二次轉生－",
    "pt-BR": "Segundo renascimento —"
  },
  "npc.prose.387969c533c6abcb": {
    "en": "Candles are needed when it's dark. without a Candle",
    "zh-TW": "天黑時需要蠟燭。沒有蠟燭",
    "pt-BR": "Velas são necessárias no escuro. Sem uma vela,"
  },
  "npc.prose.38abbb79fcabc839": {
    "en": "expensive. If you don't want to buy it try the drops from",
    "zh-TW": "價格昂貴。若不想購買，可以試著取得掉落物",
    "pt-BR": "caro. Se não quiser comprar, tente obter o espólio de"
  },
  "npc.prose.38d4aefc0f36a1ed": {
    "en": "\"Never",
    "zh-TW": "「永不",
    "pt-BR": "“Nunca"
  },
  "npc.prose.390afa77a0abfe7e": {
    "en": "Oh yes... I have seen that language before... I spent years",
    "zh-TW": "喔，是的……我見過那種語言……我花了好多年",
    "pt-BR": "Ah, sim... Já vi essa língua... Passei anos"
  },
  "npc.prose.390f84e16305b2a3": {
    "en": "Meat.",
    "zh-TW": "肉。",
    "pt-BR": "Carne."
  },
  "npc.prose.393ac3c5f7afd64a": {
    "en": "are automatically 'kicked out' from the territory.",
    "zh-TW": "會自動被逐出領地。",
    "pt-BR": "são expulsos do território automaticamente."
  },
  "npc.prose.3988b0bd33cb189a": {
    "en": "Stranger.. Stranger.. Help. I'm scared.",
    "zh-TW": "陌生人……陌生人……救命。我好害怕。",
    "pt-BR": "Estranho... Estranho... Socorro. Estou com medo."
  },
  "npc.prose.39bb28cde674a6c0": {
    "en": "it's just a piece of Iron...",
    "zh-TW": "那只是一塊鐵……",
    "pt-BR": "é apenas um pedaço de ferro..."
  },
  "npc.prose.39f328c1f463ddae": {
    "en": "Required Gold: {npc_arg0} Last's for {npc_arg1} Minutes.",
    "zh-TW": "所需金幣：{npc_arg0}。持續 {npc_arg1} 分鐘。",
    "pt-BR": "Ouro necessário: {npc_arg0}. Dura {npc_arg1} minutos."
  },
  "npc.prose.3a008c65f8f0b15a": {
    "en": "when the crisis arrives.",
    "zh-TW": "危機來臨時。",
    "pt-BR": "quando a crise chegar."
  },
  "npc.prose.3a18a3b68fbfd596": {
    "en": "Your about to enter a world of death and frustration. My duty is to",
    "zh-TW": "你即將進入充滿死亡與挫折的世界。我的職責是",
    "pt-BR": "Você está prestes a entrar em um mundo de morte e frustração. Meu dever é"
  },
  "npc.prose.3a54ffaccfd55840": {
    "en": "means war done by legal request.",
    "zh-TW": "是指經正式申請而發動的戰爭。",
    "pt-BR": "significa uma guerra declarada mediante um pedido legal."
  },
  "npc.prose.3a8608e86f708554": {
    "en": "To get meat from any of the above, first of all kill the animal.",
    "zh-TW": "要取得上述動物的肉，首先必須殺死動物。",
    "pt-BR": "Para obter carne desses animais, primeiro mate o animal."
  },
  "npc.prose.3b0d9ecb3e08d2cd": {
    "en": "have private conversations and socialize with other members.",
    "zh-TW": "與其他成員私下交談、交流。",
    "pt-BR": "conversar em particular e socializar com outros membros."
  },
  "npc.prose.3b25536ae04bf403": {
    "en": "If left alone, other person will become a victim like me....",
    "zh-TW": "若置之不理，其他人也會像我一樣成為受害者……",
    "pt-BR": "Se nada for feito, outra pessoa se tornará uma vítima como eu..."
  },
  "npc.prose.3b3b1084c86bd97b": {
    "en": "bad-smelling room... It's the Crystal of huge demon. Filled with hatred...",
    "zh-TW": "惡臭的房間……那是巨魔的結晶，充滿憎恨……",
    "pt-BR": "um quarto fétido... É o cristal de um grande demônio, cheio de ódio..."
  },
  "npc.prose.3b495201d5087a06": {
    "en": "Province. Party is recommended since soloing here is very difficult.",
    "zh-TW": "省。建議組隊前往，因為獨自狩獵非常困難。",
    "pt-BR": "Província. Recomenda-se formar um grupo, pois lutar sozinho aqui é muito difícil."
  },
  "npc.prose.3b4d7000b72d7d1a": {
    "en": "Would you like to repair a necklace?",
    "zh-TW": "你想修理項鍊嗎？",
    "pt-BR": "Gostaria de reparar um colar?"
  },
  "npc.prose.3b6a6b18cc9d4377": {
    "en": "It damages all objects(9-10 enemies) in its path.",
    "zh-TW": "傷害路徑上的所有目標（9～10 個敵人）。",
    "pt-BR": "Causa dano a todos os alvos no caminho (9 a 10 inimigos)."
  },
  "npc.prose.3b9536400018a231": {
    "en": "Well guess the rumours are true this time..",
    "zh-TW": "看來這次傳聞是真的……",
    "pt-BR": "Parece que os rumores são verdadeiros desta vez..."
  },
  "npc.prose.3bb9db6541e6fe31": {
    "en": "Fire an arrow that has a delayed explosion.",
    "zh-TW": "射出一支延遲爆炸的箭。",
    "pt-BR": "Dispara uma flecha que explode após um intervalo."
  },
  "npc.prose.3bcedea31419c735": {
    "en": "Hello {npc_arg0}, It's very strange to see new travelers.",
    "zh-TW": "你好，{npc_arg0}。很少看到新來的旅人。",
    "pt-BR": "Olá, {npc_arg0}. É muito raro ver novos viajantes."
  },
  "npc.prose.3be7ce1249d2246a": {
    "en": "I see you're holding: {npc_arg0}",
    "zh-TW": "我看見你拿著：{npc_arg0}",
    "pt-BR": "Vejo que você segura: {npc_arg0}"
  },
  "npc.prose.3c02dbcaa548bc8a": {
    "en": "Helmet.",
    "zh-TW": "頭盔。",
    "pt-BR": "Capacete."
  },
  "npc.prose.3c2a3e41a5a946e0": {
    "en": "You currently have Rebirth 3.",
    "zh-TW": "你目前已完成第三次轉生。",
    "pt-BR": "Você possui atualmente o terceiro renascimento."
  },
  "npc.prose.3c2c563d62aaa64e": {
    "en": "I'm currently on the look out for many types of materials",
    "zh-TW": "我正在尋找多種材料",
    "pt-BR": "Estou procurando diversos tipos de materiais."
  },
  "npc.prose.3c47cd5ea5760652": {
    "en": "will be booted out automatically.",
    "zh-TW": "將被自動踢出。",
    "pt-BR": "serão expulsos automaticamente."
  },
  "npc.prose.3c7b71816f2ed7eb": {
    "en": "Would you like to repair a drapery piece?",
    "zh-TW": "你想修理披風嗎？",
    "pt-BR": "Gostaria de reparar uma peça de tecido?"
  },
  "npc.prose.3ca25f6159b6c0d3": {
    "en": "You can repair various kinds of Items.",
    "zh-TW": "你可以修理各種物品。",
    "pt-BR": "Você pode reparar vários tipos de itens."
  },
  "npc.prose.3ca5f5f1cdfad166": {
    "en": "2. Trust money: when it is consigned, '1,000 Gold' is paid.",
    "zh-TW": "2. 保證金：寄售時需支付 1,000 金幣。",
    "pt-BR": "2. Caução: ao consignar, pagam-se 1.000 moedas de ouro."
  },
  "npc.prose.3caeed40875c630a": {
    "en": "Grocery",
    "zh-TW": "雜貨",
    "pt-BR": "Mantimentos"
  },
  "npc.prose.3d9a714f1c15f37b": {
    "en": "This requires high concentration of Taoism and mentality.",
    "zh-TW": "這需要高度專注的道術與精神力。",
    "pt-BR": "Isso exige grande concentração taoísta e força mental."
  },
  "npc.prose.3dbafe213e200aae": {
    "en": "If situation is dangerous but you still have potions, use this",
    "zh-TW": "若處境危險但仍有藥水，可使用這個",
    "pt-BR": "Se a situação for perigosa, mas você ainda tiver poções, use isto"
  },
  "npc.prose.3dc2327eb5efe612": {
    "en": "What a crazy old fool, I would like to know told you that.",
    "zh-TW": "真是個瘋老頭，我倒想知道是誰告訴你的。",
    "pt-BR": "Que velho maluco! Gostaria de saber quem lhe contou isso."
  },
  "npc.prose.3e418cbb29d9955a": {
    "en": "Becarful the creatures are strong. If you like a challanege",
    "zh-TW": "小心，這些生物很強。若你喜歡挑戰",
    "pt-BR": "Cuidado, as criaturas são fortes. Se gosta de um desafio,"
  },
  "npc.prose.3e666964a22ee91d": {
    "en": "What item do you want to buy or sell?",
    "zh-TW": "你想購買或出售什麼物品？",
    "pt-BR": "Que item deseja comprar ou vender?"
  },
  "npc.prose.3e83796808721c61": {
    "en": "Rebirth 3 granted.",
    "zh-TW": "已授予第三次轉生。",
    "pt-BR": "Terceiro renascimento concedido."
  },
  "npc.prose.3e8f0340c7da76b0": {
    "en": "We would like to send our love to his family and friends.",
    "zh-TW": "我們向他的家人與朋友致上關愛。",
    "pt-BR": "Enviamos nosso carinho à família e aos amigos dele."
  },
  "npc.prose.3e93ab2496689625": {
    "en": "He seems to be pisoned, He has cold sweat on his face.",
    "zh-TW": "他似乎中毒了，臉上冒著冷汗。",
    "pt-BR": "Ele parece envenenado; há suor frio em seu rosto."
  },
  "npc.prose.3ec6190c5936420e": {
    "en": "many investigators. If you'd like to see this place too, you should go",
    "zh-TW": "許多調查者。若你也想看看那裡，應該去",
    "pt-BR": "muitos investigadores. Se também quiser conhecer esse lugar, vá"
  },
  "npc.prose.3f2b64cb29b2fa8c": {
    "en": "Please visit Mary inside the Inn",
    "zh-TW": "請去客棧內找瑪麗",
    "pt-BR": "Visite Mary dentro da estalagem."
  },
  "npc.prose.3f433c19c26cf057": {
    "en": "Welcome, How many i help you?",
    "zh-TW": "歡迎，有什麼能幫你的嗎？",
    "pt-BR": "Boas-vindas. Como posso ajudar?"
  },
  "npc.prose.3fbdfd1d8922f9c4": {
    "en": "(Stunned Enemies get 1.5 times more damage than when they are hit under",
    "zh-TW": "（昏迷的敵人受到的傷害是一般狀態的 1.5 倍",
    "pt-BR": "（Inimigos atordoados recebem 1,5 vez o dano que receberiam"
  },
  "npc.prose.3fc2118d352cbb01": {
    "en": "Welcome.",
    "zh-TW": "歡迎。",
    "pt-BR": "Boas-vindas."
  },
  "npc.prose.3fc4a2ce9dc5fafa": {
    "en": "Sabuk Wall Management Panel",
    "zh-TW": "沙巴克城管理面板",
    "pt-BR": "Painel de administração da muralha de Sabuk"
  },
  "npc.prose.40831494c65edc9a": {
    "en": "I can't guarrantee successfull refining each and every time.",
    "zh-TW": "我無法保證每次精煉都會成功。",
    "pt-BR": "Não posso garantir que todo refinamento tenha sucesso."
  },
  "npc.prose.409dea468917d719": {
    "en": "Hello traveller, How may I help you?",
    "zh-TW": "你好，旅人。有什麼能幫你的？",
    "pt-BR": "Olá, viajante. Como posso ajudar?"
  },
  "npc.prose.4140bc5376177651": {
    "en": "Congratulations, you Won! Enjoy your 100,000 Gold Coins.",
    "zh-TW": "恭喜，你贏了！收下這 100,000 金幣吧。",
    "pt-BR": "Parabéns, você ganhou! Aproveite suas 100.000 moedas de ouro."
  },
  "npc.prose.414bdac5566b1ab1": {
    "en": "People tend to keep away because of the temperatures.",
    "zh-TW": "人們往往因為這裡的氣溫而避開。",
    "pt-BR": "As pessoas costumam ficar longe por causa das temperaturas."
  },
  "npc.prose.417c616ecefa40ad": {
    "en": "Meat can be gained from Hens, Deer, Sheep, and Wolves.",
    "zh-TW": "雞、鹿、羊和狼都可以取得肉。",
    "pt-BR": "É possível obter carne de galinhas, cervos, ovelhas e lobos."
  },
  "npc.prose.41b86b166a00067c": {
    "en": "ElecShock (Level 13)",
    "zh-TW": "誘惑之光（13 級）",
    "pt-BR": "Choque Elétrico (nível 13)"
  },
  "npc.prose.4282ffc92170ce44": {
    "en": "Generates 2 elements if none exist. Pushes back target if",
    "zh-TW": "若沒有元素，則產生 2 個元素。將目標擊退，若",
    "pt-BR": "Gera 2 elementos se não houver nenhum. Empurra o alvo se"
  },
  "npc.prose.4298b2b4ed89591a": {
    "en": "Rumours are theres a crazy old fool inside the Tavern.",
    "zh-TW": "聽說酒館裡有個瘋老頭。",
    "pt-BR": "Dizem que há um velho maluco na taverna."
  },
  "npc.prose.42a72cdc8f3cd22e": {
    "en": "NPC didnt select a number",
    "zh-TW": "NPC 尚未選擇號碼",
    "pt-BR": "O NPC não escolheu um número."
  },
  "npc.prose.42a7620de203a7cb": {
    "en": "The Wall Funds must be transfered to your Guild Bank,",
    "zh-TW": "城牆資金必須轉入你的行會銀行，",
    "pt-BR": "Os fundos da muralha devem ser transferidos para o banco da sua guilda,"
  },
  "npc.prose.42d9609514802700": {
    "en": "the bones.",
    "zh-TW": "那些骨頭。",
    "pt-BR": "os ossos."
  },
  "npc.prose.4374c5a7a929bacc": {
    "en": "the rent will be extended for an additional 7 days.",
    "zh-TW": "租期將再延長 7 天。",
    "pt-BR": "o aluguel será prorrogado por mais 7 dias."
  },
  "npc.prose.43a29cc67ba0af12": {
    "en": "Distracts enemy in chaos to fight each other.",
    "zh-TW": "使敵人陷入混亂，互相攻擊。",
    "pt-BR": "Confunde os inimigos, fazendo-os lutar entre si."
  },
  "npc.prose.43b710cdaba4c33d": {
    "en": "30 years old I was never scared of anyone in the world Then I was",
    "zh-TW": "30 歲時，我在世上無所畏懼。當時我是",
    "pt-BR": "Aos 30 anos, eu não temia ninguém no mundo. Na época, eu era"
  },
  "npc.prose.43d23ada3bb41ead": {
    "en": "if you are going to the valley.",
    "zh-TW": "如果你打算前往山谷。",
    "pt-BR": "se pretende ir ao vale."
  },
  "npc.prose.43d46e46a5ef6d66": {
    "en": "try again later...",
    "zh-TW": "請稍後再試……",
    "pt-BR": "tente novamente mais tarde..."
  },
  "npc.prose.4406408a957a5802": {
    "en": "A special arrow that has enhances effects.",
    "zh-TW": "具有強化效果的特殊箭矢。",
    "pt-BR": "Uma flecha especial com efeitos aprimorados."
  },
  "npc.prose.440e66b5c969d352": {
    "en": "away to creatures.",
    "zh-TW": "遠離那些生物。",
    "pt-BR": "para longe das criaturas."
  },
  "npc.prose.4427fec4e3eee90b": {
    "en": "Enhances your inner force to increase its power for a certain time.",
    "zh-TW": "增強你的內力，在一段時間內提升威力。",
    "pt-BR": "Fortalece sua energia interior e aumenta seu poder por um tempo."
  },
  "npc.prose.44379d3826e18012": {
    "en": "Wall conquest war accepted.",
    "zh-TW": "攻城戰申請已受理。",
    "pt-BR": "Pedido de guerra de conquista aceito."
  },
  "npc.prose.4439bca89c0777fb": {
    "en": "Slice meat: Getting Meat from domestic animal such as chicken, Deer.",
    "zh-TW": "割肉：從雞、鹿等動物身上取得肉。",
    "pt-BR": "Cortar carne: obtenha carne de animais, como galinhas e cervos."
  },
  "npc.prose.4484ddc235636226": {
    "en": "Create a 3x3 square high temperature fire flash,",
    "zh-TW": "產生 3×3 格的高溫火焰，",
    "pt-BR": "Cria uma labareda de alta temperatura em 3×3 casas,"
  },
  "npc.prose.44a9935df313c55f": {
    "en": "are allowed in guild names.",
    "zh-TW": "可用於行會名稱。",
    "pt-BR": "são permitidos nos nomes de guildas."
  },
  "npc.prose.44fca3a1cc4fb582": {
    "en": "Items.",
    "zh-TW": "物品。",
    "pt-BR": "Itens."
  },
  "npc.prose.451533056ac10756": {
    "en": "I miss my farther..",
    "zh-TW": "我想念我的父親……",
    "pt-BR": "Sinto falta do meu pai..."
  },
  "npc.prose.458451a9f6bc388d": {
    "en": "Do you have it?",
    "zh-TW": "你帶來了嗎？",
    "pt-BR": "Você está com isso?"
  },
  "npc.prose.45885f09fc9fa6e7": {
    "en": ",  (KR)",
    "zh-TW": "， （韓國）",
    "pt-BR": ", (Coreia)"
  },
  "npc.prose.45a72f8f067d0752": {
    "en": "they hit any obstacles. Skill will be ineffective at higher level enemy.",
    "zh-TW": "撞到任何障礙物。此技能對更高等級的敵人無效。",
    "pt-BR": "atingem algum obstáculo. A habilidade não funciona contra inimigos de nível superior."
  },
  "npc.prose.4619dad8cd1e1f30": {
    "en": "Have you ever heard about the Emperor?",
    "zh-TW": "你聽說過皇帝嗎？",
    "pt-BR": "Você já ouviu falar do imperador?"
  },
  "npc.prose.466c6f0cb0ddda13": {
    "en": "Trust me, I'm looking after your stuff.",
    "zh-TW": "相信我，我會照顧好你的東西。",
    "pt-BR": "Confie em mim, estou cuidando das suas coisas."
  },
  "npc.prose.4680cf00bd52e8bf": {
    "en": "Weapon.",
    "zh-TW": "武器。",
    "pt-BR": "Arma."
  },
  "npc.prose.46834abf4c16c793": {
    "en": "Awakening Items.",
    "zh-TW": "覺醒物品。",
    "pt-BR": "Itens de despertar."
  },
  "npc.prose.468d5096bd5aad44": {
    "en": "You killed a RootSpider, didn't you? Good job!",
    "zh-TW": "你殺死了一隻樹妖蜘蛛，對吧？做得好！",
    "pt-BR": "Você matou uma Aranha-Raiz, não é? Muito bem!"
  },
  "npc.prose.46a46351c5d72972": {
    "en": "I have been waiting for you a long time..",
    "zh-TW": "我已等你很久了……",
    "pt-BR": "Estou esperando você há muito tempo..."
  },
  "npc.prose.48b660b3187b96e8": {
    "en": "This alcohol tastes great...",
    "zh-TW": "這酒真好喝……",
    "pt-BR": "Esta bebida é ótima..."
  },
  "npc.prose.493d7dad0dd1225c": {
    "en": "Apply Rebirth          Delete Rebirth             Check Rebirth",
    "zh-TW": "套用轉生　　刪除轉生　　查看轉生",
    "pt-BR": "Aplicar renascimento  Remover renascimento  Consultar renascimento"
  },
  "npc.prose.49916400df06962c": {
    "en": "filled with Sin, Fear and Shame. Are you sure you want to enter into",
    "zh-TW": "充滿罪惡、恐懼與羞愧。你確定要進入",
    "pt-BR": "cheio de pecado, medo e vergonha. Tem certeza de que quer entrar em"
  },
  "npc.prose.49fb10c40c3c9c9a": {
    "en": "How can I trust you? I don't even know who you are.",
    "zh-TW": "我怎能相信你？我甚至不知道你是誰。",
    "pt-BR": "Como posso confiar em você? Nem sei quem você é."
  },
  "npc.prose.4a124ca1eac9009a": {
    "en": "Its duration will be extended according to practice levels and SC power.",
    "zh-TW": "持續時間會隨熟練等級與道術提升而延長。",
    "pt-BR": "A duração aumenta de acordo com o nível de prática e o poder espiritual."
  },
  "npc.prose.4a2a8c48954231ab": {
    "en": "But anger and loathing can not beat demonic power. The minuet",
    "zh-TW": "但憤怒與憎恨無法戰勝魔力。就在那一刻",
    "pt-BR": "Mas raiva e ódio não vencem o poder demoníaco. No instante em que"
  },
  "npc.prose.4a7120ebfc7b7f82": {
    "en": "Attack two people straight in a row. Click to activate or deactivate the skill.",
    "zh-TW": "攻擊前方一直線的兩個目標。點擊可啟用或停用技能。",
    "pt-BR": "Ataca dois alvos alinhados à frente. Clique para ativar ou desativar a habilidade."
  },
  "npc.prose.4abd2bc1db49cb28": {
    "en": "To extend the rental period of this territory for 7 additional days.",
    "zh-TW": "將這片領地的租期再延長 7 天。",
    "pt-BR": "Para prorrogar o aluguel deste território por mais 7 dias."
  },
  "npc.prose.4ad2991f0468debb": {
    "en": "The empty lot across the bridge is the site",
    "zh-TW": "橋對面的空地就是場地",
    "pt-BR": "O terreno vazio do outro lado da ponte é o local"
  },
  "npc.prose.4ae03039febb17ef": {
    "en": "You seem to be interested in an old tale.",
    "zh-TW": "你似乎對古老傳說很有興趣。",
    "pt-BR": "Você parece interessado em uma história antiga."
  },
  "npc.prose.4b3f4cb5537841b0": {
    "en": "Creates a shield which protects damage from monsters.",
    "zh-TW": "建立護盾，減輕怪物造成的傷害。",
    "pt-BR": "Cria um escudo que protege contra o dano dos monstros."
  },
  "npc.prose.4ba67e803bc04814": {
    "en": "Would you like to ?",
    "zh-TW": "你想要嗎？",
    "pt-BR": "Gostaria de fazer isso?"
  },
  "npc.prose.4c0031076df57b22": {
    "en": "The HolySword filled with taoist power injured RedMoonEvil fatally.",
    "zh-TW": "蘊含道術的聖劍重創了赤月惡魔。",
    "pt-BR": "A Espada Sagrada, carregada de poder taoísta, feriu mortalmente o Demônio da Lua Vermelha."
  },
  "npc.prose.4c36c68649c2a85f": {
    "en": "Production (Craft & Upgrade)",
    "zh-TW": "製作（打造與升級）",
    "pt-BR": "Produção (fabricação e melhoria)"
  },
  "npc.prose.4c3a36b5558b0f4c": {
    "en": "It may sound simple,but as a matter of fact,very difficult.",
    "zh-TW": "聽起來簡單，實際上卻非常困難。",
    "pt-BR": "Pode parecer simples, mas na verdade é muito difícil."
  },
  "npc.prose.4c467540524a961d": {
    "en": "Hello Traveller. What can I do for you?",
    "zh-TW": "你好，旅人。有什麼能為你效勞？",
    "pt-BR": "Olá, viajante. Como posso ajudar?"
  },
  "npc.prose.4c602e0916e90a50": {
    "en": "Teleport to  ,  ,",
    "zh-TW": "傳送至　、　、",
    "pt-BR": "Teleportar para  ,  ,"
  },
  "npc.prose.4d1019f0989c0a9b": {
    "en": "Next Sabuk Wall - {npc_arg0}",
    "zh-TW": "下次沙巴克攻城－{npc_arg0}",
    "pt-BR": "Próxima disputa por Sabuk — {npc_arg0}"
  },
  "npc.prose.4d4e7086cb61b079": {
    "en": "I want to  sale of my guild territory.",
    "zh-TW": "我想處理行會領地的出售。",
    "pt-BR": "Quero tratar da venda do território da minha guilda."
  },
  "npc.prose.4d5be2bfdfad233a": {
    "en": "them, the government approve  guild war.",
    "zh-TW": "他們之間，政府批准了行會戰爭。",
    "pt-BR": "elas, o governo autoriza a guerra de guildas."
  },
  "npc.prose.4dc603d1b951a828": {
    "en": "Archer One                         Archer Four",
    "zh-TW": "弓箭手一　　弓箭手四",
    "pt-BR": "Arqueiro Um  Arqueiro Quatro"
  },
  "npc.prose.4def6462ab0ce4b0": {
    "en": "Will you trade?",
    "zh-TW": "你願意交易嗎？",
    "pt-BR": "Deseja negociar?"
  },
  "npc.prose.4df0d3402fe80f54": {
    "en": "I am the Administrator. How may I help you ?",
    "zh-TW": "我是管理員。有什麼能為你效勞？",
    "pt-BR": "Sou o administrador. Como posso ajudar?"
  },
  "npc.prose.4e0287b7e9d3e6d1": {
    "en": "Medicinal stuff: You can acquire some medicinal stuff from dead body of",
    "zh-TW": "藥材：你可以從屍體上取得一些藥材",
    "pt-BR": "Materiais medicinais: você pode obter alguns nos corpos de"
  },
  "npc.prose.4e0dd9a27b1223e1": {
    "en": "It is a chance to prove your true power and",
    "zh-TW": "這是證明你真正實力的機會，也能",
    "pt-BR": "É uma chance de provar seu verdadeiro poder e"
  },
  "npc.prose.4e2841b748e25801": {
    "en": "Bring Candles, Town Teleports and enough potions.",
    "zh-TW": "請帶上蠟燭、回城卷及足夠的藥水。",
    "pt-BR": "Leve velas, pergaminhos de retorno e poções suficientes."
  },
  "npc.prose.4e56591287a9eb3b": {
    "en": "Welcome {npc_arg0}, What can i do for you?",
    "zh-TW": "歡迎，{npc_arg0}。有什麼能為你效勞？",
    "pt-BR": "Boas-vindas, {npc_arg0}. Como posso ajudar?"
  },
  "npc.prose.4e9721699459a6dc": {
    "en": "Increase the chance of gathering elements whilst active.",
    "zh-TW": "啟用時提高收集元素的機率。",
    "pt-BR": "Aumenta a chance de reunir elementos enquanto estiver ativa."
  },
  "npc.prose.4e9a168d418e146a": {
    "en": "Summons a powerful fire breathing beast called Shinsu that will act as",
    "zh-TW": "召喚能噴火的強大獸類神獸，牠將作為",
    "pt-BR": "Invoca uma fera poderosa que cospe fogo, chamada Shinsu, que atuará como"
  },
  "npc.prose.4eecef13ae01b61b": {
    "en": "Which place would you like to go?",
    "zh-TW": "你想去哪裡？",
    "pt-BR": "Aonde gostaria de ir?"
  },
  "npc.prose.2e38e77b22c314a4": {
    "en": "()",
    "zh-TW": "()",
    "pt-BR": "()",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.3973e022e93220f9": {
    "en": "-",
    "zh-TW": "-",
    "pt-BR": "-",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.4ef6883c669b0b72": {
    "en": "ask their leader.",
    "zh-TW": "詢問他們的首領。",
    "pt-BR": "pergunte ao líder deles."
  },
  "npc.prose.4f19c681a8c87275": {
    "en": "which can help on your travels?",
    "zh-TW": "能在旅途中幫上忙的東西？",
    "pt-BR": "algo que possa ajudar em suas viagens?"
  },
  "npc.prose.4f3381e9c9c1dcf0": {
    "en": "Increases physical attack.",
    "zh-TW": "提高物理攻擊。",
    "pt-BR": "Aumenta o ataque físico."
  },
  "npc.prose.4f3541b8d75dbade": {
    "en": "Level 30: Thunderstorm",
    "zh-TW": "30 級：地獄雷光",
    "pt-BR": "Nível 30: Tempestade de Raios"
  },
  "npc.prose.4f5194c1e29559b5": {
    "en": "Hello I'm Chandler, the wandering warrior.",
    "zh-TW": "你好，我是錢德勒，一名流浪戰士。",
    "pt-BR": "Olá, sou Chandler, um guerreiro errante."
  },
  "npc.prose.4f59b71b6aa05135": {
    "en": "Archer Three                       Archer Six",
    "zh-TW": "弓箭手三　　弓箭手六",
    "pt-BR": "Arqueiro Três  Arqueiro Seis"
  },
  "npc.prose.4fe7f7a791afd6de": {
    "en": "Looks like these bones been laying",
    "zh-TW": "看來這些骨頭已經躺在這裡",
    "pt-BR": "Parece que estes ossos estão aqui"
  },
  "npc.prose.4fff6cd92bdce027": {
    "en": "Trap a mob - confining it from the outer world.",
    "zh-TW": "困住怪物，將牠與外界隔離。",
    "pt-BR": "Aprisiona um monstro, isolando-o do mundo exterior."
  },
  "npc.prose.50026eec5318cdde": {
    "en": "Create a familiar by making his shape through strong magic and resurrecting",
    "zh-TW": "運用強大的魔法塑造形體，並使其復甦來創造使魔",
    "pt-BR": "Cria um familiar, moldando sua forma com magia poderosa e ressuscitando"
  },
  "npc.prose.5011abeb870ca202": {
    "en": "Reveals other HP (players or mobs).",
    "zh-TW": "顯示其他玩家或怪物的生命值。",
    "pt-BR": "Revela os PV de outros jogadores ou monstros."
  },
  "npc.prose.508c23fbb9a7b669": {
    "en": "A Mysterious Stone. With Ancient symbols.",
    "zh-TW": "一塊神祕石頭，刻有古老符號。",
    "pt-BR": "Uma pedra misteriosa com símbolos antigos."
  },
  "npc.prose.50909fcd1b67f9f8": {
    "en": "Moreover, be ready with enough potions since it is located",
    "zh-TW": "此外，記得備足藥水，因為它位於",
    "pt-BR": "Além disso, prepare poções suficientes, pois fica"
  },
  "npc.prose.5094d6d37ecf27b7": {
    "en": "Choose which Item you wish to Reset.",
    "zh-TW": "選擇你想重置的物品。",
    "pt-BR": "Escolha o item que deseja redefinir."
  },
  "npc.prose.50a16ec59ea15054": {
    "en": "start 2 days later.",
    "zh-TW": "將於 2 天後開始。",
    "pt-BR": "começará 2 dias depois."
  },
  "npc.prose.50d89cb3f78ab301": {
    "en": "This could be used for starting global events and other stuff as the",
    "zh-TW": "此功能可用來啟動全服活動及其他功能，隨著",
    "pt-BR": "Isso pode ser usado para iniciar eventos globais e outras atividades conforme"
  },
  "npc.prose.50e972fa1711a96a": {
    "en": "Powerful hit on enemy.",
    "zh-TW": "對敵人造成強力一擊。",
    "pt-BR": "Desfere um golpe poderoso no inimigo."
  },
  "npc.prose.50fc7570f6479d54": {
    "en": "His skill is ineffective at higher level enemy.",
    "zh-TW": "此技能對更高等級的敵人無效。",
    "pt-BR": "Esta habilidade não funciona contra inimigos de nível superior."
  },
  "npc.prose.51000e80de25da7f": {
    "en": "Get out of here please",
    "zh-TW": "請離開這裡",
    "pt-BR": "Por favor, saia daqui."
  },
  "npc.prose.5165779cf8b872ee": {
    "en": "The combat abillity will be increased, so try Oma, ForestYeti, CannibalPlant,",
    "zh-TW": "戰鬥能力提升後，可以挑戰半獸人、森林雪人、食人花，",
    "pt-BR": "Quando sua capacidade de combate aumentar, enfrente Omas, Yetis da Floresta, Plantas Carnívoras,"
  },
  "npc.prose.51b5d2be286759e8": {
    "en": "where privacy is still not guaranteed. Guild members may",
    "zh-TW": "仍無法保障隱私的地方。行會成員可以",
    "pt-BR": "onde a privacidade ainda não é garantida. Os membros da guilda podem"
  },
  "npc.prose.52836af0fd1b92fb": {
    "en": "You do not have Rebirth 3.",
    "zh-TW": "你尚未完成第三次轉生。",
    "pt-BR": "Você não possui o terceiro renascimento."
  },
  "npc.prose.52950fcd5eda83ff": {
    "en": "about the Bottomless Pit",
    "zh-TW": "關於無底深淵",
    "pt-BR": "sobre o Poço sem Fundo"
  },
  "npc.prose.52c55df8ffa5fa4a": {
    "en": "I have specialized in Weaponry for many year's now.",
    "zh-TW": "我鑽研武器已經很多年了。",
    "pt-BR": "Sou especialista em armas há muitos anos."
  },
  "npc.prose.52e9710949e54857": {
    "en": "SabukWall Tax Income:                   SabukWall NPC Tax Rate:",
    "zh-TW": "沙巴克稅收收入：　沙巴克 NPC 稅率：",
    "pt-BR": "Receita tributária de Sabuk:  Taxa dos NPCs de Sabuk:"
  },
  "npc.prose.52f6741ad5480c55": {
    "en": "PrajnaHeart Required Level48~55.",
    "zh-TW": "般若之心，適合 48～55 級。",
    "pt-BR": "Coração de Prajna: níveis exigidos de 48 a 55."
  },
  "npc.prose.530813827c7261c4": {
    "en": "If you are a Wizard who learned Hellfire skill,",
    "zh-TW": "若你是學會地獄火的法師，",
    "pt-BR": "Se você é um mago que aprendeu Fogo Infernal,"
  },
  "npc.prose.534b4aff269cc593": {
    "en": "have passed. Many soldiers were killed in action due to",
    "zh-TW": "已經過去了。許多士兵陣亡，原因是",
    "pt-BR": "se passaram. Muitos soldados morreram em combate devido a"
  },
  "npc.prose.535016d25a39f60b": {
    "en": "Materials.",
    "zh-TW": "材料。",
    "pt-BR": "Materiais."
  },
  "npc.prose.53510e0394449b45": {
    "en": "Boot",
    "zh-TW": "靴子",
    "pt-BR": "Bota"
  },
  "npc.prose.539e8a27a1e351e6": {
    "en": "All you have to do is guess what number I'm thinking",
    "zh-TW": "你只要猜出我心裡想的號碼",
    "pt-BR": "Você só precisa adivinhar o número em que estou pensando."
  },
  "npc.prose.539ed189a52b721b": {
    "en": "Soo many Monsters.. I hate Monsters, I wish they would all just die!",
    "zh-TW": "好多怪物……我討厭怪物，希望牠們全部死光！",
    "pt-BR": "Tantos monstros... Odeio monstros. Queria que todos morressem!"
  },
  "npc.prose.53c1044dc9d410a0": {
    "en": "You may lose the items you are equipped with.",
    "zh-TW": "你可能會失去身上裝備的物品。",
    "pt-BR": "Você pode perder os itens que está usando."
  },
  "npc.prose.53e5eb3e97c1e946": {
    "en": "The wine that was made with a Red Snake and Tiger Snake",
    "zh-TW": "以紅蛇和虎蛇釀製的酒",
    "pt-BR": "O vinho feito com Cobra Vermelha e Cobra-Tigre"
  },
  "npc.prose.54752214f1d1ad9b": {
    "en": "The RedMoonSword led me to wander over RedMoon Valley and",
    "zh-TW": "赤月劍引領我在赤月峽谷遊蕩，並",
    "pt-BR": "A Espada da Lua Vermelha me levou a vagar pelo Vale da Lua Vermelha e"
  },
  "npc.prose.54d7ffcc9fb4d283": {
    "en": "According to the former Rev.taoist of this school, the person who",
    "zh-TW": "據本門前任道長所言，那位",
    "pt-BR": "Segundo o antigo reverendo taoísta desta escola, a pessoa que"
  },
  "npc.prose.54f1983cd58e4607": {
    "en": "I am the Guild Territory Merchant and",
    "zh-TW": "我是行會領地商人，並且",
    "pt-BR": "Sou o comerciante de territórios de guilda e"
  },
  "npc.prose.54f203e5a083c273": {
    "en": "all my life. If you want to know anything about the",
    "zh-TW": "一生如此。若你想了解任何關於",
    "pt-BR": "durante toda a minha vida. Se quiser saber algo sobre"
  },
  "npc.prose.5503d28456dc1a3c": {
    "en": "for sale",
    "zh-TW": "出售中",
    "pt-BR": "à venda"
  },
  "npc.prose.551a343f99e485a5": {
    "en": "I understand your curiosity but if you are killed there,",
    "zh-TW": "我理解你的好奇心，但若你死在那裡，",
    "pt-BR": "Entendo sua curiosidade, mas, se você morrer lá,"
  },
  "npc.prose.555ededec60f3895": {
    "en": "enough, you can't root anymore.",
    "zh-TW": "足夠時，就不能再撿取了。",
    "pt-BR": "demais, você não poderá coletar mais."
  },
  "npc.prose.558bb400263a887c": {
    "en": "the swamp.",
    "zh-TW": "沼澤。",
    "pt-BR": "o pântano."
  },
  "npc.prose.558f723462dc6f20": {
    "en": "Leave us alone, please!",
    "zh-TW": "拜託，別來打擾我們！",
    "pt-BR": "Por favor, deixem-nos em paz!"
  },
  "npc.prose.559fa4f1e8fea60d": {
    "en": "Mining: Equip PickAxe to mine. Left click will automatically of mining.",
    "zh-TW": "採礦：裝備鶴嘴鋤，左鍵點擊即可自動採礦。",
    "pt-BR": "Mineração: equipe uma picareta. Clique com o botão esquerdo para minerar automaticamente."
  },
  "npc.prose.55c97073bd35e4fe": {
    "en": "TigerSnake, and in SerpentValley and OmaFighter.",
    "zh-TW": "虎蛇，以及毒蛇山谷中的半獸人戰士。",
    "pt-BR": "Cobras-Tigre e Combatentes Oma no Vale das Serpentes."
  },
  "npc.prose.55d5ba19871908d4": {
    "en": "anger... uncontrollably the rough heartbeasts of mad and murderous",
    "zh-TW": "憤怒……瘋狂與殺意的心跳不受控制地",
    "pt-BR": "raiva... as fortes batidas do coração, cheias de loucura e violência, sem controle"
  },
  "npc.prose.56007cdf8e5d73ab": {
    "en": "ready for me?",
    "zh-TW": "替我準備好了嗎？",
    "pt-BR": "está pronto para mim?"
  },
  "npc.prose.5620bcee92a9d355": {
    "en": "In short, it is a residence for clans.",
    "zh-TW": "簡單來說，這是行會的住所。",
    "pt-BR": "Em poucas palavras, é uma residência para clãs."
  },
  "npc.prose.568cea2481979558": {
    "en": "{npc_arg0} gold is required.",
    "zh-TW": "需要 {npc_arg0} 金幣。",
    "pt-BR": "São necessárias {npc_arg0} moedas de ouro."
  },
  "npc.prose.5694ee0add6d0cbb": {
    "en": "A Wizard's Thunderbolt is very effective in this place.",
    "zh-TW": "法師的雷電術在這裡非常有效。",
    "pt-BR": "O Relâmpago dos magos é muito eficaz aqui."
  },
  "npc.prose.56a9dc4a550bfe78": {
    "en": "It's your destiny you never escape from it even after you die....",
    "zh-TW": "這就是你的命運，即使死後也無法逃脫……",
    "pt-BR": "É seu destino; nem a morte permitirá que escape dele..."
  },
  "npc.prose.570f85b8a5b75541": {
    "en": "Try one of my sisters, MissRe or MissMi",
    "zh-TW": "去找我的姊妹芮小姐或米小姐試試",
    "pt-BR": "Procure uma das minhas irmãs, a senhorita Re ou a senhorita Mi."
  },
  "npc.prose.573db38968c18a48": {
    "en": "Damage 4 enemies at once near by. MP will be consumed.",
    "zh-TW": "同時傷害附近 4 個敵人，會消耗魔力。",
    "pt-BR": "Causa dano a 4 inimigos próximos ao mesmo tempo. Consome PM."
  },
  "npc.prose.575b7b9ef127c7bc": {
    "en": "and my nickname was Thunderman before i died. Originally I had",
    "zh-TW": "生前，我的綽號是雷霆人。原本我曾",
    "pt-BR": "e meu apelido antes de morrer era Homem-Trovão. Originalmente, eu tinha"
  },
  "npc.prose.578b48f060cb7cd3": {
    "en": "and 10 Continental Heroes, who brought the glory to",
    "zh-TW": "以及帶來榮耀的十大陸地英雄",
    "pt-BR": "e os 10 Heróis Continentais, que trouxeram glória a"
  },
  "npc.prose.5808a8df78dd0766": {
    "en": "I have heard that there are  which monsters",
    "zh-TW": "我聽說那裡有怪物",
    "pt-BR": "Ouvi dizer que há monstros"
  },
  "npc.prose.5810c5e9cd99f7c6": {
    "en": "Conquest Intrest Rate: {npc_arg0}",
    "zh-TW": "攻城利率：{npc_arg0}",
    "pt-BR": "Taxa de juros da conquista: {npc_arg0}"
  },
  "npc.prose.581cdaffcdd1c5fa": {
    "en": "Island and Past Bichon, the item sales information are",
    "zh-TW": "島與昔日比奇，物品銷售資訊都",
    "pt-BR": "Ilha e Bichon do Passado; as informações de venda dos itens são"
  },
  "npc.prose.582cc6aeb8a545b8": {
    "en": "If you complete a task for me, I shall escort you to the Emperor!",
    "zh-TW": "若你替我完成一件事，我就護送你去見皇帝！",
    "pt-BR": "Se cumprir uma tarefa para mim, eu acompanharei você até o imperador!"
  },
  "npc.prose.584f860500271e40": {
    "en": "So it is suggested to clean up the area and cast the skill with alliance protection.",
    "zh-TW": "所以建議先清空周圍區域，再於盟友保護下施法。",
    "pt-BR": "Por isso, recomenda-se limpar a área e usar a habilidade sob a proteção dos aliados."
  },
  "npc.prose.5871afe0067f14a2": {
    "en": "Pull one enemy nearby.",
    "zh-TW": "將一名敵人拉到身旁。",
    "pt-BR": "Puxa um inimigo para perto."
  },
  "npc.prose.58b9549472dcdfcf": {
    "en": "Well you have proven yourself to me.",
    "zh-TW": "嗯，你已經向我證明了實力。",
    "pt-BR": "Bem, você já provou seu valor para mim."
  },
  "npc.prose.593a1943d8de8fa2": {
    "en": "The arena is a place to test your own power only",
    "zh-TW": "競技場只能用來考驗你自己的力量",
    "pt-BR": "A arena é um lugar para testar apenas a sua própria força."
  },
  "npc.prose.5951a2e2ae299f8f": {
    "en": "You can repair various kinds of Torch's.",
    "zh-TW": "你可以修理各種火把。",
    "pt-BR": "Você pode reparar vários tipos de tochas."
  },
  "npc.prose.59617073f586aef5": {
    "en": "Giving hundreds around the globe enjoyment of a beloved game,",
    "zh-TW": "讓全球數百人享受這款心愛的遊戲，",
    "pt-BR": "Proporcionando a centenas de pessoas pelo mundo a diversão de um jogo querido,"
  },
  "npc.prose.596ecb8e1b5e7dba": {
    "en": "-Liam Thompson",
    "zh-TW": "－Liam Thompson",
    "pt-BR": "— Liam Thompson"
  },
  "npc.prose.59d387af9f283d36": {
    "en": "I'm still waiting to hear back from the castle..",
    "zh-TW": "我還在等待城堡的回覆……",
    "pt-BR": "Ainda estou esperando notícias do castelo..."
  },
  "npc.prose.59de4bcd93f4ff1e": {
    "en": "You have the forbidden Orb...\"",
    "zh-TW": "你持有那顆禁忌寶珠……」",
    "pt-BR": "Você está com o Orbe Proibido...”"
  },
  "npc.prose.59f8e20e0def0e7a": {
    "en": "bookstore in town. High level skill book will be acquired during monster hunt.)",
    "zh-TW": "城裡的書店。高階技能書可在狩獵怪物時取得。）",
    "pt-BR": "na livraria da cidade. Livros de habilidades avançadas podem ser obtidos ao caçar monstros.)"
  },
  "npc.prose.5a1e3175182b007e": {
    "en": "*Palace  *Competing room",
    "zh-TW": "＊皇宮　＊競技室",
    "pt-BR": "*Palácio  *Sala de competição"
  },
  "npc.prose.5a1fb63cc72c9331": {
    "en": "Welcome Traveller, my name is Bill I'm a Master Artisan..",
    "zh-TW": "歡迎，旅人。我叫比爾，是一位工藝大師……",
    "pt-BR": "Boas-vindas, viajante. Meu nome é Bill, sou um mestre artesão..."
  },
  "npc.prose.5a545bfb5ec2bbd4": {
    "en": "I can take you to the Dark Swamp.",
    "zh-TW": "我可以帶你前往黑暗沼澤。",
    "pt-BR": "Posso levar você ao Pântano Sombrio."
  },
  "npc.prose.5ab2a48b50d3bc93": {
    "en": "Start hunting around the town and move further.",
    "zh-TW": "先在城鎮周圍狩獵，再逐漸往遠處前進。",
    "pt-BR": "Comece caçando perto da cidade e vá se afastando aos poucos."
  },
  "npc.prose.5afedb5aecf59dce": {
    "en": "\"Have you seen this Language ?\"",
    "zh-TW": "「你看過這種語言嗎？」",
    "pt-BR": "“Você já viu esta língua?”"
  },
  "npc.prose.5b09754848231db9": {
    "en": "But be specially aware of poison from CaveMaggot. You mat be paralyzed.",
    "zh-TW": "但要特別小心洞蛆的毒，可能會讓你麻痺。",
    "pt-BR": "Mas tome cuidado com o veneno dos vermes de caverna: ele pode paralisar você."
  },
  "npc.prose.5b24876c37f133e4": {
    "en": "News:",
    "zh-TW": "消息：",
    "pt-BR": "Notícias:"
  },
  "npc.prose.5b2b671238d0dcd5": {
    "en": "others, which naturally makes them excellent at making fast kills. It is necessary",
    "zh-TW": "他人，因此他們十分擅長迅速擊殺。必須",
    "pt-BR": "os outros, o que os torna excelentes em abates rápidos. É necessário"
  },
  "npc.prose.5b4425071e5ca386": {
    "en": "WhiteValley is new fontier which has just been discovered.",
    "zh-TW": "白色山谷是最近才被發現的新邊境。",
    "pt-BR": "O Vale Branco é uma nova fronteira, descoberta recentemente."
  },
  "npc.prose.5b6fd48cff93dac1": {
    "en": "I'm not aware.",
    "zh-TW": "我不清楚。",
    "pt-BR": "Não tenho conhecimento disso."
  },
  "npc.prose.5b6fed7b4257c6da": {
    "en": "Choose which Item you wish to Downgrade.",
    "zh-TW": "選擇你想降級的物品。",
    "pt-BR": "Escolha o item que deseja rebaixar."
  },
  "npc.prose.5b8f4cb2b80c1fac": {
    "en": "You seem to be interested in an old tale. Ok.",
    "zh-TW": "你似乎對古老傳說很有興趣。好吧。",
    "pt-BR": "Você parece interessado em uma história antiga. Está bem."
  },
  "npc.prose.5bee41d4fb02895a": {
    "en": "where stone statues of the top 10 World Heroes",
    "zh-TW": "那裡有世界十大英雄的石像",
    "pt-BR": "onde estão as estátuas de pedra dos 10 maiores Heróis do Mundo"
  },
  "npc.prose.5c44389687ad53a0": {
    "en": "The damage inflicted raises with the skill level.",
    "zh-TW": "造成的傷害隨技能等級提高。",
    "pt-BR": "O dano causado aumenta com o nível da habilidade."
  },
  "npc.prose.5c46d5c2a07ea7db": {
    "en": "TownTeleport scrolls but it could still save your life.",
    "zh-TW": "回城卷，但仍然可能救你一命。",
    "pt-BR": "pergaminhos de retorno, mas ainda pode salvar sua vida."
  },
  "npc.prose.5c48ce6b2675e1b0": {
    "en": "Level 17: Thunderbolt",
    "zh-TW": "17 級：雷電術",
    "pt-BR": "Nível 17: Raio"
  },
  "npc.prose.5c8243e31f4b1398": {
    "en": "Bounty Board.",
    "zh-TW": "懸賞告示板。",
    "pt-BR": "Quadro de recompensas."
  },
  "npc.prose.5ca2d470fee0e486": {
    "en": "crafting items or for selling.",
    "zh-TW": "用於製作物品或出售。",
    "pt-BR": "para fabricar itens ou vender."
  },
  "npc.prose.5d0f88d9daa0b781": {
    "en": "in a flash. It's also useful for taming monsters.",
    "zh-TW": "轉瞬之間。也可用來馴服怪物。",
    "pt-BR": "em um instante. Também é útil para domar monstros."
  },
  "npc.prose.5d35fd24a5174b37": {
    "en": "village or inside contest area (like inside some buildings).",
    "zh-TW": "村莊內或競技區域內（例如某些建築中）。",
    "pt-BR": "dentro da vila ou de áreas de competição, como certos edifícios."
  },
  "npc.prose.5d3ede66195e45bc": {
    "en": "If you came to me again at this time",
    "zh-TW": "若你此時再次來找我",
    "pt-BR": "Se você voltar a me procurar neste momento,"
  },
  "npc.prose.5d96edee2c823cf8": {
    "en": "If it matches with your job and level,",
    "zh-TW": "若符合你的職業與等級，",
    "pt-BR": "Se corresponder à sua classe e ao seu nível,"
  },
  "npc.prose.5d9925c136e855db": {
    "en": "They're capable of hiding themselves and performing attacks while being unseen by",
    "zh-TW": "他們能隱藏身形，並在不被發現時發動攻擊",
    "pt-BR": "Eles conseguem se esconder e atacar sem serem vistos por"
  },
  "npc.prose.5e04a3c7624dd93a": {
    "en": "Which necklace do you want to buy or sell?",
    "zh-TW": "你想購買或出售哪條項鍊？",
    "pt-BR": "Qual colar deseja comprar ou vender?"
  },
  "npc.prose.5e2a8ae43155224c": {
    "en": "May he Rest In Peace <3",
    "zh-TW": "願他安息。♡",
    "pt-BR": "Que ele descanse em paz. <3"
  },
  "npc.prose.5e3f953f88145839": {
    "en": "Murder. Fighting and slaying for over 100 years when i faught against",
    "zh-TW": "殺戮。戰鬥與殺戮持續了百餘年，直到我對上",
    "pt-BR": "Assassinato. Lutei e matei por mais de 100 anos, até enfrentar"
  },
  "npc.prose.5ef932484e530695": {
    "en": "Here the guild leader can change the Sabuk Tax!",
    "zh-TW": "行會會長可在此更改沙巴克稅率！",
    "pt-BR": "Aqui o líder da guilda pode alterar o imposto de Sabuk!"
  },
  "npc.prose.5f152e58fa297d8b": {
    "en": "all monsters nearby and attack with a chance to paralysis",
    "zh-TW": "所有附近的怪物，並有機率造成麻痺",
    "pt-BR": "todos os monstros próximos e ataca com chance de paralisar"
  },
  "npc.prose.5f4362073a44f7e0": {
    "en": "He always hangs out near the book store.",
    "zh-TW": "他總是在書店附近閒逛。",
    "pt-BR": "Ele sempre fica perto da livraria."
  },
  "npc.prose.5f4d07e19c61bfb1": {
    "en": "I came here with the sword but... It seems to have been the first",
    "zh-TW": "我帶著劍來到這裡，但……那似乎是第一次",
    "pt-BR": "Vim aqui com a espada, mas... Parece ter sido a primeira"
  },
  "npc.prose.5f82d6c9186794b2": {
    "en": "of the village",
    "zh-TW": "村莊的",
    "pt-BR": "da vila"
  },
  "npc.prose.610ffab95843d76d": {
    "en": "Sorry, I dont have anything to tell you.",
    "zh-TW": "抱歉，我沒有什麼能告訴你。",
    "pt-BR": "Desculpe, não tenho nada para contar."
  },
  "npc.prose.613a8e362f744674": {
    "en": "The story is over, now get out of here.",
    "zh-TW": "故事說完了，現在離開吧。",
    "pt-BR": "A história acabou. Agora vá embora."
  },
  "npc.prose.6194658d1cc89ca5": {
    "en": "If you wish for a safe hunt, try RedSnake,",
    "zh-TW": "若想安全地狩獵，可以挑戰紅蛇，",
    "pt-BR": "Para uma caçada segura, procure Cobras Vermelhas,"
  },
  "npc.prose.61db4aed2deb5e92": {
    "en": "The Bones.. They look freshly eaten.",
    "zh-TW": "那些骨頭……看起來剛被啃食過。",
    "pt-BR": "Os ossos... Parecem ter sido roídos há pouco."
  },
  "npc.prose.622c23f60da176b6": {
    "en": "Long distance flaming attack which is basic skill of Wizard as a beginner.",
    "zh-TW": "遠距離火焰攻擊，是初學法師的基本技能。",
    "pt-BR": "Ataque de fogo à distância, uma habilidade básica de magos iniciantes."
  },
  "npc.prose.6248ad41f4f9bfaf": {
    "en": "Please select the Egg you want to buy.",
    "zh-TW": "請選擇你想購買的蛋。",
    "pt-BR": "Selecione o ovo que deseja comprar."
  },
  "npc.prose.62743a8c230307de": {
    "en": "Ah your the traveler.. Ive been hearing about.",
    "zh-TW": "啊，你就是那位旅人……我聽過你的消息。",
    "pt-BR": "Ah, você é o viajante... de quem tenho ouvido falar."
  },
  "npc.prose.6296853811706c2f": {
    "en": "Guild territory is an exclusive space for a guild,",
    "zh-TW": "行會領地是行會專屬的空間，",
    "pt-BR": "Um território de guilda é um espaço exclusivo da guilda,"
  },
  "npc.prose.62bd3fb1d553ef5f": {
    "en": "I can send you several places nearby.",
    "zh-TW": "我能送你去附近幾個地方。",
    "pt-BR": "Posso enviar você a vários lugares próximos."
  },
  "npc.prose.63517a10e035d7c4": {
    "en": "Fire double arrows at your target.",
    "zh-TW": "向目標射出雙箭。",
    "pt-BR": "Dispara duas flechas contra o alvo."
  },
  "npc.prose.636faaf6896b7d40": {
    "en": "Level 35: IceStrom",
    "zh-TW": "35 級：冰咆哮",
    "pt-BR": "Nível 35: Tempestade de Gelo"
  },
  "npc.prose.63805854c72d5b05": {
    "en": "Allows gathering of elements when attacking monsters.",
    "zh-TW": "攻擊怪物時可收集元素。",
    "pt-BR": "Permite reunir elementos ao atacar monstros."
  },
  "npc.prose.63ea0d5f9f0dfa38": {
    "en": "=== Special Bounty ===",
    "zh-TW": "＝＝＝特殊懸賞＝＝＝",
    "pt-BR": "=== Recompensa especial ==="
  },
  "npc.prose.64420e194da1b154": {
    "en": "MP will continously be consumed while the image is in use.",
    "zh-TW": "影像存在期間會持續消耗魔力。",
    "pt-BR": "Consome PM continuamente enquanto a imagem estiver em uso."
  },
  "npc.prose.64592b2a404b2ff4": {
    "en": "But be specially aware of the poison from CaveMaggots. You may be paralyzed.",
    "zh-TW": "但要特別小心洞蛆的毒，可能會讓你麻痺。",
    "pt-BR": "Mas tome cuidado com o veneno dos vermes de caverna: ele pode paralisar você."
  },
  "npc.prose.645c927a51faf2c5": {
    "en": "The HolyWeapon... I would never believe it.",
    "zh-TW": "聖兵……我絕不相信。",
    "pt-BR": "A Arma Sagrada... Eu jamais acreditaria nisso."
  },
  "npc.prose.64c1658aeb1aa495": {
    "en": "Hellfire (Level 16)",
    "zh-TW": "地獄火（16 級）",
    "pt-BR": "Fogo Infernal (nível 16)"
  },
  "npc.prose.651576b26ad126d4": {
    "en": "Oh... how can someone be so mighty...",
    "zh-TW": "喔……怎麼會有人如此強大……",
    "pt-BR": "Oh... como alguém pode ser tão poderoso...?"
  },
  "npc.prose.6529ac1dc9e2f25c": {
    "en": "The skill book will be dissappeared showing the message.",
    "zh-TW": "出現提示訊息後，技能書就會消失。",
    "pt-BR": "O livro de habilidade desaparecerá, exibindo uma mensagem."
  },
  "npc.prose.652f0a88b25ed0c0": {
    "en": "You must rely on your own power only.",
    "zh-TW": "你只能依靠自己的力量。",
    "pt-BR": "Você deve contar apenas com a própria força."
  },
  "npc.prose.657380a0af1be834": {
    "en": "related to the tragedy.",
    "zh-TW": "與那場悲劇有關。",
    "pt-BR": "relacionado à tragédia."
  },
  "npc.prose.657ecaf5a2fc0f73": {
    "en": "Specialized in undead type monsters.",
    "zh-TW": "專門對付不死系怪物。",
    "pt-BR": "Especializada em monstros mortos-vivos."
  },
  "npc.prose.6592f5a5e6d2fa05": {
    "en": "Welcome, how may I help you?",
    "zh-TW": "歡迎，有什麼能幫你的嗎？",
    "pt-BR": "Boas-vindas. Como posso ajudar?"
  },
  "npc.prose.65a7588e906724e4": {
    "en": "You are already Rebirth 1.",
    "zh-TW": "你已完成第一次轉生。",
    "pt-BR": "Você já possui o primeiro renascimento."
  },
  "npc.prose.65d82e6e5d1bc40c": {
    "en": "Wizard:",
    "zh-TW": "法師：",
    "pt-BR": "Mago:"
  },
  "npc.prose.65e20900038802f7": {
    "en": "My service fee is 2000 gold.",
    "zh-TW": "我的服務費是 2,000 金幣。",
    "pt-BR": "Minha taxa de serviço é de 2.000 moedas de ouro."
  },
  "npc.prose.65f2432e2a291bb8": {
    "en": "you can not timetravel even with the stone. I know it is",
    "zh-TW": "即使有這塊石頭，你也無法穿越時空。我知道這",
    "pt-BR": "você não pode viajar no tempo nem com a pedra. Sei que é"
  },
  "npc.prose.665fca59595ced72": {
    "en": "However, only the guild masters or the members who",
    "zh-TW": "不過，只有行會會長或那些成員",
    "pt-BR": "No entanto, somente os líderes de guilda ou os membros que"
  },
  "npc.prose.666400ca298322b3": {
    "en": ",  (KR) ,  ,  ,",
    "zh-TW": "， （韓國） ， ， ，",
    "pt-BR": ", (Coreia), , ,"
  },
  "npc.prose.66be299f48261b90": {
    "en": "Ah those symbols... I helped Sir Mogu with them in the past.",
    "zh-TW": "啊，那些符號……我以前幫莫古爵士研究過。",
    "pt-BR": "Ah, aqueles símbolos... Ajudei Sir Mogu com eles no passado."
  },
  "npc.prose.66c01ebf5e925429": {
    "en": "Keep clicking skill key and left click the target in 15 seconds.",
    "zh-TW": "按下技能快捷鍵後，於 15 秒內左鍵點擊目標。",
    "pt-BR": "Pressione a tecla da habilidade e clique no alvo com o botão esquerdo em até 15 segundos."
  },
  "npc.prose.66c40125b0b840c6": {
    "en": "Fire a poison arrow to green poison enemies.",
    "zh-TW": "射出毒箭，使敵人中了綠毒。",
    "pt-BR": "Dispara uma flecha venenosa que aplica veneno verde aos inimigos."
  },
  "npc.prose.66ddb37329e32543": {
    "en": "What do you want to know?",
    "zh-TW": "你想知道什麼？",
    "pt-BR": "O que deseja saber?"
  },
  "npc.prose.66e65edff12e7490": {
    "en": "In fact, you can get some fresh  and by trading it with butcher in town,",
    "zh-TW": "事實上，你可以取得新鮮物品，再與城裡的屠夫交易，",
    "pt-BR": "Na verdade, você pode obter produtos frescos e negociá-los com o açougueiro da cidade,"
  },
  "npc.prose.66f62586171737b9": {
    "en": "but this oil doesn't special repair your weapon, so",
    "zh-TW": "但這種油無法特修武器，所以",
    "pt-BR": "mas este óleo não faz um reparo especial na arma; portanto,"
  },
  "npc.prose.67736da60b20ce6d": {
    "en": "Tip: Use @superman for an infinite amount of HP and MP.",
    "zh-TW": "提示：使用 @superman 可獲得無限生命與魔力。",
    "pt-BR": "Dica: use @superman para ter PV e PM infinitos."
  },
  "npc.prose.67828183f217d3b5": {
    "en": "Do you want to listen to it again?",
    "zh-TW": "你想再聽一次嗎？",
    "pt-BR": "Deseja ouvir novamente?"
  },
  "npc.prose.68078d59e6479b53": {
    "en": "However, if you are PK your ID will be red in colour,",
    "zh-TW": "不過，若你殺害玩家，名字會變成紅色，",
    "pt-BR": "No entanto, se você matar jogadores, seu nome ficará vermelho,"
  },
  "npc.prose.689f3f302732f9ba": {
    "en": "Hello traveller. How can i help you?",
    "zh-TW": "你好，旅人。有什麼能幫你的？",
    "pt-BR": "Olá, viajante. Como posso ajudar?"
  },
  "npc.prose.68e5b0e5cc3e1750": {
    "en": "Someones remains.",
    "zh-TW": "某人的遺骸。",
    "pt-BR": "Os restos mortais de alguém."
  },
  "npc.prose.68fdc68bcd6088a3": {
    "en": "use the 'trade' menu of the main screen.",
    "zh-TW": "使用主畫面的「交易」選單。",
    "pt-BR": "use o menu “Negociar” na tela principal."
  },
  "npc.prose.697136bf33dd2588": {
    "en": "Go to PrajnaTemple at Level 35 to 40.",
    "zh-TW": "35 至 40 級時可前往般若寺廟。",
    "pt-BR": "Dos níveis 35 a 40, vá ao Templo de Prajna."
  },
  "npc.prose.69cad61a82cfff9a": {
    "en": "I'm dispatched from the Bichon Wall,",
    "zh-TW": "我是從比奇城派來的，",
    "pt-BR": "Fui enviado da muralha de Bichon,"
  },
  "npc.prose.6a36aaf5c248a326": {
    "en": "My task is important but I feel sorry watching my fellow",
    "zh-TW": "我的任務很重要，但看著同伴們，我感到難過",
    "pt-BR": "Minha missão é importante, mas fico triste ao ver meus companheiros"
  },
  "npc.prose.6a9534f58e4f39e1": {
    "en": "half of Level 30.",
    "zh-TW": "30 級的一半。",
    "pt-BR": "metade do nível 30."
  },
  "npc.prose.6ac33967ac929483": {
    "en": "Candles are needed when it's dark. without a candle",
    "zh-TW": "天黑時需要蠟燭。沒有蠟燭",
    "pt-BR": "Velas são necessárias no escuro. Sem uma vela,"
  },
  "npc.prose.6b0014a0911f16bf": {
    "en": "20000 Gold for one piece...You want it?",
    "zh-TW": "一件 20,000 金幣……你要嗎？",
    "pt-BR": "20.000 moedas de ouro por peça... Você quer?"
  },
  "npc.prose.6b0c9b1f9fc86dfa": {
    "en": "TO DO",
    "zh-TW": "待辦事項",
    "pt-BR": "A fazer"
  },
  "npc.prose.6b3159315bba407c": {
    "en": "the Guild Territory Bulletin Board.",
    "zh-TW": "行會領地公告板。",
    "pt-BR": "o mural do território da guilda."
  },
  "npc.prose.6b69c66e392cd845": {
    "en": "and so on forth.",
    "zh-TW": "等等。",
    "pt-BR": "e assim por diante."
  },
  "npc.prose.6b77ad28792e0358": {
    "en": "Tip: @observer if you want to be invisible to other players.",
    "zh-TW": "提示：若想對其他玩家隱身，可使用 @observer。",
    "pt-BR": "Dica: use @observer se quiser ficar invisível para outros jogadores."
  },
  "npc.prose.6b7d8493c4554e95": {
    "en": "purchase a guild territory from the list, send a 'note'",
    "zh-TW": "從清單購買行會領地，請寄出一則「留言」",
    "pt-BR": "comprar um território de guilda da lista, envie uma “nota”"
  },
  "npc.prose.6c064ff2452648df": {
    "en": "can help save a life.",
    "zh-TW": "也可能挽救一條生命。",
    "pt-BR": "pode ajudar a salvar uma vida."
  },
  "npc.prose.6c7cb04a40919699": {
    "en": "I pay well.. if ever wish to sell anything come back",
    "zh-TW": "我出價不錯……若有東西要賣，歡迎再來",
    "pt-BR": "Pago bem... Volte quando quiser vender algo."
  },
  "npc.prose.6c9603cbe52c2d8b": {
    "en": "A wave of energy that sends anything stood next to them",
    "zh-TW": "釋放能量波，將身旁的任何目標",
    "pt-BR": "Uma onda de energia que envia qualquer alvo próximo"
  },
  "npc.prose.6cb314e20b3d30d3": {
    "en": "Skill will be ineffective at higher level enemy.",
    "zh-TW": "此技能對更高等級的敵人無效。",
    "pt-BR": "A habilidade não funciona contra inimigos de nível superior."
  },
  "npc.prose.6dea5a923b43cbb5": {
    "en": "Current Owner of SabukWall is: {npc_arg0}",
    "zh-TW": "目前沙巴克的擁有者是：{npc_arg0}",
    "pt-BR": "Atual proprietário de Sabuk: {npc_arg0}"
  },
  "npc.prose.6e0066a02d7c8778": {
    "en": "from monsters only",
    "zh-TW": "只能從怪物取得",
    "pt-BR": "apenas de monstros"
  },
  "npc.prose.6e3b5022adea1873": {
    "en": "Nothing happens.",
    "zh-TW": "什麼事也沒發生。",
    "pt-BR": "Nada acontece."
  },
  "npc.prose.6e4f3e814b6f2584": {
    "en": "I want to check the  of guild territories.",
    "zh-TW": "我想查看行會領地的資訊。",
    "pt-BR": "Quero consultar as informações dos territórios de guilda."
  },
  "npc.prose.6e713b83858d769e": {
    "en": "1% of the selling price is paid.",
    "zh-TW": "需支付售價的 1%。",
    "pt-BR": "paga-se 1% do preço de venda."
  },
  "npc.prose.6e7f29da162cdeaa": {
    "en": "I sell Items for your Tiger! Please take alook!",
    "zh-TW": "我販售猛虎用品！請看看吧！",
    "pt-BR": "Vendo itens para seu tigre! Dê uma olhada!"
  },
  "npc.prose.6e9d0dd7b59c670f": {
    "en": "\"You are a party leader, Youre party is present,",
    "zh-TW": "「你是隊長，你的隊伍也已到場，",
    "pt-BR": "“Você é líder de grupo e seu grupo está presente,"
  },
  "npc.prose.6ea549b2df0a70f1": {
    "en": "\"Those who dare enters will die\"",
    "zh-TW": "「膽敢進入者必死」",
    "pt-BR": "“Quem ousar entrar morrerá.”"
  },
  "npc.prose.6f08fe104c7460b3": {
    "en": "If you wish to know the names of the heroes,",
    "zh-TW": "若想知道那些英雄的名字，",
    "pt-BR": "Se quiser saber os nomes dos heróis,"
  },
  "npc.prose.6f4798aa96ac9557": {
    "en": "it means nothing to us who stay in the Taoist world,",
    "zh-TW": "對留在道界的我們而言，毫無意義，",
    "pt-BR": "isso nada significa para nós, que permanecemos no mundo taoísta,"
  },
  "npc.prose.6f88cccb3df8aca2": {
    "en": "No information about him remains. I just heard that the former",
    "zh-TW": "關於他的資料已不復存在。我只聽說前任",
    "pt-BR": "Não restam informações sobre ele. Só ouvi dizer que o antigo"
  },
  "npc.prose.7014cf6d1c54dc02": {
    "en": "Learn about",
    "zh-TW": "了解",
    "pt-BR": "Saiba mais sobre"
  },
  "npc.prose.70657fcbb4b7ab44": {
    "en": "remember that then maximum durabillity of the weapon",
    "zh-TW": "記住，武器的最高耐久度",
    "pt-BR": "lembre-se de que a durabilidade máxima da arma"
  },
  "npc.prose.706c6ad29ca89aa0": {
    "en": "More powerful skill of the basic fireball.",
    "zh-TW": "比基本火球術更強大的技能。",
    "pt-BR": "Uma versão mais poderosa da habilidade básica Bola de Fogo."
  },
  "npc.prose.708e556d8b770e5c": {
    "en": "Although RedMoonEvil defeated me and the HolySword at the time,",
    "zh-TW": "雖然當年赤月惡魔擊敗了我與聖劍，",
    "pt-BR": "Embora o Demônio da Lua Vermelha tenha derrotado a mim e à Espada Sagrada naquela época,"
  },
  "npc.prose.7098feddace01265": {
    "en": "TownTeleport scrolls carn't be made now because the ancient",
    "zh-TW": "現在已無法製作回城卷，因為古代的",
    "pt-BR": "Hoje não é possível fabricar pergaminhos de retorno, pois os antigos"
  },
  "npc.prose.70e8a9bc23b0f25d": {
    "en": "taking part in CALMs Lost Hours Walk, to join",
    "zh-TW": "參加 CALM 的 Lost Hours Walk，加入",
    "pt-BR": "participando da caminhada Lost Hours Walk da CALM, para se juntar"
  },
  "npc.prose.716b69866e1c4b0c": {
    "en": "Theres not much information about the Language..",
    "zh-TW": "關於這種語言的資料不多……",
    "pt-BR": "Há pouca informação sobre essa língua..."
  },
  "npc.prose.71715d8ed733cf91": {
    "en": "What item would you like to buy or sell?",
    "zh-TW": "你想購買或出售什麼物品？",
    "pt-BR": "Que item deseja comprar ou vender?"
  },
  "npc.prose.71c3dc644a430b7a": {
    "en": "Shop moving function in village",
    "zh-TW": "村莊商店傳送功能",
    "pt-BR": "Função de deslocamento entre lojas da vila"
  },
  "npc.prose.71c56d23e40b3824": {
    "en": "Bring Candle, Portal Scroll and enough potions.",
    "zh-TW": "請帶上蠟燭、傳送卷軸及足夠的藥水。",
    "pt-BR": "Leve uma vela, um pergaminho de portal e poções suficientes."
  },
  "npc.prose.7219d2eba4d3c6fc": {
    "en": "If the situation is dangerous but you still have potions, use this",
    "zh-TW": "若處境危險但仍有藥水，可使用這個",
    "pt-BR": "Se a situação for perigosa, mas você ainda tiver poções, use isto"
  },
  "npc.prose.72cc42442dd004d3": {
    "en": "I am craft lady, specialized in the creation of new items.",
    "zh-TW": "我是工藝師，專門製作新物品。",
    "pt-BR": "Sou artesã, especializada na criação de novos itens."
  },
  "npc.prose.72d70acc3c11dc04": {
    "en": "Using magic to kill an animal burns the meat, reducing its quality to 0.",
    "zh-TW": "用魔法殺死動物會燒焦肉，使品質降為 0。",
    "pt-BR": "Matar um animal com magia queima a carne, reduzindo sua qualidade a 0."
  },
  "npc.prose.72e46f1071d56908": {
    "en": "Duration time depends on the skill level.",
    "zh-TW": "持續時間取決於技能等級。",
    "pt-BR": "A duração depende do nível da habilidade."
  },
  "npc.prose.7340e2589b4876b3": {
    "en": "Ahh...where am i?",
    "zh-TW": "啊……我在哪裡？",
    "pt-BR": "Ah... onde estou?"
  },
  "npc.prose.737f3e24574d15af": {
    "en": "Poison that keeps giving damage over certain time to enemies.",
    "zh-TW": "在一段時間內持續傷害敵人的毒。",
    "pt-BR": "Veneno que causa dano contínuo aos inimigos durante um período."
  },
  "npc.prose.742813dc4379dd8e": {
    "en": "me a lot of enemies. But I was no longer interested in community life",
    "zh-TW": "讓我樹敵眾多。但我對群居生活已不再有興趣",
    "pt-BR": "me trouxe muitos inimigos. Mas eu já não tinha interesse em viver em comunidade"
  },
  "npc.prose.7440f2b4d159d174": {
    "en": "I want to  the rental period.",
    "zh-TW": "我想調整租期。",
    "pt-BR": "Quero alterar o período de aluguel."
  },
  "npc.prose.744a1c5ec2a860bb": {
    "en": "you can not see anything-even your foot for darkness.",
    "zh-TW": "黑暗中，你什麼也看不到，連自己的腳都看不見。",
    "pt-BR": "na escuridão, você não consegue ver nada, nem os próprios pés."
  },
  "npc.prose.7460dbdf0260e7c1": {
    "en": "My beloved house..",
    "zh-TW": "我心愛的房子……",
    "pt-BR": "Minha querida casa..."
  },
  "npc.prose.746c289b21b9673c": {
    "en": "a certain RedEvilApe the RedMoonSword was finally broken I recovered",
    "zh-TW": "某隻赤惡猿時，赤月劍終於斷裂，我恢復了",
    "pt-BR": "um certo Macaco Demoníaco Vermelho, a Espada da Lua Vermelha finalmente se quebrou e recuperei"
  },
  "npc.prose.746caecf2990f7ae": {
    "en": "Greetings! How can i help you?",
    "zh-TW": "你好！有什麼能幫你的？",
    "pt-BR": "Saudações! Como posso ajudar?"
  },
  "npc.prose.749e9ddb39e69c40": {
    "en": "This will enhance the protection from enemies.",
    "zh-TW": "這能增強對敵人的防護。",
    "pt-BR": "Isso aumenta a proteção contra os inimigos."
  },
  "npc.prose.74a7be86f78e8298": {
    "en": "I have heard that it may come randomly from Monsters",
    "zh-TW": "我聽說可能從怪物身上隨機取得",
    "pt-BR": "Ouvi dizer que pode ser obtido aleatoriamente de monstros."
  },
  "npc.prose.74e8c4cadad40a5d": {
    "en": "about the local legend",
    "zh-TW": "關於當地的傳說",
    "pt-BR": "sobre a lenda local"
  },
  "npc.prose.565d240f5343e625": {
    "en": "||",
    "zh-TW": "||",
    "pt-BR": "||",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.57cc2b52066e506b": {
    "en": ",  , , ,",
    "zh-TW": ",  , , ,",
    "pt-BR": ",  , , ,",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.753e5bcb58a07e67": {
    "en": "Mir continent, will be erected.",
    "zh-TW": "傳奇大陸的石像將在此豎立。",
    "pt-BR": "do continente de Mir serão erguidas."
  },
  "npc.prose.755e8a7f412de78c": {
    "en": "Archer skill page 2:",
    "zh-TW": "弓箭手技能第 2 頁：",
    "pt-BR": "Habilidades de arqueiro, página 2:"
  },
  "npc.prose.75d27f8a03d63e5d": {
    "en": "that will be disposed and click ground.",
    "zh-TW": "選好要丟棄的物品，再點擊地面。",
    "pt-BR": "a ser descartado e clique no chão."
  },
  "npc.prose.7642f46fcbb4095f": {
    "en": "After a successful transaction, the ownership shall be",
    "zh-TW": "交易成功後，所有權將",
    "pt-BR": "Após uma transação bem-sucedida, a propriedade será"
  },
  "npc.prose.764916d7fa27dd14": {
    "en": "Conquest Gold Stored: {npc_arg0}",
    "zh-TW": "已儲存的攻城金幣：{npc_arg0}",
    "pt-BR": "Ouro de conquista armazenado: {npc_arg0}"
  },
  "npc.prose.767b35a1a04e7e4d": {
    "en": "\"You are a crazy old .\"",
    "zh-TW": "「你是個瘋老頭。」",
    "pt-BR": "“Você é um velho maluco.”"
  },
  "npc.prose.771dcce51e3cf700": {
    "en": "You have no GoldBarBundle for me to Exchange...",
    "zh-TW": "你沒有可供兌換的金磚束……",
    "pt-BR": "Você não tem um pacote de barras de ouro para trocar..."
  },
  "npc.prose.7735a6027da1c080": {
    "en": "The master and members of the guild that sold the territory",
    "zh-TW": "售出領地的行會會長及成員",
    "pt-BR": "O líder e os membros da guilda que vendeu o território"
  },
  "npc.prose.776d1e816b7264fc": {
    "en": "Multiple area attack against enemies a distance away.",
    "zh-TW": "對遠處敵人發動多目標範圍攻擊。",
    "pt-BR": "Ataque de área contra vários inimigos distantes."
  },
  "npc.prose.77773062628f4309": {
    "en": "Level 53: IceFreeze",
    "zh-TW": "53 級：冰封",
    "pt-BR": "Nível 53: Congelamento"
  },
  "npc.prose.77908d4e7226a127": {
    "en": "In fact, you can get some fresh  and by trading it with a",
    "zh-TW": "事實上，你可以取得新鮮物品，再與",
    "pt-BR": "Na verdade, você pode obter produtos frescos e negociá-los com um"
  },
  "npc.prose.77eb67e121720ebd": {
    "en": "Hello I'm Denzel, the wandering warrior.",
    "zh-TW": "你好，我是丹澤爾，一名流浪戰士。",
    "pt-BR": "Olá, sou Denzel, um guerreiro errante."
  },
  "npc.prose.780597fbe2bdd132": {
    "en": "How do you feel about my song?",
    "zh-TW": "你覺得我的歌如何？",
    "pt-BR": "O que achou da minha canção?"
  },
  "npc.prose.78869c1dd1309cbc": {
    "en": "I can sell you items or you can sell them to me..",
    "zh-TW": "你可以向我購買物品，也可以賣給我……",
    "pt-BR": "Posso vender itens para você, ou você pode vendê-los para mim..."
  },
  "npc.prose.790fda42ce6ff959": {
    "en": "repair with this oil sometimes during hunting.",
    "zh-TW": "狩獵途中可偶爾使用這種油修理。",
    "pt-BR": "repare com este óleo de vez em quando durante a caçada."
  },
  "npc.prose.7930b3b0dcfec1a2": {
    "en": "It is located far away from town.",
    "zh-TW": "它位於遠離城鎮的地方。",
    "pt-BR": "Fica longe da cidade."
  },
  "npc.prose.793f1fb175caeaf0": {
    "en": "Which ring would you like to buy or sell?",
    "zh-TW": "你想購買或出售哪枚戒指？",
    "pt-BR": "Qual anel deseja comprar ou vender?"
  },
  "npc.prose.79c78e00972c7487": {
    "en": "I told you everything I can.",
    "zh-TW": "我已經把能說的都告訴你了。",
    "pt-BR": "Já contei tudo o que podia."
  },
  "npc.prose.79d9dff660401510": {
    "en": "\"Would rather not say.. Have you seen this \"",
    "zh-TW": "「我不太想說……你看過這個嗎？」",
    "pt-BR": "“Prefiro não dizer... Você já viu isto?”"
  },
  "npc.prose.7a183a26f0d58e47": {
    "en": "have it anymore. People say that the sword is able to",
    "zh-TW": "已不再擁有它。人們說，那把劍能夠",
    "pt-BR": "a temos mais. Dizem que a espada é capaz de"
  },
  "npc.prose.7a2670b12a537842": {
    "en": "\"The Bones are crunching the Fear is \"",
    "zh-TW": "「骨頭嘎嘎作響，恐懼正在」",
    "pt-BR": "“Os ossos estalam, o medo está”"
  },
  "npc.prose.7a85e0fdaaaa2f17": {
    "en": "Fishing Stuff.",
    "zh-TW": "釣魚用品。",
    "pt-BR": "Artigos de pesca."
  },
  "npc.prose.7b60d059b37ca484": {
    "en": "LionsRoar (Level 36)",
    "zh-TW": "獅子吼（36 級）",
    "pt-BR": "Rugido do Leão (nível 36)"
  },
  "npc.prose.7b8c630cb1211feb": {
    "en": "striven for martial art training since I was 5 years old When I was",
    "zh-TW": "從 5 歲起便努力習武。當我",
    "pt-BR": "me dediquei às artes marciais desde os 5 anos. Quando eu"
  },
  "npc.prose.7bd77da3a4b61e11": {
    "en": "\"Dungeon of the Ancient Ones.\"",
    "zh-TW": "「遠古者地牢。」",
    "pt-BR": "“Calabouço dos Antigos.”"
  },
  "npc.prose.7c2025a538dc5573": {
    "en": "Hey you... I'll let you get into the dungeon",
    "zh-TW": "喂，你……我可以讓你進入地牢",
    "pt-BR": "Ei, você... Vou deixar você entrar no calabouço."
  },
  "npc.prose.7c53e1e5ce21f68d": {
    "en": "Can also proc a critical attack to do extra damage.",
    "zh-TW": "也有機率觸發暴擊，造成額外傷害。",
    "pt-BR": "Também pode ativar um golpe crítico e causar dano adicional."
  },
  "npc.prose.7c78dc95e118ca38": {
    "en": "Defense power and duration time will depend on the skill level.",
    "zh-TW": "防禦力與持續時間取決於技能等級。",
    "pt-BR": "A defesa e a duração dependem do nível da habilidade."
  },
  "npc.prose.7cb53600ec2e622e": {
    "en": "This will be provided later.",
    "zh-TW": "此功能將於稍後提供。",
    "pt-BR": "Isso será disponibilizado mais tarde."
  },
  "npc.prose.7cb5bcab88850434": {
    "en": "past the BorderVillage. To investigate this area, we have prepared",
    "zh-TW": "越過邊境村。為了調查這個區域，我們已準備",
    "pt-BR": "além da Vila da Fronteira. Para investigar esta área, preparamos"
  },
  "npc.prose.7cc4ca82c9a3e2dd": {
    "en": "if and when something happens around here.",
    "zh-TW": "若這裡發生什麼事情。",
    "pt-BR": "caso algo aconteça por aqui."
  },
  "npc.prose.7cde18ae32d9830f": {
    "en": "Poison.",
    "zh-TW": "毒藥。",
    "pt-BR": "Veneno."
  },
  "npc.prose.7ce8a7dad814ba9c": {
    "en": "In Memory Of Luke Thompson",
    "zh-TW": "紀念 Luke Thompson",
    "pt-BR": "Em memória de Luke Thompson"
  },
  "npc.prose.7d24d76646cc19be": {
    "en": "I deal with ,,",
    "zh-TW": "我經營這些商品：",
    "pt-BR": "Negocio estes produtos:"
  },
  "npc.prose.7d32672b726cba9a": {
    "en": "and ill take alook.",
    "zh-TW": "我會看看。",
    "pt-BR": "e darei uma olhada."
  },
  "npc.prose.7d3b37aeadedba3c": {
    "en": "And the time limit is two hours",
    "zh-TW": "時間限制為兩小時",
    "pt-BR": "O limite de tempo é de duas horas."
  },
  "npc.prose.7d41279f32d8c0d6": {
    "en": "Exchange:  into Gold - Commission (10000 Gold)",
    "zh-TW": "兌換：換成金幣－手續費 10,000 金幣",
    "pt-BR": "Troca: por ouro — comissão de 10.000 moedas de ouro"
  },
  "npc.prose.7d70dec66c8a45a6": {
    "en": "Increases physical defensive power which also affects people around",
    "zh-TW": "提高物理防禦，也會影響周圍的人",
    "pt-BR": "Aumenta a defesa física e também afeta as pessoas ao redor."
  },
  "npc.prose.7e7f4c3ee708dac4": {
    "en": "Slice meat: Getting Meat from domestic animal such as chicken, Deer Click",
    "zh-TW": "割肉：從雞、鹿等動物身上取得肉。點擊",
    "pt-BR": "Cortar carne: obtenha carne de animais, como galinhas e cervos. Clique"
  },
  "npc.prose.7e89b38e8a3ad691": {
    "en": "Traveller.. You are trying to bribe the Commander of the 7th Rank.",
    "zh-TW": "旅人……你竟想賄賂第七階指揮官。",
    "pt-BR": "Viajante... Você está tentando subornar o comandante do sétimo escalão."
  },
  "npc.prose.7ed1c958cc7958e5": {
    "en": "monsters when taken such slicing meat action.",
    "zh-TW": "採取割肉動作時，可從怪物取得。",
    "pt-BR": "de monstros ao realizar a ação de cortar carne."
  },
  "npc.prose.7ee718909e0b85d3": {
    "en": "I deal with , , ,",
    "zh-TW": "我經營這些商品：",
    "pt-BR": "Negocio estes produtos:"
  },
  "npc.prose.7f18c37461a470f6": {
    "en": "murdered each others, enemies and friends alike the heaps of corpses",
    "zh-TW": "互相殘殺，無論敵友，屍體堆積如山",
    "pt-BR": "matavam uns aos outros, inimigos e amigos, deixando pilhas de corpos"
  },
  "npc.prose.7f22919f4489ef5e": {
    "en": "If the caster is attacked while using the skill, it will be cancelled.",
    "zh-TW": "若施法者在施放期間受到攻擊，技能會被中斷。",
    "pt-BR": "Se o conjurador for atacado durante o uso, a habilidade será cancelada."
  },
  "npc.prose.7fb53aa26d6afadf": {
    "en": "You currently have Main Rebirth Effect.",
    "zh-TW": "你目前擁有主轉生效果。",
    "pt-BR": "Você possui atualmente o efeito principal de renascimento."
  },
  "npc.prose.7fb8a0f37eee68d2": {
    "en": "Or give it a go?",
    "zh-TW": "還是試試看？",
    "pt-BR": "Ou quer tentar?"
  },
  "npc.prose.7fcac6ac9d552252": {
    "en": "even for toilet. I wonder I might suffer from",
    "zh-TW": "連上廁所都不行。我擔心自己會得",
    "pt-BR": "nem para ir ao banheiro. Temo que eu possa sofrer de"
  },
  "npc.prose.802f0ed2c3c1464d": {
    "en": "It has a very long casting time but gives a higher damage than the Blizzard skill.",
    "zh-TW": "施法時間很長，但傷害高於暴風雪。",
    "pt-BR": "O tempo de conjuração é muito longo, mas causa mais dano que a Nevasca."
  },
  "npc.prose.804583352041c8fb": {
    "en": "Parcel",
    "zh-TW": "包裹",
    "pt-BR": "Encomenda"
  },
  "npc.prose.8067d0712620b867": {
    "en": "I will guide you to the desired store.",
    "zh-TW": "我會帶你前往想去的商店。",
    "pt-BR": "Vou guiar você até a loja desejada."
  },
  "npc.prose.80a548954972c1a1": {
    "en": "in deep in the Wooma Temple in Woomyon Woods.",
    "zh-TW": "位於沃瑪森林的沃瑪寺廟深處。",
    "pt-BR": "nas profundezas do Templo de Wooma, no Bosque de Woomyon."
  },
  "npc.prose.80c6ce3926904f36": {
    "en": "The boat goes many places. The boat will depart soon",
    "zh-TW": "這艘船前往許多地方，很快就要啟航了",
    "pt-BR": "O barco vai a muitos lugares e partirá em breve."
  },
  "npc.prose.80f7a60667a54a58": {
    "en": "Repair.",
    "zh-TW": "修理。",
    "pt-BR": "Reparar."
  },
  "npc.prose.811f33b7b7ac139a": {
    "en": "Have you heard of the legends of Ice Hell?",
    "zh-TW": "你聽過冰獄的傳說嗎？",
    "pt-BR": "Você já ouviu as lendas do Inferno de Gelo?"
  },
  "npc.prose.8174c731bf48de56": {
    "en": "leaving here You came here with a JadeCrystal.. My name is Abel,",
    "zh-TW": "離開這裡。你帶著翡翠水晶來了……我叫亞伯，",
    "pt-BR": "sair daqui. Você chegou com um Cristal de Jade... Meu nome é Abel,"
  },
  "npc.prose.81c365ce5f4b6a96": {
    "en": "Once the skill has been used, you will have to wait to use it again.",
    "zh-TW": "使用此技能後，需要等待一段時間才能再次使用。",
    "pt-BR": "Após usar a habilidade, você terá de esperar para usá-la novamente."
  },
  "npc.prose.81da6bcba04be71f": {
    "en": "Hes such a lazy good for nothing fool. Sooner we remove him",
    "zh-TW": "他是個又懶又沒用的傻瓜。我們越早除掉他",
    "pt-BR": "Ele é um preguiçoso imprestável. Quanto antes o removermos,"
  },
  "npc.prose.82189e67d81ca5ff": {
    "en": "no matter what.",
    "zh-TW": "無論如何。",
    "pt-BR": "não importa o que aconteça."
  },
  "npc.prose.8228fcf0106b7512": {
    "en": "two identical drakes rising the sky.",
    "zh-TW": "兩條相同的飛龍升上天空。",
    "pt-BR": "dois dragões idênticos subindo ao céu."
  },
  "npc.prose.82bc28f3f1993991": {
    "en": "A strong monster called BoneElite lives in OmaCave",
    "zh-TW": "半獸人洞穴中住著一隻名叫骷髏精靈的強大怪物",
    "pt-BR": "Um monstro poderoso chamado Esqueleto de Elite vive na Caverna dos Omas."
  },
  "npc.prose.82e5cd9423c84b78": {
    "en": "\"I need to  the Emperor!\"",
    "zh-TW": "「我需要見皇帝！」",
    "pt-BR": "“Preciso ver o imperador!”"
  },
  "npc.prose.8323473e297aa210": {
    "en": "Once you have killed it, press the ALT button, while left clicking",
    "zh-TW": "殺死牠後，按住 ALT 鍵並按滑鼠左鍵",
    "pt-BR": "Depois de matá-lo, segure ALT e clique com o botão esquerdo."
  },
  "npc.prose.83247e0596cea71c": {
    "en": "Throws a column of flame straight ahead of the caster. (5 square range)",
    "zh-TW": "向施法者正前方噴出火柱。（射程 5 格）",
    "pt-BR": "Lança uma coluna de chamas à frente do conjurador. (Alcance de 5 casas.)"
  },
  "npc.prose.839f3e92d496d0e3": {
    "en": "It made me angry and I extorted the sword by force of arms.",
    "zh-TW": "這讓我憤怒，我以武力奪走了那把劍。",
    "pt-BR": "Aquilo me enfureceu e tomei a espada pela força."
  },
  "npc.prose.83a18434f65a2011": {
    "en": "I see you're wearing:",
    "zh-TW": "我看見你戴著：",
    "pt-BR": "Vejo que você usa:"
  },
  "npc.prose.83e4085e3aa504a2": {
    "en": "CaveMaggot, and Skeleton appear.",
    "zh-TW": "會出現洞蛆與骷髏。",
    "pt-BR": "Aparecem vermes de caverna e esqueletos."
  },
  "npc.prose.841db51df1b44fa7": {
    "en": "You haven't beat them yet.",
    "zh-TW": "你還沒打敗牠們。",
    "pt-BR": "Você ainda não os derrotou."
  },
  "npc.prose.84437d294e44fbf9": {
    "en": "He created his own Mir 2 Files from Crystal and called it Eden Elite,",
    "zh-TW": "他以 Crystal 為基礎製作自己的 Mir 2 檔案，命名為 Eden Elite，",
    "pt-BR": "Ele criou seus próprios arquivos de Mir 2 a partir do Crystal e os chamou de Eden Elite,"
  },
  "npc.prose.8465930008843606": {
    "en": "Absorbs strength of enemy to recover players strength.",
    "zh-TW": "吸取敵人的力量，恢復玩家的力量。",
    "pt-BR": "Absorve a força do inimigo para restaurar a do jogador."
  },
  "npc.prose.84822e75eb9ad5b5": {
    "en": "day were affected by the RedMoonEvil, the origin of the demon,",
    "zh-TW": "那天都受到惡魔之源赤月惡魔的影響，",
    "pt-BR": "naquele dia foram afetados pelo Demônio da Lua Vermelha, a origem do mal,"
  },
  "npc.prose.8491cf3568446a55": {
    "en": "promptly demon energy made me a demon of a man Because the",
    "zh-TW": "魔氣迅速將我變成人形惡魔，因為",
    "pt-BR": "rapidamente a energia demoníaca me transformou em um demônio humano, pois"
  },
  "npc.prose.8493912a0245b1b9": {
    "en": "However if you guess wrongly you will lose 10,000 Gold",
    "zh-TW": "但若猜錯，你會失去 10,000 金幣。",
    "pt-BR": "Mas, se errar, perderá 10.000 moedas de ouro."
  },
  "npc.prose.84bb7152552b5f03": {
    "en": "distance attack and only attacked mobs will approach you.",
    "zh-TW": "遠距攻擊，只有遭到攻擊的怪物才會靠近你。",
    "pt-BR": "ataque à distância, e apenas os monstros atingidos se aproximarão de você."
  },
  "npc.prose.84fe5e9c3544f886": {
    "en": "I'm afraid if you haven't heard of the beast",
    "zh-TW": "恐怕，若你沒聽說過那頭野獸",
    "pt-BR": "Receio que, se você nunca ouviu falar da fera,"
  },
  "npc.prose.85cb9d38135279a1": {
    "en": "150,000 Gold",
    "zh-TW": "150,000 金幣",
    "pt-BR": "150.000 moedas de ouro"
  },
  "npc.prose.85e5efe6bbe54ab5": {
    "en": "I will buy high quality for high price.",
    "zh-TW": "品質好的，我會出高價收購。",
    "pt-BR": "Pago um bom preço por produtos de alta qualidade."
  },
  "npc.prose.8625830086ded040": {
    "en": "To request Sabuk conquest war you should have ZumaRelic.",
    "zh-TW": "申請沙巴克攻城戰，需要祖瑪遺物。",
    "pt-BR": "Para solicitar uma guerra de conquista de Sabuk, você precisa de uma Relíquia de Zuma."
  },
  "npc.prose.8654977a7705b119": {
    "en": "about CastleBichon",
    "zh-TW": "關於比奇城堡",
    "pt-BR": "sobre o Castelo de Bichon"
  },
  "npc.prose.8679bac8fc320053": {
    "en": "Purifies poisoned or corrupted body by dark foce.",
    "zh-TW": "淨化中毒或受黑暗力量侵蝕的身體。",
    "pt-BR": "Purifica o corpo envenenado ou corrompido por forças sombrias."
  },
  "npc.prose.86b74d4dca4cceee": {
    "en": "When you request guild war, names of allies will appear in blue,",
    "zh-TW": "申請行會戰後，盟友的名字會顯示為藍色，",
    "pt-BR": "Ao solicitar uma guerra de guildas, os nomes dos aliados aparecem em azul,"
  },
  "npc.prose.86b93311f2962ac4": {
    "en": "Go and see the owner of this school.",
    "zh-TW": "去見見本門掌門吧。",
    "pt-BR": "Vá falar com o mestre desta escola."
  },
  "npc.prose.86d7dc5781fdbb02": {
    "en": "(Title For Link/https://trello.com/c/9xZB8U2c/162-viperpath-cave)",
    "zh-TW": "（毒蛇小徑洞穴/https://trello.com/c/9xZB8U2c/162-viperpath-cave）",
    "pt-BR": "(Caverna do Caminho das Víboras/https://trello.com/c/9xZB8U2c/162-viperpath-cave)"
  },
  "npc.prose.86fd162a2482579c": {
    "en": "Bracelets or Gloves.",
    "zh-TW": "手鐲或手套。",
    "pt-BR": "Braceletes ou luvas."
  },
  "npc.prose.8709254486ccb0b5": {
    "en": "Welcome to the Guild Territory Panel.",
    "zh-TW": "歡迎使用行會領地面板。",
    "pt-BR": "Boas-vindas ao painel de territórios de guilda."
  },
  "npc.prose.872ed98b9120c003": {
    "en": "Show me which Stones you wish to sell.",
    "zh-TW": "讓我看看你想出售哪些石頭。",
    "pt-BR": "Mostre quais pedras deseja vender."
  },
  "npc.prose.8739e4b1657e3167": {
    "en": "I am the MasterMage Don, What's your name?",
    "zh-TW": "我是大法師唐。你叫什麼名字？",
    "pt-BR": "Sou Don, o mestre mago. Qual é o seu nome?"
  },
  "npc.prose.8741adda67b47ca3": {
    "en": "From all of us at LOMCN,",
    "zh-TW": "來自 LOMCN 全體成員，",
    "pt-BR": "De todos nós da LOMCN,"
  },
  "npc.prose.8762ab9b1a89f227": {
    "en": "Welcome. Thanks for dropping in.",
    "zh-TW": "歡迎，謝謝你來訪。",
    "pt-BR": "Boas-vindas. Obrigado pela visita."
  },
  "npc.prose.87751c07bd08114f": {
    "en": "You get an eerie feeling when touching then pillar..",
    "zh-TW": "觸摸柱子時，你感到一陣寒意……",
    "pt-BR": "Você sente algo sinistro ao tocar no pilar..."
  },
  "npc.prose.87f1d5b337ac35bc": {
    "en": "Level 16: Hellfire",
    "zh-TW": "16 級：地獄火",
    "pt-BR": "Nível 16: Fogo Infernal"
  },
  "npc.prose.87f80afab39e77fb": {
    "en": "However I must warn you, you will need to relog after changing your hair.",
    "zh-TW": "不過得提醒你，更換髮型後需要重新登入。",
    "pt-BR": "Mas devo avisar: após mudar o cabelo, será necessário entrar novamente."
  },
  "npc.prose.883202656243e938": {
    "en": "goes down as time goes by.",
    "zh-TW": "會隨著時間逐漸下降。",
    "pt-BR": "diminui com o passar do tempo."
  },
  "npc.prose.886c95d5c170fa25": {
    "en": "about each frontiers.",
    "zh-TW": "關於各個邊境地區。",
    "pt-BR": "sobre cada fronteira."
  },
  "npc.prose.8890478e402e123c": {
    "en": "1,000 - 50,000,000 Gold.",
    "zh-TW": "1,000～50,000,000 金幣。",
    "pt-BR": "De 1.000 a 50.000.000 de moedas de ouro."
  },
  "npc.prose.88aaca10d23617bb": {
    "en": "I have heard that there are  which come",
    "zh-TW": "我聽說有些東西會出現",
    "pt-BR": "Ouvi dizer que há coisas que vêm"
  },
  "npc.prose.88ab42be2cbe0bee": {
    "en": "HAHA... Mogu sent you...",
    "zh-TW": "哈哈……是莫古派你來的……",
    "pt-BR": "Haha... Mogu enviou você..."
  },
  "npc.prose.89d53511731e7e64": {
    "en": "cursed land?",
    "zh-TW": "受詛咒的土地？",
    "pt-BR": "uma terra amaldiçoada?"
  },
  "npc.prose.89eebbbbcd1989b6": {
    "en": "Ah friend, Will you take my orders and get through the hard way?",
    "zh-TW": "朋友，你願意接受我的指示，走過艱難的道路嗎？",
    "pt-BR": "Amigo, seguirá minhas ordens e enfrentará o caminho difícil?"
  },
  "npc.prose.8a1a348111bbebb6": {
    "en": "Hello I'm Edwin, the wandering warrior.",
    "zh-TW": "你好，我是埃德溫，一名流浪戰士。",
    "pt-BR": "Olá, sou Edwin, um guerreiro errante."
  },
  "npc.prose.8a209d151016076b": {
    "en": "that lurks in the 4th floor?",
    "zh-TW": "潛伏在第四層的？",
    "pt-BR": "que se esconde no quarto andar?"
  },
  "npc.prose.8a24a19de575657c": {
    "en": "so there aren't many merchants out in the valley.",
    "zh-TW": "所以山谷裡的商人並不多。",
    "pt-BR": "por isso há poucos comerciantes no vale."
  },
  "npc.prose.8a682058b1b6cd87": {
    "en": "Summon a spider vampire every 40 seconds to fight for you.",
    "zh-TW": "每 40 秒召喚一隻吸血蜘蛛為你作戰。",
    "pt-BR": "Invoca uma aranha vampira a cada 40 segundos para lutar por você."
  },
  "npc.prose.8a735b46be2e62f3": {
    "en": "Hello {npc_arg0}, My name is Jerald.",
    "zh-TW": "你好，{npc_arg0}。我叫傑拉德。",
    "pt-BR": "Olá, {npc_arg0}. Meu nome é Jerald."
  },
  "npc.prose.8a8d42d6bf950680": {
    "en": "Wine.",
    "zh-TW": "酒。",
    "pt-BR": "Vinho."
  },
  "npc.prose.8aacf19531ded911": {
    "en": "Please choose what you want to buy.",
    "zh-TW": "請選擇想購買的物品。",
    "pt-BR": "Escolha o que deseja comprar."
  },
  "npc.prose.8aaf95605ef7779c": {
    "en": "I see you have: {npc_arg0}",
    "zh-TW": "我看見你擁有：{npc_arg0}",
    "pt-BR": "Vejo que você tem: {npc_arg0}"
  },
  "npc.prose.8ab6ca85999f064f": {
    "en": "by Guildchief.",
    "zh-TW": "由行會會長提出。",
    "pt-BR": "pelo líder da guilda."
  },
  "npc.prose.8acaaa2fd4e9711f": {
    "en": "More elements on cast increases damage reduction.",
    "zh-TW": "施法時擁有越多元素，減傷效果越強。",
    "pt-BR": "Quanto mais elementos houver ao conjurar, maior será a redução de dano."
  },
  "npc.prose.8ae8b5c1816bf112": {
    "en": "demonic creature giving serious problems to the human community",
    "zh-TW": "為人類社會帶來嚴重問題的魔物",
    "pt-BR": "uma criatura demoníaca que causa sérios problemas à comunidade humana"
  },
  "npc.prose.8af825385c12a9c0": {
    "en": "Travelling Merchant Damian visists everyday between 9:30 to 10:30",
    "zh-TW": "旅行商人達米安每天 9:30～10:30 會來訪",
    "pt-BR": "O mercador viajante Damian aparece todos os dias entre 9h30 e 10h30."
  },
  "npc.prose.8b0e0f649ecdf6a8": {
    "en": "Proposal of guild war only can be requested",
    "zh-TW": "行會戰申請只能",
    "pt-BR": "A guerra de guildas só pode ser solicitada"
  },
  "npc.prose.8b3a3bb44e7b2f14": {
    "en": "Walls and Gates",
    "zh-TW": "城牆與城門",
    "pt-BR": "Muralhas e portões"
  },
  "npc.prose.8b5b43b5dfea2e5f": {
    "en": "Would you like to repair a ring?",
    "zh-TW": "你想修理戒指嗎？",
    "pt-BR": "Gostaria de reparar um anel?"
  },
  "npc.prose.8baefd7e6d2e3cdd": {
    "en": "Assassin skill page 1:",
    "zh-TW": "刺客技能第 1 頁：",
    "pt-BR": "Habilidades de assassino, página 1:"
  },
  "npc.prose.8bbe39690beb09b7": {
    "en": "Do you want to move to the guild territory?",
    "zh-TW": "你想前往行會領地嗎？",
    "pt-BR": "Deseja ir ao território da guilda?"
  },
  "npc.prose.8bd8bc45d9f6864c": {
    "en": "I want to",
    "zh-TW": "我想要",
    "pt-BR": "Eu quero"
  },
  "npc.prose.8be73cc0dbb8c10d": {
    "en": "Check your rebirth eligibility above.",
    "zh-TW": "請查看上方的轉生資格。",
    "pt-BR": "Confira acima os requisitos para renascer."
  },
  "npc.prose.8c35c582b291a619": {
    "en": "I'll scold these boys who are still silly",
    "zh-TW": "我會訓斥這些還不懂事的孩子",
    "pt-BR": "Vou repreender esses rapazes que ainda agem como tolos."
  },
  "npc.prose.8c5ad76ee95fbbd4": {
    "en": "to the following Premium Dungeons which don't have any Restrictions.",
    "zh-TW": "前往下列不受限制的進階地牢。",
    "pt-BR": "para os seguintes calabouços premium, que não possuem restrições."
  },
  "npc.prose.8c69b9b46f80cfe4": {
    "en": "From now on many monsters will appear in this chamber.",
    "zh-TW": "從現在開始，許多怪物將出現在這個房間。",
    "pt-BR": "A partir de agora, muitos monstros aparecerão nesta câmara."
  },
  "npc.prose.8cc64027c85875dc": {
    "en": "to certain fields near the last village you visited.",
    "zh-TW": "前往你最近造訪村莊附近的某些區域。",
    "pt-BR": "para certas áreas próximas da última vila que você visitou."
  },
  "npc.prose.8d392f3805daa088": {
    "en": "Taoist skill page 1:",
    "zh-TW": "道士技能第 1 頁：",
    "pt-BR": "Habilidades de taoísta, página 1:"
  },
  "npc.prose.8d5258333f86f47f": {
    "en": "Welcome {npc_arg0}, I Purchase Awakening Items",
    "zh-TW": "歡迎，{npc_arg0}。我收購覺醒物品。",
    "pt-BR": "Boas-vindas, {npc_arg0}. Compro itens de despertar."
  },
  "npc.prose.8d6cab7515ae3a1f": {
    "en": "seen if unless do not move or attack after exercising this skill.",
    "zh-TW": "施展此技能後，只要不移動或攻擊，就不會被看見。",
    "pt-BR": "visto, desde que não se mova nem ataque após usar a habilidade."
  },
  "npc.prose.8e20245c7e3309de": {
    "en": "govern these lands.",
    "zh-TW": "治理這些土地。",
    "pt-BR": "governar estas terras."
  },
  "npc.prose.8e4f3c84d3de1d77": {
    "en": "Success rate depends on the skill level. It requires an Amulet of Revival.",
    "zh-TW": "成功率取決於技能等級，並需要復活護身符。",
    "pt-BR": "A chance de sucesso depende do nível da habilidade. Requer um Talismã de Ressurreição."
  },
  "npc.prose.8e9da1bffffce8a5": {
    "en": "and I'll take alook.",
    "zh-TW": "我會看看。",
    "pt-BR": "e darei uma olhada."
  },
  "npc.prose.8efb83b6c451c717": {
    "en": "about how to gain meat.",
    "zh-TW": "關於如何取得肉。",
    "pt-BR": "sobre como obter carne."
  },
  "npc.prose.8f08f690957d9e36": {
    "en": "southwest of WoomyonWoods.",
    "zh-TW": "沃瑪森林的西南方。",
    "pt-BR": "a sudoeste do Bosque de Woomyon."
  },
  "npc.prose.8f1edac9b0c5beb9": {
    "en": "Only Taoists can use tallisman or poison as one of the skills.",
    "zh-TW": "只有道士能在技能中使用護身符或毒藥。",
    "pt-BR": "Somente taoístas podem usar talismãs ou veneno em suas habilidades."
  },
  "npc.prose.8f5d37e6b07a41a9": {
    "en": "recovered from the wound Therefore you who are not affected",
    "zh-TW": "傷勢恢復。因此，未受影響的你",
    "pt-BR": "se recuperou da ferida. Portanto, você, que não foi afetado,"
  },
  "npc.prose.8f7028f948240290": {
    "en": "I stabbed the RedMoonEvil I was showered with its blood and",
    "zh-TW": "我刺中赤月惡魔時，牠的血淋了我一身，並且",
    "pt-BR": "Ao apunhalar o Demônio da Lua Vermelha, fui banhado por seu sangue e"
  },
  "npc.prose.8f8514efa94ab90f": {
    "en": "*Shout                             *Block guild chat",
    "zh-TW": "＊喊話　　＊屏蔽行會聊天",
    "pt-BR": "*Gritar  *Bloquear conversa da guilda"
  },
  "npc.prose.8f8f2f382c7620b2": {
    "en": "RedMoonSword. Ugh everything was my fault.",
    "zh-TW": "赤月劍。唉，一切都是我的錯。",
    "pt-BR": "a Espada da Lua Vermelha. Ah, tudo foi culpa minha."
  },
  "npc.prose.8f9a0c31d03ff974": {
    "en": "have their own territories can be transported.",
    "zh-TW": "擁有領地的人才能被傳送。",
    "pt-BR": "possuem seus próprios territórios podem ser transportados."
  },
  "npc.prose.8fa6cac907a261f7": {
    "en": "Buy.",
    "zh-TW": "購買。",
    "pt-BR": "Comprar."
  },
  "npc.prose.8fcb2f12320507d2": {
    "en": "soldiers dying at the battlefield while fighting against",
    "zh-TW": "在戰場上與敵人作戰而犧牲的士兵",
    "pt-BR": "soldados morrendo no campo de batalha enquanto lutam contra"
  },
  "npc.prose.900abedeb01e170b": {
    "en": "Paralyze enemies within 5x5 square range for certain probability.",
    "zh-TW": "有一定機率麻痺 5×5 格範圍內的敵人。",
    "pt-BR": "Tem uma chance de paralisar inimigos em uma área de 5×5 casas."
  },
  "npc.prose.9078f20bb30db40b": {
    "en": "And boy o boy doesn't she talk a lot of rubbish..",
    "zh-TW": "天啊，她真是說了一堆廢話……",
    "pt-BR": "Nossa, como ela fala bobagens..."
  },
  "npc.prose.91155adac24f0ef2": {
    "en": "In a book written 100 years ago,there is a section where it",
    "zh-TW": "一本百年前寫成的書中，有一章",
    "pt-BR": "Em um livro escrito há 100 anos, há uma seção em que"
  },
  "npc.prose.915f135c9cf1be1e": {
    "en": "Welcome. What can I do for you?",
    "zh-TW": "歡迎，有什麼能為你效勞？",
    "pt-BR": "Boas-vindas. Como posso ajudar?"
  },
  "npc.prose.917423e21f9d8e70": {
    "en": "What kind of Poison would you like to purchase?",
    "zh-TW": "你想購買哪種毒藥？",
    "pt-BR": "Que tipo de veneno deseja comprar?"
  },
  "npc.prose.919d119acc4378fb": {
    "en": "Jewellery.",
    "zh-TW": "首飾。",
    "pt-BR": "Joias."
  },
  "npc.prose.91fe72a1d8bf673b": {
    "en": "This skill requires long casting time and cast can be cancelled if attacked.",
    "zh-TW": "此技能施法時間較長，受到攻擊時可能被中斷。",
    "pt-BR": "Esta habilidade demora para ser conjurada e pode ser cancelada se você for atacado."
  },
  "npc.prose.9202c74c1d9f5a9b": {
    "en": "We should get rid of it absolutely. Now keep in mind what I say",
    "zh-TW": "我們一定要除掉牠。現在記住我說的話",
    "pt-BR": "Precisamos eliminá-lo de vez. Agora lembre-se do que digo."
  },
  "npc.prose.924863d66ebe6d86": {
    "en": "Test what you are capable of by defeating the monsters.",
    "zh-TW": "擊敗怪物，測試自己的實力。",
    "pt-BR": "Teste sua capacidade derrotando os monstros."
  },
  "npc.prose.9263fdd602313131": {
    "en": "(KR)",
    "zh-TW": "（韓國）",
    "pt-BR": "(Coreia)"
  },
  "npc.prose.926f60ad2511629d": {
    "en": "(He's a drunk... lets ignore hine.)",
    "zh-TW": "（他喝醉了……別理他。）",
    "pt-BR": "(Ele está bêbado... Vamos ignorá-lo.)"
  },
  "npc.prose.928b978216ff2b8c": {
    "en": "The howling coming from within this Cave sends shivers",
    "zh-TW": "洞穴裡傳來的嚎叫令人毛骨悚然",
    "pt-BR": "Os uivos vindos desta caverna causam arrepios."
  },
  "npc.prose.92a080879591a00e": {
    "en": "I will gladly die for the general, if I can protect",
    "zh-TW": "若能保護，我願為將軍犧牲",
    "pt-BR": "Eu morreria de bom grado pelo general, se pudesse proteger"
  },
  "npc.prose.92d1776accaebc3c": {
    "en": "Menu:  -",
    "zh-TW": "選單：－",
    "pt-BR": "Menu: —"
  },
  "npc.prose.931b3a5e78ed90d8": {
    "en": "the files have been used and will continue to be used by many.",
    "zh-TW": "這些檔案一直有許多人使用，也將繼續如此。",
    "pt-BR": "os arquivos foram usados por muitos e continuarão sendo."
  },
  "npc.prose.935b9c3c46f7433c": {
    "en": "You are the true hero can save the continent",
    "zh-TW": "你是能拯救這片大陸的真正英雄",
    "pt-BR": "Você é o verdadeiro herói capaz de salvar o continente."
  },
  "npc.prose.9407c99849c4f297": {
    "en": "So what do you say?",
    "zh-TW": "那麼，你怎麼說？",
    "pt-BR": "Então, o que me diz?"
  },
  "npc.prose.942c6cf8eb70a553": {
    "en": "been until then And I found something in a ugly, wet, shady and",
    "zh-TW": "直到那時。而我在一個醜陋、潮濕、陰暗又",
    "pt-BR": "até então. E encontrei algo em um lugar feio, úmido, sombrio e"
  },
  "npc.prose.94a59e14c711b5da": {
    "en": "my blood boil. You may not be aware of it since you are still",
    "zh-TW": "讓我的血液沸騰。你或許還不知道，因為你仍然",
    "pt-BR": "faz meu sangue ferver. Talvez você ainda não saiba, pois ainda é"
  },
  "npc.prose.94a5b8bcd44a3e02": {
    "en": "purchase the guild territory after reaching agreement,",
    "zh-TW": "達成協議後購買行會領地，",
    "pt-BR": "comprar o território da guilda após chegar a um acordo,"
  },
  "npc.prose.94c1e010eb0070c4": {
    "en": "$ {npc_arg0} Gold                                              % {npc_arg1} Gold",
    "zh-TW": "金額 {npc_arg0} 金幣　　比例 {npc_arg1} 金幣",
    "pt-BR": "Valor: {npc_arg0} de ouro  Percentual: {npc_arg1} de ouro"
  },
  "npc.prose.9522a2dcfd7d19c6": {
    "en": "the monster.. run away!",
    "zh-TW": "那隻怪物……快逃！",
    "pt-BR": "o monstro... fuja!"
  },
  "npc.prose.95624911fe09e335": {
    "en": "Bout for unlimited challenges?",
    "zh-TW": "可無限挑戰的比試？",
    "pt-BR": "Um desafio com tentativas ilimitadas?"
  },
  "npc.prose.9562815fd8b147e4": {
    "en": "about the monsters",
    "zh-TW": "關於怪物",
    "pt-BR": "sobre os monstros"
  },
  "npc.prose.956895965ee19b75": {
    "en": "instantly damaging every enemy in range.",
    "zh-TW": "立即傷害範圍內的所有敵人。",
    "pt-BR": "causando dano instantâneo a todos os inimigos no alcance."
  },
  "npc.prose.957b98cca163c67d": {
    "en": "Hmm... Think ive seen those symbols before...",
    "zh-TW": "嗯……我好像以前見過那些符號……",
    "pt-BR": "Hmm... Acho que já vi esses símbolos antes..."
  },
  "npc.prose.960922d4ec924082": {
    "en": "Listen to the  about skills.",
    "zh-TW": "聽取關於技能的說明。",
    "pt-BR": "Ouça as explicações sobre habilidades."
  },
  "npc.prose.9639b7a273f957b4": {
    "en": "4) Detailed explanation is displayed in skill type section.",
    "zh-TW": "4）技能類型區域會顯示詳細說明。",
    "pt-BR": "4) A explicação detalhada aparece na seção do tipo de habilidade."
  },
  "npc.prose.963e3f9ecffa156c": {
    "en": "Are you still willing to go ahead?",
    "zh-TW": "你仍然願意繼續嗎？",
    "pt-BR": "Ainda deseja prosseguir?"
  },
  "npc.prose.965dc3d26c1693b9": {
    "en": "I am the Assassin-Level Warrior, it's good to see you.",
    "zh-TW": "我是刺客級戰士，很高興見到你。",
    "pt-BR": "Sou o guerreiro do nível Assassino. É bom ver você."
  },
  "npc.prose.9683744db6628e2e": {
    "en": "I'll use this",
    "zh-TW": "我會使用這個",
    "pt-BR": "Vou usar isto."
  },
  "npc.prose.96960973896f0eef": {
    "en": "\"You are a party leader but your party is not present.",
    "zh-TW": "「你是隊長，但你的隊伍不在場。",
    "pt-BR": "“Você é líder de grupo, mas seu grupo não está presente."
  },
  "npc.prose.9704c6982055a55d": {
    "en": "In order to qualify for guild creation",
    "zh-TW": "為了取得建立行會的資格",
    "pt-BR": "Para se qualificar para criar uma guilda,"
  },
  "npc.prose.972c21eae685683d": {
    "en": "I wonder... No that's crazy talk!",
    "zh-TW": "我在想……不，那太瘋狂了！",
    "pt-BR": "Fico pensando... Não, isso é loucura!"
  },
  "npc.prose.973def9ae63b2916": {
    "en": "I'm so honored to meet you.",
    "zh-TW": "很榮幸見到你。",
    "pt-BR": "É uma grande honra conhecer você."
  },
  "npc.prose.9757dd54fa92b1dc": {
    "en": "2) Double click at skill book in your inventory.",
    "zh-TW": "2）雙擊背包中的技能書。",
    "pt-BR": "2) Clique duas vezes no livro de habilidade na bolsa."
  },
  "npc.prose.97676b57a9f8961f": {
    "en": "Your right. I tried hiring a HolySword, the best sword of all, from",
    "zh-TW": "你說得對。我曾想借用最強的聖劍，向",
    "pt-BR": "Você tem razão. Tentei alugar a Espada Sagrada, a melhor de todas, de"
  },
  "npc.prose.97a68815a705dfc3": {
    "en": "I'll send you back to the village with a prize.",
    "zh-TW": "我會送你回村莊，並給你獎勵。",
    "pt-BR": "Vou enviar você de volta à vila com um prêmio."
  },
  "npc.prose.98e9ea9deeecf87a": {
    "en": "5x5 square range attack around player(max.24 enemies).",
    "zh-TW": "攻擊玩家周圍 5×5 格範圍，最多 24 個敵人。",
    "pt-BR": "Ataca uma área de 5×5 casas ao redor do jogador, atingindo até 24 inimigos."
  },
  "npc.prose.990b515b07f6f50b": {
    "en": "Maybe you could do something for me?",
    "zh-TW": "或許你能替我做件事？",
    "pt-BR": "Talvez possa fazer algo por mim?"
  },
  "npc.prose.9927d24498a95e00": {
    "en": "Well I refuse to tell anyone about the Emperor!",
    "zh-TW": "我拒絕向任何人透露皇帝的事！",
    "pt-BR": "Recuso-me a contar a qualquer pessoa sobre o imperador!"
  },
  "npc.prose.9957033d861c6481": {
    "en": "Passengers, Please board the ship.",
    "zh-TW": "旅客們，請登船。",
    "pt-BR": "Passageiros, por favor, embarquem."
  },
  "npc.prose.9992d5ada808d8b6": {
    "en": "The trap Hexagon emits light to indicate its boundary.",
    "zh-TW": "六角陷阱會發光，標示它的邊界。",
    "pt-BR": "A armadilha hexagonal emite luz para indicar seus limites."
  },
  "npc.prose.99b4ceb61eff84df": {
    "en": "I will show them a crazy old fool.",
    "zh-TW": "我會讓他們看看什麼叫瘋老頭。",
    "pt-BR": "Vou mostrar a eles o que é um velho maluco."
  },
  "npc.prose.99d99ff513b59071": {
    "en": "All rebirth flags cleared.",
    "zh-TW": "已清除所有轉生標記。",
    "pt-BR": "Todas as marcas de renascimento foram removidas."
  },
  "npc.prose.9a0fcc78454c59bc": {
    "en": "be regarded as PK.",
    "zh-TW": "會被視為殺害玩家。",
    "pt-BR": "será considerado assassinato de jogador."
  },
  "npc.prose.9a5961a79a1daf83": {
    "en": "are greatly admired.",
    "zh-TW": "備受敬仰。",
    "pt-BR": "são muito admirados."
  },
  "npc.prose.9a77233212be6a88": {
    "en": "Welcome Traveller, my name is Wayne I'm a Master Artisan..",
    "zh-TW": "歡迎，旅人。我叫韋恩，是一位工藝大師……",
    "pt-BR": "Boas-vindas, viajante. Meu nome é Wayne, sou um mestre artesão..."
  },
  "npc.prose.9ae17e894aae6ee5": {
    "en": "But there are still more to go.",
    "zh-TW": "但還有更多路要走。",
    "pt-BR": "Mas ainda há muito pela frente."
  },
  "npc.prose.9b42d075f7eac7f6": {
    "en": "When , war will be allowed for 3 hours.",
    "zh-TW": "之後，戰爭將獲准持續 3 小時。",
    "pt-BR": "depois disso, a guerra será permitida por 3 horas."
  },
  "npc.prose.9b68a64de5603957": {
    "en": "Those acquired minerals will be used as raw material for",
    "zh-TW": "取得的礦石可作為原料，用於",
    "pt-BR": "Os minérios obtidos servirão de matéria-prima para"
  },
  "npc.prose.9ba9ab635ce4b60d": {
    "en": "a new strategy to deal with the Omas.",
    "zh-TW": "對付半獸人的新戰略。",
    "pt-BR": "uma nova estratégia para enfrentar os Omas."
  },
  "npc.prose.9cc1ff1b774dd4a9": {
    "en": "If the weapon was not expensive,",
    "zh-TW": "若武器並不昂貴，",
    "pt-BR": "Se a arma não era cara,"
  },
  "npc.prose.9ccbcd477d770aca": {
    "en": "Please don't try trick me.",
    "zh-TW": "請別試圖欺騙我。",
    "pt-BR": "Por favor, não tente me enganar."
  },
  "npc.prose.9d11b6230baf81b8": {
    "en": "need BlackIronOre, Jewelery, Weapon and Gold. Do you have these",
    "zh-TW": "需要黑鐵礦、首飾、武器及金幣。你有這些",
    "pt-BR": "precisa de Minério de Ferro Negro, joias, uma arma e ouro. Você tem esses"
  },
  "npc.prose.9d4c4629f0ca287d": {
    "en": "to all targets within a 5×5 radius of the archer.",
    "zh-TW": "對弓箭手周圍 5×5 範圍內的所有目標。",
    "pt-BR": "a todos os alvos em uma área de 5×5 ao redor do arqueiro."
  },
  "npc.prose.9d8f86d862b53b76": {
    "en": "Your PK POINTS: {npc_arg0}",
    "zh-TW": "你的殺人值：{npc_arg0}",
    "pt-BR": "Seus pontos de PK: {npc_arg0}"
  },
  "npc.prose.9d96d049c7cc5d5b": {
    "en": "fierece attacks from the Omas.",
    "zh-TW": "半獸人的猛烈攻擊。",
    "pt-BR": "ataques ferozes dos Omas."
  },
  "npc.prose.9dc54382a8f2dc4b": {
    "en": "then it's best I don't help you get to his lair.",
    "zh-TW": "那我最好別幫你前往牠的巢穴。",
    "pt-BR": "então é melhor eu não ajudar você a chegar ao covil dele."
  },
  "npc.prose.9e438ff355c94202": {
    "en": "preventing the crossed force from Bottomless Pit breaking out..",
    "zh-TW": "阻止無底深淵的交錯力量爆發……",
    "pt-BR": "impedindo que a força cruzada do Poço sem Fundo escape..."
  },
  "npc.prose.9e4f558ada5c4e90": {
    "en": "( SpiderBat, ToxicGhoul, Dung, etc )",
    "zh-TW": "（蝙蝠蜘蛛、毒鬼、糞蛆等）",
    "pt-BR": "(Aranha-Morcego, Carniçal Tóxico, Verme de Esterco etc.)"
  },
  "npc.prose.9e518ea32138c8e8": {
    "en": "Lay down your Fish to be sold",
    "zh-TW": "把要出售的魚放下吧",
    "pt-BR": "Coloque aqui os peixes que deseja vender."
  },
  "npc.prose.9effd6f49b1e9e8d": {
    "en": "Attacks enemies by calling a thunderbolt that will hit the target from the sky.",
    "zh-TW": "召喚從天而降的雷電攻擊目標。",
    "pt-BR": "Ataca o inimigo invocando um raio que atinge o alvo vindo do céu."
  },
  "npc.prose.9f2a24463b88d047": {
    "en": "What Posion's do you wish to buy?",
    "zh-TW": "你想購買哪些毒藥？",
    "pt-BR": "Quais venenos deseja comprar?"
  },
  "npc.prose.9f3cfacb1edd88e9": {
    "en": "It's difficult to kill the monster.. If you face",
    "zh-TW": "這隻怪物很難殺死……如果你遇到",
    "pt-BR": "É difícil matar o monstro... Se você o encontrar,"
  },
  "npc.prose.9f7902e6473ed1c2": {
    "en": "make you randomly move to another place in the same map.",
    "zh-TW": "讓你隨機移動到同一張地圖上的其他位置。",
    "pt-BR": "move você para outro lugar aleatório do mesmo mapa."
  },
  "npc.prose.9f87e4193c518bc0": {
    "en": "so they can be there for anyone struggling,",
    "zh-TW": "讓他們能陪伴任何正在掙扎的人，",
    "pt-BR": "para que possam apoiar qualquer pessoa que esteja enfrentando dificuldades,"
  },
  "npc.prose.9f8a0ca4f76a8831": {
    "en": "Please speak with the Local Vendor's im sure they would welcome any",
    "zh-TW": "請向當地商販詢問，我相信他們會歡迎任何",
    "pt-BR": "Fale com os vendedores locais; tenho certeza de que receberão bem qualquer"
  },
  "npc.prose.9fb7d12f9128bd1a": {
    "en": "You are already Rebirth 3.",
    "zh-TW": "你已完成第三次轉生。",
    "pt-BR": "Você já possui o terceiro renascimento."
  },
  "npc.prose.9fd7ea6962d1748f": {
    "en": "Push away enemies by charging them with his shoulder, inflicting damage if",
    "zh-TW": "以肩膀衝撞擊退敵人，若符合條件則造成傷害",
    "pt-BR": "Empurra inimigos com uma investida de ombro, causando dano se"
  },
  "npc.prose.9ff4d79538cc72ac": {
    "en": "You dont have a Premium Pass.",
    "zh-TW": "你沒有進階通行證。",
    "pt-BR": "Você não tem um passe premium."
  },
  "npc.prose.9ff8ff5ef9ac3964": {
    "en": "sword, the best sword of this school, and dissapeared.",
    "zh-TW": "劍，那是本門最好的劍，然後便消失了。",
    "pt-BR": "espada, a melhor desta escola, e desapareceu."
  },
  "npc.prose.a0020a5a5ad45c46": {
    "en": "Hello I'm the inspector dispatched from Bichon.",
    "zh-TW": "你好，我是比奇派來的督察。",
    "pt-BR": "Olá, sou o inspetor enviado de Bichon."
  },
  "npc.prose.a01be2e69e3d32a1": {
    "en": "Stuff",
    "zh-TW": "用品",
    "pt-BR": "Artigos"
  },
  "npc.prose.a02d244b2ff0e889": {
    "en": "We were brought here for the War.",
    "zh-TW": "我們因戰爭被帶到這裡。",
    "pt-BR": "Fomos trazidos para cá por causa da guerra."
  },
  "npc.prose.a07f623e632921c9": {
    "en": "Item.",
    "zh-TW": "物品。",
    "pt-BR": "Item.",
    "sameValueReason": "Item is the correct Portuguese noun, identical to the English spelling."
  },
  "npc.prose.a0859f8ece47b05f": {
    "en": "I'm happy to do it for 20,000 gold.",
    "zh-TW": "我很樂意為你服務，費用是 20,000 金幣。",
    "pt-BR": "Faço isso com prazer por 20.000 moedas de ouro."
  },
  "npc.prose.a0961f4473282ef2": {
    "en": "Please select what you want to buy",
    "zh-TW": "請選擇想購買的物品",
    "pt-BR": "Selecione o que deseja comprar."
  },
  "npc.prose.a104399a48e8d1f0": {
    "en": "Please leave this guild before trying again.",
    "zh-TW": "請先退出此行會，再試一次。",
    "pt-BR": "Saia desta guilda antes de tentar novamente."
  },
  "npc.prose.a1a640ff24c7e836": {
    "en": "you need to bring me several items:-",
    "zh-TW": "你需要帶幾件物品給我：",
    "pt-BR": "você precisa me trazer vários itens:"
  },
  "npc.prose.a1b7b39b9e062daa": {
    "en": "leaves its view range.",
    "zh-TW": "離開牠的視野範圍。",
    "pt-BR": "sair do campo de visão dele."
  },
  "npc.prose.a1c8a0e2e6321878": {
    "en": "This is the palace of Bichon wall.",
    "zh-TW": "這裡是比奇城的皇宮。",
    "pt-BR": "Este é o palácio da muralha de Bichon."
  },
  "npc.prose.a21d72b56ba31e73": {
    "en": "Holysword? Huh.. don't believe in the apprentice's words",
    "zh-TW": "聖劍？哼……別相信那個學徒的話",
    "pt-BR": "Espada Sagrada? Hã... Não acredite nas palavras do aprendiz."
  },
  "npc.prose.8a30a8fdeadb9b8d": {
    "en": "[@before1>",
    "zh-TW": "[@before1>",
    "pt-BR": "[@before1>",
    "sameValueReason": "Legacy structural marker or layout punctuation, not player prose."
  },
  "npc.prose.8e7049fb78f0e841": {
    "en": ",  ,  ,",
    "zh-TW": ",  ,  ,",
    "pt-BR": ",  ,  ,",
    "sameValueReason": "Legacy structural marker or layout punctuation, not player prose."
  },
  "npc.prose.992734415948b811": {
    "en": ",  ,",
    "zh-TW": ",  ,",
    "pt-BR": ",  ,",
    "sameValueReason": "Legacy structural marker or layout punctuation, not player prose."
  },
  "npc.prose.a268212c6f0b6a4f": {
    "en": "young. Do you want to hear about Abel?",
    "zh-TW": "年輕。你想聽亞伯的故事嗎？",
    "pt-BR": "jovem. Quer ouvir a história de Abel?"
  },
  "npc.prose.a26c5b65fd5cf9c9": {
    "en": "You may pass hunter!",
    "zh-TW": "獵人，你可以通過！",
    "pt-BR": "Pode passar, caçador!"
  },
  "npc.prose.a27b7447040542c8": {
    "en": "Thunderbolt of Wizard is very effective in this place.",
    "zh-TW": "法師的雷電術在這裡非常有效。",
    "pt-BR": "O Relâmpago dos magos é muito eficaz aqui."
  },
  "npc.prose.a282b3422be21462": {
    "en": "Pay 3000 Gold  start>",
    "zh-TW": "支付 3,000 金幣，開始＞",
    "pt-BR": "Pague 3.000 moedas de ouro. Iniciar >"
  },
  "npc.prose.a29b7f31c96a3dd7": {
    "en": "Teleport to:",
    "zh-TW": "傳送至：",
    "pt-BR": "Teleportar para:"
  },
  "npc.prose.a309ee7f91abe847": {
    "en": "I had to seek refuge in this town.. My Cabin is gone.",
    "zh-TW": "我不得不到這個城鎮避難……我的小屋沒了。",
    "pt-BR": "Tive de buscar refúgio nesta cidade... Minha cabana se foi."
  },
  "npc.prose.a331d694d6496df3": {
    "en": "Create a powerful column on enemy no matter of any obstacles.",
    "zh-TW": "無視障礙物，在敵人身上形成強力光柱。",
    "pt-BR": "Cria uma coluna poderosa sobre o inimigo, independentemente dos obstáculos."
  },
  "npc.prose.a347cb300b3b3e42": {
    "en": "Be safe traveler.",
    "zh-TW": "旅人，保重。",
    "pt-BR": "Cuide-se, viajante."
  },
  "npc.prose.a3fc180b273a026a": {
    "en": "help.",
    "zh-TW": "幫助。",
    "pt-BR": "ajuda."
  },
  "npc.prose.a4235c99911c71b3": {
    "en": "Traveller, I will purchase any Stones you have.",
    "zh-TW": "旅人，我願意收購你身上的任何石頭。",
    "pt-BR": "Viajante, compro quaisquer pedras que você tenha."
  },
  "npc.prose.a467df464b1c3a2a": {
    "en": "you can not see anything.",
    "zh-TW": "你什麼也看不見。",
    "pt-BR": "você não consegue ver nada."
  },
  "npc.prose.a4a90aca26c7c060": {
    "en": "Item: You can acquire some item from dead body of some",
    "zh-TW": "物品：你可以從某些屍體上取得物品",
    "pt-BR": "Itens: você pode obter itens nos corpos de alguns"
  },
  "npc.prose.a4d6b3ff3ace926e": {
    "en": "will be deleted and no refund made.",
    "zh-TW": "將被刪除，且不予退款。",
    "pt-BR": "serão removidos, sem reembolso."
  },
  "npc.prose.a563cfacd54a27ed": {
    "en": "Hello. I am the 'Commission Merchant'. I am in charge",
    "zh-TW": "你好，我是寄售商人。我負責",
    "pt-BR": "Olá. Sou o comerciante de consignação. Sou responsável"
  },
  "npc.prose.a56a01605febf9d6": {
    "en": "Please select the Book you either want to Buy or Sell.",
    "zh-TW": "請選擇你想購買或出售的書。",
    "pt-BR": "Selecione o livro que deseja comprar ou vender."
  },
  "npc.prose.a57ea3520308e034": {
    "en": "The missing Trainee.. Not much left on him anymore.",
    "zh-TW": "失蹤的學徒……他的遺體所剩無幾。",
    "pt-BR": "O aprendiz desaparecido... Pouco restou dele."
  },
  "npc.prose.a5f81adeec96dea6": {
    "en": "Exchange:  into Gold - Commission (20000 Gold)",
    "zh-TW": "兌換：換成金幣－手續費 20,000 金幣",
    "pt-BR": "Troca: por ouro — comissão de 20.000 moedas de ouro"
  },
  "npc.prose.a606af32252877cd": {
    "en": "There is already someone fighting!",
    "zh-TW": "已經有人在戰鬥了！",
    "pt-BR": "Já há alguém lutando!"
  },
  "npc.prose.a61a4d9a0f1335d6": {
    "en": "Your level is to high to fight here.",
    "zh-TW": "你的等級太高，不能在此戰鬥。",
    "pt-BR": "Seu nível é alto demais para lutar aqui."
  },
  "npc.prose.a667886297e593f8": {
    "en": "We are not collecting donation money yet.",
    "zh-TW": "我們尚未開始收取捐款。",
    "pt-BR": "Ainda não estamos arrecadando doações."
  },
  "npc.prose.a6728f647a952a00": {
    "en": "You do not have the DragonScale!",
    "zh-TW": "你沒有龍鱗！",
    "pt-BR": "Você não tem a Escama de Dragão!"
  },
  "npc.prose.a6f20789b0c5d19b": {
    "en": "You don't have enough funds..",
    "zh-TW": "你的資金不足……",
    "pt-BR": "Você não tem fundos suficientes..."
  },
  "npc.prose.a72da03ab7d47ce8": {
    "en": "Trapped mob can evade it when they are attacked from distance.",
    "zh-TW": "被困的怪物受到遠距攻擊時，可能逃出陷阱。",
    "pt-BR": "Um monstro preso pode escapar se for atacado à distância."
  },
  "npc.prose.a73f335fafd0d97d": {
    "en": "This is the Guild Territory Bulletin Board.",
    "zh-TW": "這是行會領地公告板。",
    "pt-BR": "Este é o mural do território da guilda."
  },
  "npc.prose.a7870fad86540537": {
    "en": "Just pay the fee then ill escort you to anywhere.",
    "zh-TW": "付清費用後，我就會護送你去任何地方。",
    "pt-BR": "Basta pagar a taxa e eu acompanharei você a qualquer lugar."
  },
  "npc.prose.a7974e233c19b8d6": {
    "en": "Tell me your wishes.",
    "zh-TW": "告訴我你的願望。",
    "pt-BR": "Diga quais são seus desejos."
  },
  "npc.prose.a86c8099dc489817": {
    "en": "Bichon Token.",
    "zh-TW": "比奇代幣。",
    "pt-BR": "Ficha de Bichon."
  },
  "npc.prose.a88953b21ca5ad3d": {
    "en": "Passively increases chance to hit with physical attacks.",
    "zh-TW": "被動提高物理攻擊的命中率。",
    "pt-BR": "Aumenta passivamente a chance de acertar ataques físicos."
  },
  "npc.prose.a8e436b2c0b1fff8": {
    "en": "HolySword could not protect me any more due to demon energy growning",
    "zh-TW": "隨著魔氣增長，聖劍已無法繼續保護我",
    "pt-BR": "A Espada Sagrada já não podia me proteger, pois a energia demoníaca crescia."
  },
  "npc.prose.a92403a37bce97f1": {
    "en": "Please press ('H') to bring up the help menu.",
    "zh-TW": "請按 H 開啟說明選單。",
    "pt-BR": "Pressione H para abrir o menu de ajuda."
  },
  "npc.prose.a9293df11edb54d2": {
    "en": "Also you will need the Bichon Token to go thought the cave.",
    "zh-TW": "此外，穿越洞穴還需要比奇代幣。",
    "pt-BR": "Você também precisará da Ficha de Bichon para atravessar a caverna."
  },
  "npc.prose.a9554ff6922d86ff": {
    "en": "(Decide which key will be used for quick slot.)",
    "zh-TW": "（設定要用於快捷欄的按鍵。）",
    "pt-BR": "(Defina qual tecla será usada no atalho.)"
  },
  "npc.prose.a95e9ce30988d011": {
    "en": "the player. Duration time increases are practices level.",
    "zh-TW": "玩家。持續時間隨熟練等級提高。",
    "pt-BR": "o jogador. A duração aumenta com o nível de prática."
  },
  "npc.prose.a9620bbc4f47918a": {
    "en": "If you are connected during a Guild War, the message:",
    "zh-TW": "若你在行會戰期間上線，訊息：",
    "pt-BR": "Se você estiver conectado durante uma guerra de guildas, a mensagem:"
  },
  "npc.prose.a964bed4558b714c": {
    "en": "Rebirth 1 -",
    "zh-TW": "第一次轉生－",
    "pt-BR": "Primeiro renascimento —"
  },
  "npc.prose.a99f2a861ffdcf36": {
    "en": "I'm afraid I dont know anything about that.",
    "zh-TW": "恐怕我對那件事一無所知。",
    "pt-BR": "Receio não saber nada sobre isso."
  },
  "npc.prose.a9c3a6db94e5cead": {
    "en": "love for the disappeared best sword of this school.",
    "zh-TW": "對本門失落名劍的珍愛。",
    "pt-BR": "o amor pela melhor espada desta escola, que desapareceu."
  },
  "npc.prose.a9de9742ec53d897": {
    "en": "no one came back. By perfroming magical astrology I found out there",
    "zh-TW": "沒有一個人回來。我透過占星術得知那裡",
    "pt-BR": "ninguém voltou. Por meio da astrologia mágica, descobri que lá"
  },
  "npc.prose.aa1e19bba331fb35": {
    "en": "A supreme skill of Taoists which can revive a dead player.",
    "zh-TW": "道士的高階技能，能使死亡的玩家復活。",
    "pt-BR": "Uma habilidade suprema dos taoístas, capaz de ressuscitar um jogador morto."
  },
  "npc.prose.aa36c121e8aed46f": {
    "en": "(Physical objects).",
    "zh-TW": "（物理物件）。",
    "pt-BR": "(Objetos físicos)."
  },
  "npc.prose.aa4c5e6b030d53d0": {
    "en": "ZumaHeart Required Level55~70.",
    "zh-TW": "祖瑪之心，適合 55～70 級。",
    "pt-BR": "Coração de Zuma: níveis exigidos de 55 a 70."
  },
  "npc.prose.aa7c1d1d418ca356": {
    "en": "By the time you get used to this place, go to PastBichon by the TimeStone",
    "zh-TW": "熟悉此地之後，可用時間石前往昔日比奇",
    "pt-BR": "Depois de se acostumar com este lugar, use a Pedra do Tempo para ir a Bichon do Passado."
  },
  "npc.prose.aac1c9c0b4aa7be5": {
    "en": "Traveller, You are lucky... We have the materials to do Special repairs.",
    "zh-TW": "旅人，你真幸運……我們有進行特殊修理所需的材料。",
    "pt-BR": "Viajante, você tem sorte... Temos materiais para reparos especiais."
  },
  "npc.prose.aaea554742825824": {
    "en": "How can i help you?",
    "zh-TW": "有什麼能幫你的？",
    "pt-BR": "Como posso ajudar?"
  },
  "npc.prose.aafc502718d7ea3b": {
    "en": "stole the HolySword went forward in to the Redmoon Valley across a",
    "zh-TW": "偷走聖劍後，穿過一條路，前往赤月峽谷",
    "pt-BR": "roubou a Espada Sagrada e seguiu para o Vale da Lua Vermelha, atravessando uma"
  },
  "npc.prose.ab6e9fd2ee65d64e": {
    "en": "You do not have Rebirth 1.",
    "zh-TW": "你尚未完成第一次轉生。",
    "pt-BR": "Você não possui o primeiro renascimento."
  },
  "npc.prose.abd0335ab5dc82a6": {
    "en": "his or her own familiar.",
    "zh-TW": "自己的使魔。",
    "pt-BR": "seu próprio familiar."
  },
  "npc.prose.ac0f570258c96e40": {
    "en": "though. The most helpful information i found was at the Palace.",
    "zh-TW": "不過，我找到最有用的資料是在皇宮。",
    "pt-BR": "porém. A informação mais útil que encontrei estava no palácio."
  },
  "npc.prose.ac2444cdde97ad43": {
    "en": "5. Trust money amount: can be set in the range of",
    "zh-TW": "5. 寄售金額：可設定的範圍為",
    "pt-BR": "5. Valor em consignação: pode ser definido no intervalo de"
  },
  "npc.prose.ac63bdb081bed194": {
    "en": "I open the closed door to you. You are about to enter the land",
    "zh-TW": "我為你打開封閉的門。你即將踏入這片土地",
    "pt-BR": "Abro a porta fechada para você. Está prestes a entrar na terra"
  },
  "npc.prose.ac6c7d16e3a7a375": {
    "en": "Pay 2,000 Gold and Board.",
    "zh-TW": "支付 2,000 金幣並登船。",
    "pt-BR": "Pague 2.000 moedas de ouro e embarque."
  },
  "npc.prose.ac88928922fd318e": {
    "en": "Please kill a RootSpider located in TreePath, Once you have completed",
    "zh-TW": "請殺死林間小徑的一隻樹妖蜘蛛，完成後",
    "pt-BR": "Mate uma Aranha-Raiz na Trilha das Árvores. Quando terminar,"
  },
  "npc.prose.ac8fcb5e4290a0cb": {
    "en": "blocked the way and the flow of blood became a river of red I",
    "zh-TW": "阻塞了道路，鮮血匯成紅色河流。我",
    "pt-BR": "bloquearam o caminho, e o sangue formou um rio vermelho. Eu"
  },
  "npc.prose.ac9961d5b3540ed4": {
    "en": "Greetings! You have come to purchase Creature Eggs, have you?",
    "zh-TW": "你好！你是來購買寵物蛋的吧？",
    "pt-BR": "Saudações! Veio comprar ovos de criaturas, não é?"
  },
  "npc.prose.acedddf1729d3782": {
    "en": "Crafting Material",
    "zh-TW": "製作材料",
    "pt-BR": "Material de fabricação"
  },
  "npc.prose.ad02c20f46bbb310": {
    "en": "Archer Seven                          Archer Ten",
    "zh-TW": "弓箭手七　　弓箭手十",
    "pt-BR": "Arqueiro Sete  Arqueiro Dez"
  },
  "npc.prose.ad317f5ff8df9399": {
    "en": "Prajna field, PrajnaStoneCave are good for the first",
    "zh-TW": "般若原野與般若石窟適合前期",
    "pt-BR": "Os Campos de Prajna e a Caverna de Pedra de Prajna são bons para a primeira"
  },
  "npc.prose.ad7994bc22bbc4fb": {
    "en": "Good! you are a true Mirian fighter!",
    "zh-TW": "很好！你是真正的瑪法勇士！",
    "pt-BR": "Muito bem! Você é um verdadeiro guerreiro de Mir!"
  },
  "npc.prose.ad828c27ce4dcca8": {
    "en": "\"I can  you Gold to let me past.\"",
    "zh-TW": "「我可以給你金幣，讓我通過。」",
    "pt-BR": "“Posso dar ouro a você para me deixar passar.”"
  },
  "npc.prose.adeb7c14f47fd062": {
    "en": "Tree path. Then a big group of heroes went there to pursue him but",
    "zh-TW": "林間小徑。之後，大批英雄前去追捕他，但",
    "pt-BR": "Trilha das Árvores. Depois, um grande grupo de heróis foi persegui-lo, mas"
  },
  "npc.prose.adf17ff270fe619c": {
    "en": "{npc_arg0}, I'm The Watcher of this aweful place.",
    "zh-TW": "{npc_arg0}，我是這個可怕地方的守望者。",
    "pt-BR": "{npc_arg0}, sou o vigia deste lugar terrível."
  },
  "npc.prose.ae58f09e85d3a72e": {
    "en": "Special repair.",
    "zh-TW": "特殊修理。",
    "pt-BR": "Reparo especial."
  },
  "npc.prose.ae58f49bcbffcb94": {
    "en": "\"Obtain the scroll and bring it to me.\"",
    "zh-TW": "「取得卷軸，並把它帶給我。」",
    "pt-BR": "“Obtenha o pergaminho e traga-o para mim.”"
  },
  "npc.prose.ae74a43a5a0c5ccf": {
    "en": "in 2003, He released multiple servers throughout his time with us.",
    "zh-TW": "2003 年，他在與我們共度的歲月中發布了多個伺服器。",
    "pt-BR": "em 2003. Ele lançou vários servidores durante o tempo que passou conosco."
  },
  "npc.prose.aeff50880a2d46aa": {
    "en": "A missing Carriage. Content's seems to be missing.",
    "zh-TW": "一輛失蹤的馬車，裡面的東西似乎不見了。",
    "pt-BR": "Uma carroça desaparecida. O conteúdo parece ter sumido."
  },
  "npc.prose.af4426c4254495c0": {
    "en": "Exchange:  into Gold - Commission (2000 Gold)",
    "zh-TW": "兌換：換成金幣－手續費 2,000 金幣",
    "pt-BR": "Troca: por ouro — comissão de 2.000 moedas de ouro"
  },
  "npc.prose.af6daa3316a879da": {
    "en": "Upgrade version of hiding skill which also affect other players to be invisible.",
    "zh-TW": "隱身術的強化版，也能使其他玩家隱身。",
    "pt-BR": "Uma versão aprimorada da ocultação, que também torna outros jogadores invisíveis."
  },
  "npc.prose.af98d35f8b9caad8": {
    "en": "WoomaHeart Required Level 43~48.",
    "zh-TW": "沃瑪之心，適合 43～48 級。",
    "pt-BR": "Coração de Wooma: níveis exigidos de 43 a 48."
  },
  "npc.prose.aff0da8d269b823c": {
    "en": "Rebirth 1 granted.",
    "zh-TW": "已授予第一次轉生。",
    "pt-BR": "Primeiro renascimento concedido."
  },
  "npc.prose.b01dee7b26a7c5c4": {
    "en": "If you wish for higher exp cave. Go OmaCave northwest",
    "zh-TW": "若想在洞穴取得更多經驗，前往西北方的半獸人洞穴",
    "pt-BR": "Para ganhar mais experiência em cavernas, vá à Caverna dos Omas, a noroeste"
  },
  "npc.prose.b02ec3fc276537f1": {
    "en": "More to come.",
    "zh-TW": "更多內容即將推出。",
    "pt-BR": "Mais novidades em breve."
  },
  "npc.prose.b0837092a581d009": {
    "en": "Please refer to the whole map by pressing B.",
    "zh-TW": "請按 B 查看完整地圖。",
    "pt-BR": "Pressione B para consultar o mapa completo."
  },
  "npc.prose.b0916d6b07626378": {
    "en": "Defenitly there is such a legend from the past but...",
    "zh-TW": "過去確實有這樣的傳說，但是……",
    "pt-BR": "Realmente existe uma lenda assim, mas..."
  },
  "npc.prose.b094b46fa1f748e3": {
    "en": "Warrior skill type:",
    "zh-TW": "戰士技能類型：",
    "pt-BR": "Tipos de habilidades de guerreiro:"
  },
  "npc.prose.b0bd554507d352e2": {
    "en": "Please select what GT you would like to teleport to.",
    "zh-TW": "請選擇要傳送前往的行會領地。",
    "pt-BR": "Selecione o território de guilda para o qual deseja se teleportar."
  },
  "npc.prose.b0bf34689aee5972": {
    "en": "1. Consignment fee: After confirming the sale,",
    "zh-TW": "1. 寄售費：確認出售後，",
    "pt-BR": "1. Taxa de consignação: após confirmar a venda,"
  },
  "npc.prose.b0f121fc6a091c5f": {
    "en": "I'll buy it at low price.",
    "zh-TW": "我只能低價收購。",
    "pt-BR": "Vou comprar por um preço baixo."
  },
  "npc.prose.b0f9ff634f420639": {
    "en": "Drapery pieces.",
    "zh-TW": "布料衣物。",
    "pt-BR": "Peças de tecido."
  },
  "npc.prose.b12e40b014636061": {
    "en": "I want to  a guild territory.",
    "zh-TW": "我想處理一處行會領地。",
    "pt-BR": "Quero tratar de um território de guilda."
  },
  "npc.prose.b160f97efe72164b": {
    "en": "archer is higher level.",
    "zh-TW": "弓箭手的等級較高時。",
    "pt-BR": "o arqueiro for de nível superior."
  },
  "npc.prose.b196cec2ee46bd47": {
    "en": "Guildwar cannot take place in village.",
    "zh-TW": "行會戰不能在村莊內進行。",
    "pt-BR": "Guerras de guildas não podem ocorrer dentro de vilas."
  },
  "npc.prose.b238b2f03c4dac9b": {
    "en": "even during war.",
    "zh-TW": "即使在戰爭期間也是如此。",
    "pt-BR": "mesmo durante a guerra."
  },
  "npc.prose.b24486f5c8565045": {
    "en": "for them to avoid being in battles with multiple enemies due to their weak vitality",
    "zh-TW": "由於生命力薄弱，他們應避免同時與多名敵人交戰",
    "pt-BR": "que evitem combater vários inimigos ao mesmo tempo, devido à sua baixa vitalidade"
  },
  "npc.prose.b24a6e6368b789e9": {
    "en": "I have nothing to tell you...",
    "zh-TW": "我沒有什麼能告訴你……",
    "pt-BR": "Não tenho nada para contar..."
  },
  "npc.prose.b30e366343e53262": {
    "en": "MP and amulet will be consumed.",
    "zh-TW": "會消耗魔力與護身符。",
    "pt-BR": "Consome PM e talismãs."
  },
  "npc.prose.b347302af99e7b8d": {
    "en": "You don't currently have a Sealed Hero.",
    "zh-TW": "你目前沒有封印的英雄。",
    "pt-BR": "Você não possui um herói selado."
  },
  "npc.prose.b3820b44c3887d43": {
    "en": "The enemy heard I had extorted a treasure and they rushed me, to",
    "zh-TW": "敵人聽說我奪走了寶物，便朝我湧來，想要",
    "pt-BR": "Os inimigos ouviram que eu havia tomado um tesouro e avançaram contra mim para"
  },
  "npc.prose.b3869bfac4623fc1": {
    "en": "You will meet scary monsters and fire dragon.",
    "zh-TW": "你會遇到可怕的怪物與火龍。",
    "pt-BR": "Você encontrará monstros assustadores e um dragão de fogo."
  },
  "npc.prose.b38a47ebf62ea08d": {
    "en": "appear. But thats just hot air. And also an expression of pride and",
    "zh-TW": "出現。但那只是空話，也是驕傲與",
    "pt-BR": "aparece. Mas são apenas palavras vazias e uma expressão de orgulho e"
  },
  "npc.prose.b3f89de6d3ffe039": {
    "en": "Rumours... Rumours... What have people have saying about me now?",
    "zh-TW": "傳聞……傳聞……大家又在說我什麼了？",
    "pt-BR": "Rumores... Rumores... O que estão dizendo de mim agora?"
  },
  "npc.prose.b4a3deb0c1381b3a": {
    "en": "You do not have Rebirth 2.",
    "zh-TW": "你尚未完成第二次轉生。",
    "pt-BR": "Você não possui o segundo renascimento."
  },
  "npc.prose.b4c2c3edc949dddc": {
    "en": "kill with anger The people who got into the RedMoon Valley on that",
    "zh-TW": "憤怒地殺戮。那天進入赤月峽谷的人們",
    "pt-BR": "matar com raiva. As pessoas que entraram no Vale da Lua Vermelha naquele"
  },
  "npc.prose.b4c54a0402b89cff": {
    "en": "Level 48: CelestalShield",
    "zh-TW": "48 級：天盾",
    "pt-BR": "Nível 48: Escudo Celestial"
  },
  "npc.prose.b4e32399f44e9727": {
    "en": "Ah are you one of those villains?",
    "zh-TW": "啊，你也是那些惡徒之一嗎？",
    "pt-BR": "Ah, você é um daqueles vilões?"
  },
  "npc.prose.b4ef949aa92e871e": {
    "en": "What kind of Books are you interested in?",
    "zh-TW": "你對哪類書籍有興趣？",
    "pt-BR": "Em que tipo de livro você está interessado?"
  },
  "npc.prose.b54794d90eb472f6": {
    "en": "Fire an arrow that explodes in a 5×5 radius around the target.",
    "zh-TW": "射出一支箭，在目標周圍 5×5 範圍內爆炸。",
    "pt-BR": "Dispara uma flecha que explode em uma área de 5×5 ao redor do alvo."
  },
  "npc.prose.b56a22317e53b58b": {
    "en": "but the time to get into dungeon is at weekend evening",
    "zh-TW": "但進入地牢的時間是週末晚上",
    "pt-BR": "mas o horário para entrar no calabouço é nas noites de fim de semana"
  },
  "npc.prose.b57a65373f7cf2a1": {
    "en": "monsters. It may take some time but you can save money...",
    "zh-TW": "怪物。或許要花些時間，但可以省錢……",
    "pt-BR": "monstros. Pode levar algum tempo, mas você economiza..."
  },
  "npc.prose.b6572811e4d12356": {
    "en": "There.. Hm.. it is so dangerous that I can not tell you.",
    "zh-TW": "那裡……嗯……太危險了，我不能告訴你。",
    "pt-BR": "Lá... Hmm... É tão perigoso que não posso contar."
  },
  "npc.prose.b6667d7397787ab1": {
    "en": "Do you still want to rent a territory?",
    "zh-TW": "你仍想租用領地嗎？",
    "pt-BR": "Ainda deseja alugar um território?"
  },
  "npc.prose.b74d5bdcaec24d18": {
    "en": "What do you have for trade?",
    "zh-TW": "你有什麼可以交易？",
    "pt-BR": "O que você tem para negociar?"
  },
  "npc.prose.b78d1b1a38bd3901": {
    "en": "This oil doesn't special repair your weapon, so",
    "zh-TW": "這種油無法特修武器，所以",
    "pt-BR": "Este óleo não faz um reparo especial na arma; portanto,"
  },
  "npc.prose.b7c86f8a5b5c23a9": {
    "en": "CHECK TRELLO BOARD.",
    "zh-TW": "查看 Trello 看板。",
    "pt-BR": "Consulte o quadro do Trello."
  },
  "npc.prose.b7ee2802639d7371": {
    "en": "How are you? Mirian {npc_arg0}!",
    "zh-TW": "瑪法人 {npc_arg0}，你好嗎？",
    "pt-BR": "Como vai, habitante de Mir {npc_arg0}?"
  },
  "npc.prose.b7f8ad0b314fbbd6": {
    "en": "Consequently,madness of murder dominated their spirit. People",
    "zh-TW": "於是，殺戮的瘋狂支配了他們的心靈。人們",
    "pt-BR": "Assim, a loucura assassina dominou seus espíritos. As pessoas"
  },
  "npc.prose.b812cedf314c9c21": {
    "en": "MP will be consumed.",
    "zh-TW": "會消耗魔力。",
    "pt-BR": "Consome PM."
  },
  "npc.prose.b81c09ca29b07427": {
    "en": "Welcome traveller,",
    "zh-TW": "歡迎，旅人，",
    "pt-BR": "Boas-vindas, viajante,"
  },
  "npc.prose.b8584576ef98adfd": {
    "en": "It requires Grey Poison.",
    "zh-TW": "需要灰色毒粉。",
    "pt-BR": "Requer pó de veneno cinza."
  },
  "npc.prose.b871dc177b0e2450": {
    "en": "It was burned to the ground thanks to this war!",
    "zh-TW": "它因這場戰爭被燒成了平地！",
    "pt-BR": "Foi completamente queimada por causa desta guerra!"
  },
  "npc.prose.b8c5cde780dc3a30": {
    "en": "(Hmm?! There is a note left in the skeleton pile?)",
    "zh-TW": "（嗯？！骷髏堆裡留著一張紙條？）",
    "pt-BR": "(Hã?! Há um bilhete na pilha de esqueletos?)"
  },
  "npc.prose.b90a9b1262e4cf53": {
    "en": "Concentrate inner force and spreads it to all the parts of your body.",
    "zh-TW": "凝聚內力，並將其散布至全身各處。",
    "pt-BR": "Concentra a energia interior e a distribui por todo o corpo."
  },
  "npc.prose.b94c47ce01720bdb": {
    "en": "the ownership is transferred in 24 hours and the previous owners",
    "zh-TW": "所有權將於 24 小時後轉移，而原擁有者",
    "pt-BR": "a propriedade é transferida em 24 horas, e os antigos proprietários"
  },
  "npc.prose.b9ab3183d04fb2cf": {
    "en": "Please turn back and never come back.",
    "zh-TW": "請回去，永遠別再來。",
    "pt-BR": "Volte e nunca mais retorne."
  },
  "npc.prose.b9ca6b9805cdd251": {
    "en": "What a wierd pillar.",
    "zh-TW": "真是根奇怪的柱子。",
    "pt-BR": "Que pilar estranho."
  },
  "npc.prose.ba4bdce7ee525b5c": {
    "en": "In Memory Of John Schwartz Better known as Iceman",
    "zh-TW": "紀念 John Schwartz，大家更熟悉他的名字 Iceman",
    "pt-BR": "Em memória de John Schwartz, mais conhecido como Iceman"
  },
  "npc.prose.ba7b5cd4c91f98c4": {
    "en": "You will be right here after 3 hours.. Take care of yourself",
    "zh-TW": "3 小時後你會回到這裡……保重",
    "pt-BR": "Você estará de volta aqui em 3 horas... Cuide-se."
  },
  "npc.prose.bac1b6b459573c06": {
    "en": "Before deafeating the monsters,",
    "zh-TW": "在擊敗怪物之前，",
    "pt-BR": "Antes de derrotar os monstros,"
  },
  "npc.prose.bad44cc8b9889411": {
    "en": "Left Wall",
    "zh-TW": "左側城牆",
    "pt-BR": "Muralha esquerda"
  },
  "npc.prose.bb7433b84be7ad1a": {
    "en": "HItting accuracy will be increased in accordance with practice level.",
    "zh-TW": "命中率隨熟練等級提高。",
    "pt-BR": "A precisão dos golpes aumenta com o nível de prática."
  },
  "npc.prose.bb8845035e11f19c": {
    "en": "Main Rebirth cleared.",
    "zh-TW": "已清除主轉生。",
    "pt-BR": "Renascimento principal removido."
  },
  "npc.prose.bb9bee2d2babcd5a": {
    "en": "Below are some Sabuk Statistics.",
    "zh-TW": "以下是沙巴克的部分統計資料。",
    "pt-BR": "Abaixo estão algumas estatísticas de Sabuk."
  },
  "npc.prose.bbe37252ca579524": {
    "en": "about Item.",
    "zh-TW": "關於物品。",
    "pt-BR": "sobre itens."
  },
  "npc.prose.bbef37a378aae5cb": {
    "en": "should I bear in mind?",
    "zh-TW": "我該記住什麼？",
    "pt-BR": "o que devo ter em mente?"
  },
  "npc.prose.bcf5d595e05e677e": {
    "en": "silence around suicide. The money I raise will",
    "zh-TW": "打破對自殺議題的沉默。我募集的款項將",
    "pt-BR": "o silêncio em torno do suicídio. O dinheiro que eu arrecadar vai"
  },
  "npc.prose.bd18ff5dfd492234": {
    "en": "Item  Item.",
    "zh-TW": "物品　物品。",
    "pt-BR": "Itens  Itens."
  },
  "npc.prose.bd224e0e24086ea5": {
    "en": "about Item",
    "zh-TW": "關於物品",
    "pt-BR": "sobre itens"
  },
  "npc.prose.bd534d5257721b73": {
    "en": "Are you crazy? The swamp is very dangerous.",
    "zh-TW": "你瘋了嗎？沼澤非常危險。",
    "pt-BR": "Você enlouqueceu? O pântano é muito perigoso."
  },
  "npc.prose.bd5551beab90558a": {
    "en": "on the dead body for a few seconds.",
    "zh-TW": "對著屍體持續幾秒鐘。",
    "pt-BR": "sobre o corpo por alguns segundos."
  },
  "npc.prose.bdb8a2dfd2c68fd3": {
    "en": "You will meet scary monsters and a firey dragon.",
    "zh-TW": "你會遇到可怕的怪物與烈焰火龍。",
    "pt-BR": "Você encontrará monstros assustadores e um dragão flamejante."
  },
  "npc.prose.bde14f02577fb2d3": {
    "en": "him to stay here. Because he was running away as he had committed",
    "zh-TW": "讓他留在這裡。因為他犯了罪，正在逃亡",
    "pt-BR": "que ficasse aqui, pois estava fugindo depois de cometer"
  },
  "npc.prose.be4a7839d2c3aa24": {
    "en": "Taoist skill page 2:",
    "zh-TW": "道士技能第 2 頁：",
    "pt-BR": "Habilidades de taoísta, página 2:"
  },
  "npc.prose.be540a83dbf3f6d4": {
    "en": "Before the demonic power came there. If a certain special power got into",
    "zh-TW": "在魔力來臨之前。若某種特殊力量進入",
    "pt-BR": "Antes de o poder demoníaco chegar lá. Se um poder especial entrasse em"
  },
  "npc.prose.be63684ff09bb05c": {
    "en": "We've come here to subdue the Omas and already 3 months",
    "zh-TW": "我們來此討伐半獸人，已經過了 3 個月",
    "pt-BR": "Viemos subjugar os Omas e já se passaram 3 meses."
  },
  "npc.prose.be99b75f9fb18995": {
    "en": "When it is cast, it randomly freezes enemies and after they are frozen, their",
    "zh-TW": "施放時會隨機凍結敵人，被凍結後，他們的",
    "pt-BR": "Ao ser conjurada, congela inimigos aleatoriamente. Depois de congelados, sua"
  },
  "npc.prose.beaf3156fb49fce4": {
    "en": "Go to dungeon from Level 11. It is a dark place where Candle",
    "zh-TW": "11 級起可前往地牢。那裡很黑暗，需要蠟燭",
    "pt-BR": "A partir do nível 11, vá ao calabouço. É um lugar escuro onde velas"
  },
  "npc.prose.beb7120fdcddfc38": {
    "en": "You defeated them all already.",
    "zh-TW": "你已經擊敗全部怪物了。",
    "pt-BR": "Você já derrotou todos eles."
  },
  "npc.prose.becb4f659435fb27": {
    "en": "'War with {nnn} guild' will appear in chatting window and",
    "zh-TW": "聊天視窗會顯示「與 {nnn} 行會交戰」，並且",
    "pt-BR": "“Guerra com a guilda {nnn}” aparecerá na janela de conversa e"
  },
  "npc.prose.bf0765d2f3711380": {
    "en": "Here you can Bring your achers back to life!",
    "zh-TW": "你可以在這裡復活弓箭手！",
    "pt-BR": "Aqui você pode ressuscitar seus arqueiros!"
  },
  "npc.prose.bf1cd72ee2b8e6b2": {
    "en": "*CraftLady  *SignPost  *Coordinate  *Guard",
    "zh-TW": "＊工藝師　＊路標　＊座標　＊守衛",
    "pt-BR": "*Artesã  *Placa  *Coordenadas  *Guarda"
  },
  "npc.prose.bf5015b5fedb7a8b": {
    "en": "with a JadeCrystal there. Keep this in mind.",
    "zh-TW": "帶著翡翠水晶前往。請記住這一點。",
    "pt-BR": "com um Cristal de Jade. Lembre-se disso."
  },
  "npc.prose.bf590a685cec94e1": {
    "en": "Check other user info.  to check the equipped item,",
    "zh-TW": "查看其他玩家資訊，檢查其裝備物品，",
    "pt-BR": "Veja as informações de outro jogador para conferir os itens equipados,"
  },
  "npc.prose.bf7fb91f12a71a68": {
    "en": "people across the country in smashing the",
    "zh-TW": "全國各地的人一起打破",
    "pt-BR": "pessoas de todo o país para quebrar"
  },
  "npc.prose.c0410dc06a372b04": {
    "en": "MP and Poison will be consumed.",
    "zh-TW": "會消耗魔力與毒藥。",
    "pt-BR": "Consome PM e veneno."
  },
  "npc.prose.c042e91b257e0a45": {
    "en": "I see you have {npc_arg0} | {npc_arg1} HP and {npc_arg2} | {npc_arg3} MP",
    "zh-TW": "我看見你有 {npc_arg0}／{npc_arg1} 生命，以及 {npc_arg2}／{npc_arg3} 魔力",
    "pt-BR": "Vejo que você tem {npc_arg0}/{npc_arg1} PV e {npc_arg2}/{npc_arg3} PM."
  },
  "npc.prose.c0699476f8547349": {
    "en": "Once you request to conquer Sabuk the fight will",
    "zh-TW": "提出沙巴克攻城申請後，戰鬥將",
    "pt-BR": "Após solicitar a conquista de Sabuk, a batalha"
  },
  "npc.prose.c0a9c6e0cf882980": {
    "en": "Hello there {npc_arg0}. I don't have any work that",
    "zh-TW": "你好，{npc_arg0}。我沒有任何工作可以",
    "pt-BR": "Olá, {npc_arg0}. Não tenho nenhum trabalho que"
  },
  "npc.prose.c0abc3b799625e8b": {
    "en": "you can start to practise this properly at level 7.",
    "zh-TW": "從 7 級起，你就可以正式練習。",
    "pt-BR": "você pode começar a praticar isso adequadamente no nível 7."
  },
  "npc.prose.c0ecea32ab2c98ac": {
    "en": "move to another place on that floor by using the RT.",
    "zh-TW": "使用隨機傳送卷移動到同一層的其他位置。",
    "pt-BR": "vá a outro lugar do mesmo andar usando um pergaminho de teleporte aleatório."
  },
  "npc.prose.c14c70099e8e2a49": {
    "en": "As the skill level reaches max, one shot one kill will be possible.",
    "zh-TW": "技能達到最高等級時，有可能一擊必殺。",
    "pt-BR": "Quando a habilidade atingir o nível máximo, será possível matar com um único disparo."
  },
  "npc.prose.c15d0f8a3e048c93": {
    "en": "If you also want anything to be made, you may as well ask me.",
    "zh-TW": "若你也想製作任何物品，儘管告訴我。",
    "pt-BR": "Se também quiser fabricar algo, é só me pedir."
  },
  "npc.prose.c1add4cc94184310": {
    "en": "Do you have any good idea?",
    "zh-TW": "你有什麼好主意嗎？",
    "pt-BR": "Você tem alguma boa ideia?"
  },
  "npc.prose.c1efdf1d99957c2f": {
    "en": "the Mir world.",
    "zh-TW": "傳奇世界。",
    "pt-BR": "o mundo de Mir."
  },
  "npc.prose.c213929e950faa77": {
    "en": "Pay 10,000 Gold and Board.",
    "zh-TW": "支付 10,000 金幣並登船。",
    "pt-BR": "Pague 10.000 moedas de ouro e embarque."
  },
  "npc.prose.c22d07dfd80d0f22": {
    "en": "You cannot proceed to the next stage.",
    "zh-TW": "你無法前往下一關。",
    "pt-BR": "Você não pode avançar para a próxima etapa."
  },
  "npc.prose.c2825711273e4a7e": {
    "en": "Level 15: GreatFireball",
    "zh-TW": "15 級：大火球",
    "pt-BR": "Nível 15: Grande Bola de Fogo"
  },
  "npc.prose.c28aade31ea9ade4": {
    "en": "A long time ago, the Redmoon valley was a the habitat of the JadeCrystal",
    "zh-TW": "很久以前，赤月峽谷是翡翠水晶的棲息地",
    "pt-BR": "Há muito tempo, o Vale da Lua Vermelha era o habitat do Cristal de Jade."
  },
  "npc.prose.c28fb28d40d23571": {
    "en": "the sword disappeared from here long time ago. So I don't know",
    "zh-TW": "那把劍很久以前就從這裡消失了。所以我不知道",
    "pt-BR": "a espada desapareceu daqui há muito tempo. Por isso, não sei"
  },
  "npc.prose.c2ddd52e0a2c22fc": {
    "en": "You appear to have {npc_arg0} gold coins to spend.",
    "zh-TW": "看來你有 {npc_arg0} 金幣可以花用。",
    "pt-BR": "Parece que você tem {npc_arg0} moedas de ouro para gastar."
  },
  "npc.prose.c35aac566a385aba": {
    "en": "I can allow you through the gate but only if",
    "zh-TW": "我可以讓你通過大門，但前提是",
    "pt-BR": "Posso deixar você passar pelo portão, mas somente se"
  },
  "npc.prose.c37e8003255b7d42": {
    "en": "and scared of it and I thought I should try to kill it anyhow",
    "zh-TW": "我害怕牠，卻認為無論如何都應該試著殺死牠",
    "pt-BR": "e com medo dele, achei que deveria tentar matá-lo de qualquer maneira"
  },
  "npc.prose.c3b4696701f609b9": {
    "en": "How are you. {npc_arg0}. I am the 'Guild Territory Steward'.",
    "zh-TW": "{npc_arg0}，你好。我是行會領地管家。",
    "pt-BR": "Como vai, {npc_arg0}? Sou o administrador do território da guilda."
  },
  "npc.prose.c3d52fb7b0e0d617": {
    "en": "Sorry {npc_arg0}, You are to high level.",
    "zh-TW": "抱歉，{npc_arg0}，你的等級太高了。",
    "pt-BR": "Desculpe, {npc_arg0}. Seu nível é alto demais."
  },
  "npc.prose.c47236b5faf747dc": {
    "en": "Hey {npc_arg0}!!",
    "zh-TW": "嘿，{npc_arg0}！！",
    "pt-BR": "Ei, {npc_arg0}!!"
  },
  "npc.prose.c48a396f0dfb17f8": {
    "en": "Thats not okay. Thats why this October, Im",
    "zh-TW": "這不應該發生。所以今年十月，我將",
    "pt-BR": "Isso não está certo. Por isso, neste outubro, vou"
  },
  "npc.prose.c4b471a659287d16": {
    "en": "Hello {npc_arg0}",
    "zh-TW": "你好，{npc_arg0}",
    "pt-BR": "Olá, {npc_arg0}"
  },
  "npc.prose.c4e8f0811d4c8dcc": {
    "en": "High damage magical attack. Damage is increased per element.",
    "zh-TW": "高傷害魔法攻擊，每個元素都會提高傷害。",
    "pt-BR": "Ataque mágico de alto dano. Cada elemento aumenta o dano."
  },
  "npc.prose.c53a8c9e31141454": {
    "en": "//////////////////////////////////////////////////////////////////// CAVES",
    "zh-TW": "//////////////////////////////////////////////////////////////////// 洞穴",
    "pt-BR": "//////////////////////////////////////////////////////////////////// Cavernas"
  },
  "npc.prose.c5621d34fae37ffe": {
    "en": "I'm aware.",
    "zh-TW": "我知道。",
    "pt-BR": "Estou ciente."
  },
  "npc.prose.c56bc99fe20f6127": {
    "en": "Would you like to repair a weapon?",
    "zh-TW": "你想修理武器嗎？",
    "pt-BR": "Gostaria de reparar uma arma?"
  },
  "npc.prose.c5700a86723f7a40": {
    "en": "*Under Development*",
    "zh-TW": "＊開發中＊",
    "pt-BR": "*Em desenvolvimento*"
  },
  "npc.prose.c59353983cbc2f92": {
    "en": ", and so on forth.",
    "zh-TW": "，等等。",
    "pt-BR": ", e assim por diante."
  },
  "npc.prose.c5a3515c1d1c39cd": {
    "en": "(Is this the scroll?)",
    "zh-TW": "（這就是那張卷軸嗎？）",
    "pt-BR": "(Este é o pergaminho?)"
  },
  "npc.prose.c6239b9f72c83882": {
    "en": "You need to be Lebel 22 for mounts, Your Level: {npc_arg0}",
    "zh-TW": "坐騎需要 22 級，你目前的等級：{npc_arg0}",
    "pt-BR": "É preciso estar no nível 22 para usar montarias. Seu nível: {npc_arg0}"
  },
  "npc.prose.c63b40059b69eb71": {
    "en": "go and ask him about it.",
    "zh-TW": "去向他詢問吧。",
    "pt-BR": "vá perguntar a ele."
  },
  "npc.prose.c660f171b0239b31": {
    "en": "It's a useful skill to make any situation 1 on 1, as you can hide and use a",
    "zh-TW": "這個技能能讓局面變成一對一：先隱身，再使用",
    "pt-BR": "É útil para transformar qualquer situação em um duelo: você pode se ocultar e usar um"
  },
  "npc.prose.c6e9d3c0bc400132": {
    "en": "Glad to see you, how can I help you?",
    "zh-TW": "很高興見到你，有什麼能幫你的？",
    "pt-BR": "É bom ver você. Como posso ajudar?"
  },
  "npc.prose.c76f3894e1fb5ca9": {
    "en": "To teleport back to this map: @MOVE R001 100 100 / Or use @GM",
    "zh-TW": "傳送回此地圖：@MOVE R001 100 100 ／ 或使用 @GM",
    "pt-BR": "Para voltar a este mapa: @MOVE R001 100 100 / Ou use @GM"
  },
  "npc.prose.c79ff21a84064a8c": {
    "en": "Hello, Are you looking for something particular?",
    "zh-TW": "你好，你在找什麼特定的東西嗎？",
    "pt-BR": "Olá, está procurando algo específico?"
  },
  "npc.prose.c7b398e7fb1860c6": {
    "en": "You have {npc_arg0} parcels awaiting collection.",
    "zh-TW": "你有 {npc_arg0} 個包裹待領取。",
    "pt-BR": "Você tem {npc_arg0} encomendas aguardando retirada."
  },
  "npc.prose.c7bd2f13e3de9b68": {
    "en": "Unfortunately, it seems that many people are still suffering from",
    "zh-TW": "不幸的是，似乎仍有許多人飽受",
    "pt-BR": "Infelizmente, parece que muitas pessoas ainda sofrem com"
  },
  "npc.prose.c7c9f302383e73fc": {
    "en": "to enter the swamp.",
    "zh-TW": "進入沼澤。",
    "pt-BR": "entrar no pântano."
  },
  "npc.prose.c853a7440847e4cc": {
    "en": "go home safely.",
    "zh-TW": "平安回家。",
    "pt-BR": "voltar para casa em segurança."
  },
  "npc.prose.c8601e947acaff63": {
    "en": "For example if you were in a dungeon 1st floor, you will",
    "zh-TW": "例如，若你在地牢一層，你會",
    "pt-BR": "Por exemplo, se estiver no primeiro andar de um calabouço, você"
  },
  "npc.prose.c879471a9e379621": {
    "en": "I transport men and goods to other places fast and safe.",
    "zh-TW": "我能快速、安全地運送人員與貨物。",
    "pt-BR": "Transporto pessoas e mercadorias para outros lugares com rapidez e segurança."
  },
  "npc.prose.c91a36d7c5d51b20": {
    "en": "on the other side, enemy names will appear orange.",
    "zh-TW": "另一方面，敵人的名字會顯示為橙色。",
    "pt-BR": "por outro lado, os nomes dos inimigos aparecem em laranja."
  },
  "npc.prose.c9840f11140af37c": {
    "en": "You are now Reborn.",
    "zh-TW": "你已完成轉生。",
    "pt-BR": "Você renasceu."
  },
  "npc.prose.c985aa5c6942e8aa": {
    "en": "Sir Mogu was asking about that language many years ago.",
    "zh-TW": "莫古爵士多年前曾詢問過那種語言。",
    "pt-BR": "Sir Mogu perguntou sobre essa língua muitos anos atrás."
  },
  "npc.prose.c9899277ac489058": {
    "en": "the ownership of the guild territory will be completely removed.",
    "zh-TW": "行會領地的所有權將被完全移除。",
    "pt-BR": "a propriedade do território da guilda será removida por completo."
  },
  "npc.prose.c9abb9eed51eccf3": {
    "en": "potential to the very limit.",
    "zh-TW": "將潛能發揮至極限。",
    "pt-BR": "o potencial ao limite máximo."
  },
  "npc.prose.c9ae1cec3a0a2c62": {
    "en": "If you guess correctly you will be gifted 100,000 Gold",
    "zh-TW": "若猜對，你將獲得 100,000 金幣。",
    "pt-BR": "Se acertar, receberá 100.000 moedas de ouro."
  },
  "npc.prose.ca01be6babf0ed09": {
    "en": "a serious crime before. But he betrayed that trust and stole the",
    "zh-TW": "先前犯下重罪。但他辜負了信任，偷走了",
    "pt-BR": "um crime grave. Mas ele traiu essa confiança e roubou a"
  },
  "npc.prose.ca85e5428c34d520": {
    "en": "and sales of guild territories at once. If you intend to",
    "zh-TW": "同時處理行會領地的出售。若你打算",
    "pt-BR": "e vendas de territórios de guilda de uma só vez. Se pretende"
  },
  "npc.prose.cb6d0ac60cc4c87d": {
    "en": "Center Wall",
    "zh-TW": "中央城牆",
    "pt-BR": "Muralha central"
  },
  "npc.prose.cb89e0362d2b4cf5": {
    "en": "I finally reached the highest floor of the RedMoon Valley, no one had",
    "zh-TW": "我終於登上赤月峽谷最高層，從來沒有人",
    "pt-BR": "Finalmente alcancei o andar mais alto do Vale da Lua Vermelha, onde ninguém havia"
  },
  "npc.prose.cb9eb156daafab63": {
    "en": "Wizard skill page 2:",
    "zh-TW": "法師技能第 2 頁：",
    "pt-BR": "Habilidades de mago, página 2:"
  },
  "npc.prose.cbc1741588ba1c1d": {
    "en": "Anyone can feel suicidal. But every one of us",
    "zh-TW": "任何人都可能有輕生念頭，但我們每個人",
    "pt-BR": "Qualquer pessoa pode ter pensamentos suicidas. Mas cada um de nós"
  },
  "npc.prose.cc1c9e3880944232": {
    "en": "TownTeleport scrolls can't be made now because the ancient",
    "zh-TW": "現在已無法製作回城卷，因為古代的",
    "pt-BR": "Hoje não é possível fabricar pergaminhos de retorno, pois os antigos"
  },
  "npc.prose.cc5aea45a7b02901": {
    "en": "Level 50: MoonBlade",
    "zh-TW": "50 級：月刃",
    "pt-BR": "Nível 50: Lâmina da Lua"
  },
  "npc.prose.cc6f3fa4b83115ff": {
    "en": "Next Conquest Date: {npc_arg0}",
    "zh-TW": "下次攻城日期：{npc_arg0}",
    "pt-BR": "Data da próxima conquista: {npc_arg0}"
  },
  "npc.prose.ccbc8a7c8fe80f27": {
    "en": "Wizard skill page 1:",
    "zh-TW": "法師技能第 1 頁：",
    "pt-BR": "Habilidades de mago, página 1:"
  },
  "npc.prose.cd486eb975267f43": {
    "en": "drop sometimes.",
    "zh-TW": "有時會掉落。",
    "pt-BR": "às vezes deixam cair."
  },
  "npc.prose.cd501e63bc1a9c88": {
    "en": "So you want my help spreading the good deed's of the ?",
    "zh-TW": "所以，你想請我幫忙宣揚善行？",
    "pt-BR": "Então, quer minha ajuda para divulgar as boas ações?"
  },
  "npc.prose.cd86b6ad356277bb": {
    "en": "How about you do something for me first?",
    "zh-TW": "不如先替我做件事？",
    "pt-BR": "Que tal fazer algo por mim primeiro?"
  },
  "npc.prose.ce5fbf9e2972e7c3": {
    "en": "Please let me know when you're ready and i'll take you.",
    "zh-TW": "準備好後告訴我，我會帶你去。",
    "pt-BR": "Avise quando estiver pronto e eu levarei você."
  },
  "npc.prose.cf4df45bec56ad0a": {
    "en": "It's very dangerous around here,",
    "zh-TW": "這附近非常危險，",
    "pt-BR": "É muito perigoso por aqui,"
  },
  "npc.prose.cf5e87a1419886bb": {
    "en": "Flame attack can damage the target distance away.",
    "zh-TW": "火焰攻擊能傷害遠處的目標。",
    "pt-BR": "O ataque de chamas pode causar dano a um alvo distante."
  },
  "npc.prose.cfa182672d4cc646": {
    "en": "remember that the maximum durability of the weapon",
    "zh-TW": "記住，武器的最高耐久度",
    "pt-BR": "lembre-se de que a durabilidade máxima da arma"
  },
  "npc.prose.d0027b0f3795660e": {
    "en": "monsters that will keep appearing all by themself.",
    "zh-TW": "不斷出現的怪物，必須獨力對付。",
    "pt-BR": "monstros que continuarão aparecendo, sem ajuda de ninguém."
  },
  "npc.prose.d03502c43d74a30b": {
    "en": ",",
    "zh-TW": ",",
    "pt-BR": ",",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.d078a5b101538ed9": {
    "en": "Go to Dead Mine at the west of Bichon-Province or InsectCave at the",
    "zh-TW": "前往比奇省西方的死亡礦區，或是位於",
    "pt-BR": "Vá à Mina Morta, a oeste da Província de Bichon, ou à Caverna dos Insetos, em"
  },
  "npc.prose.d090fbfd43beb9c2": {
    "en": "You have already completed this quest!",
    "zh-TW": "你已經完成這個任務了！",
    "pt-BR": "Você já concluiu esta missão!"
  },
  "npc.prose.d097cb1db76799bc": {
    "en": "I shall inform those will .",
    "zh-TW": "我會通知相關的人。",
    "pt-BR": "Vou informar os envolvidos."
  },
  "npc.prose.d0bfc51c06ee95f9": {
    "en": "Please be careful, monsters are strong there.",
    "zh-TW": "請小心，那裡的怪物很強。",
    "pt-BR": "Tome cuidado: os monstros de lá são fortes."
  },
  "npc.prose.d14733ffdaa0f3a8": {
    "en": "taoist of the highest order is able to make a key JadeCrystal. It is",
    "zh-TW": "唯有最高階道士才能製作作為鑰匙的翡翠水晶。它是",
    "pt-BR": "um taoísta da mais alta ordem consegue criar a chave de Cristal de Jade. É"
  },
  "npc.prose.d174ee4f66bba99d": {
    "en": "island, feel free to ask me at any time.",
    "zh-TW": "島嶼的事，隨時都可以問我。",
    "pt-BR": "a ilha, fique à vontade para me perguntar a qualquer momento."
  },
  "npc.prose.d1d845952f036d67": {
    "en": "Unstoppable tense atmosphere during the battle will be given.",
    "zh-TW": "戰鬥中將始終充滿緊張氣氛。",
    "pt-BR": "A tensão será constante durante a batalha."
  },
  "npc.prose.d1eaf847bee201d4": {
    "en": "wrong step. My mind was still filled with arrogance at that time.",
    "zh-TW": "錯誤的一步。當時，我的心仍充滿傲慢。",
    "pt-BR": "um passo em falso. Na época, minha mente ainda estava cheia de arrogância."
  },
  "npc.prose.d2562801bd5488a7": {
    "en": "These are the items still available to purchase back.",
    "zh-TW": "這些物品仍可購回。",
    "pt-BR": "Estes são os itens que ainda podem ser recomprados."
  },
  "npc.prose.d287fa446480b3f3": {
    "en": "so summoned mobs are prohibitied to be used. The fee is 3000 Gold.",
    "zh-TW": "因此禁止使用召喚物。費用為 3,000 金幣。",
    "pt-BR": "por isso é proibido usar criaturas invocadas. A taxa é de 3.000 moedas de ouro."
  },
  "npc.prose.d2b7c4c3ff215b99": {
    "en": "Archer:",
    "zh-TW": "弓箭手：",
    "pt-BR": "Arqueiro:"
  },
  "npc.prose.d31af56a3ae80a2b": {
    "en": "Taoists.",
    "zh-TW": "道士。",
    "pt-BR": "Taoístas."
  },
  "npc.prose.d41959ac4a0f28d7": {
    "en": "StoneHeart Required Level22~43.",
    "zh-TW": "石之心，適合 22～43 級。",
    "pt-BR": "Coração de Pedra: níveis exigidos de 22 a 43."
  },
  "npc.prose.d4506e6280f4e360": {
    "en": "Players Online {npc_arg0} - You Have {npc_arg1} Pk Points",
    "zh-TW": "線上玩家 {npc_arg0}－你的殺人值為 {npc_arg1}",
    "pt-BR": "Jogadores online: {npc_arg0} — Você tem {npc_arg1} pontos de PK."
  },
  "npc.prose.d450fa056fe8ea56": {
    "en": "Rebirth starts at Level 60 and grants you a Rebirth Effect",
    "zh-TW": "60 級起可轉生，並獲得轉生效果",
    "pt-BR": "O renascimento começa no nível 60 e concede um efeito de renascimento."
  },
  "npc.prose.d4664b4ea9ee6005": {
    "en": "This is a temporary base for the explorers.",
    "zh-TW": "這裡是探險者的臨時基地。",
    "pt-BR": "Esta é uma base temporária para exploradores."
  },
  "npc.prose.d47d7cb0e4f8fd2b": {
    "en": "More",
    "zh-TW": "更多",
    "pt-BR": "Mais"
  },
  "npc.prose.d48e6d7a3f2563e0": {
    "en": "damage from range. Much like wizards, they rely on their",
    "zh-TW": "遠距離造成傷害。如同法師，他們依靠自己的",
    "pt-BR": "causar dano à distância. Assim como os magos, dependem de sua"
  },
  "npc.prose.d52d3b50a0fb022f": {
    "en": "Increases magic defensive power.",
    "zh-TW": "提高魔法防禦力。",
    "pt-BR": "Aumenta a defesa mágica."
  },
  "npc.prose.d57494e1d3772c30": {
    "en": "repair with this oil occasionally during hunting.",
    "zh-TW": "狩獵途中可偶爾使用這種油修理。",
    "pt-BR": "repare com este óleo ocasionalmente durante a caçada."
  },
  "npc.prose.d5845d063cfb635b": {
    "en": "Caves:  ,",
    "zh-TW": "洞穴：",
    "pt-BR": "Cavernas:"
  },
  "npc.prose.d59d1c8ce6425c06": {
    "en": "Just pay the fee then I'll escort you to anywhere.",
    "zh-TW": "付清費用後，我就會護送你去任何地方。",
    "pt-BR": "Basta pagar a taxa e eu acompanharei você a qualquer lugar."
  },
  "npc.prose.d5b6b03c3de86be2": {
    "en": "Level 32: TwinDragonBlade",
    "zh-TW": "32 級：雙龍劍",
    "pt-BR": "Nível 32: Lâmina dos Dragões Gêmeos"
  },
  "npc.prose.d5e0b5866fa93384": {
    "en": "Hello Mirian, We need to improve on how we teach our",
    "zh-TW": "你好，瑪法人。我們需要改進教導方式，讓我們的",
    "pt-BR": "Olá, habitante de Mir. Precisamos melhorar a forma de ensinar nossos"
  },
  "npc.prose.d606231f3b4f394a": {
    "en": "Creature Pearls.",
    "zh-TW": "寵物珍珠。",
    "pt-BR": "Pérolas de criaturas."
  },
  "npc.prose.d60b8785cdcd0bad": {
    "en": "Seeking",
    "zh-TW": "尋找",
    "pt-BR": "Procurando"
  },
  "npc.prose.d622e796bd50dbce": {
    "en": "reviving for a few times.",
    "zh-TW": "多次復活。",
    "pt-BR": "ressuscitando algumas vezes."
  },
  "npc.prose.d64cd7c9e6c69f3c": {
    "en": "This will burn all the objects within the area.",
    "zh-TW": "這會燒灼範圍內的所有目標。",
    "pt-BR": "Isso queimará todos os alvos na área."
  },
  "npc.prose.d6ce4ed1461dbf14": {
    "en": "in Prajna field.",
    "zh-TW": "在般若原野。",
    "pt-BR": "nos Campos de Prajna."
  },
  "npc.prose.d73e1e1b27579989": {
    "en": "You must prepare yourself",
    "zh-TW": "你必須做好準備",
    "pt-BR": "Você precisa se preparar."
  },
  "npc.prose.d76656cd1e09466b": {
    "en": "skills. Ask me anytime, I will be glad to guide you.",
    "zh-TW": "技能。隨時問我，我很樂意指導你。",
    "pt-BR": "habilidades. Pergunte quando quiser; terei prazer em orientar você."
  },
  "npc.prose.d77b53cf1f934c82": {
    "en": "Archer Nine                            Archer Twelve",
    "zh-TW": "弓箭手九　　弓箭手十二",
    "pt-BR": "Arqueiro Nove  Arqueiro Doze"
  },
  "npc.prose.a5e7075a435058c8": {
    "en": ", , , ,",
    "zh-TW": ", , , ,",
    "pt-BR": ", , , ,",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.a9253dc8529dd214": {
    "en": "\\",
    "zh-TW": "\\",
    "pt-BR": "\\",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.ab0f60a002de9823": {
    "en": ", ) ,  ,",
    "zh-TW": ", ) ,  ,",
    "pt-BR": ", ) ,  ,",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.aeb88a1d07dc41d6": {
    "en": ",  ,  ,  ,",
    "zh-TW": ",  ,  ,  ,",
    "pt-BR": ",  ,  ,  ,",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.cbe5cfdf7c2118a9": {
    "en": "|",
    "zh-TW": "|",
    "pt-BR": "|",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.cdb4ee2aea69cc6a": {
    "en": ".",
    "zh-TW": ".",
    "pt-BR": ".",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.d400b2362a38c284": {
    "en": "------------------------------------------------------------------------------------------------------",
    "zh-TW": "------------------------------------------------------------------------------------------------------",
    "pt-BR": "------------------------------------------------------------------------------------------------------",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.d7ba6c5421e3e89c": {
    "en": "of Bichon-province. This is the first dungeon in Mir world where CaveBat,",
    "zh-TW": "比奇省。這是傳奇世界的第一個地牢，裡面有洞蝠、",
    "pt-BR": "da Província de Bichon. Este é o primeiro calabouço do mundo de Mir, onde há Morcegos de Caverna,"
  },
  "npc.prose.d7d3a0aab6e71c20": {
    "en": "Archer Eight                           Archer Eleven",
    "zh-TW": "弓箭手八　　弓箭手十一",
    "pt-BR": "Arqueiro Oito  Arqueiro Onze"
  },
  "npc.prose.d7e1552397f549b0": {
    "en": "Sorry you do not have enough gold.",
    "zh-TW": "抱歉，你的金幣不足。",
    "pt-BR": "Desculpe, você não tem ouro suficiente."
  },
  "npc.prose.d812c49378ab50a4": {
    "en": "Throws a boly of lightning forwards from the casters hands.",
    "zh-TW": "從施法者手中向前射出一道閃電。",
    "pt-BR": "Lança um raio para a frente a partir das mãos do conjurador."
  },
  "npc.prose.d83d73971a065807": {
    "en": "This trap will disappear if approach nearby hexagon.",
    "zh-TW": "靠近六角形邊界時，陷阱會消失。",
    "pt-BR": "A armadilha desaparece quando alguém se aproxima do hexágono."
  },
  "npc.prose.d86a8e157cf87fd1": {
    "en": "Possible that the heroes who went into the Redmoon Valley left a sign",
    "zh-TW": "或許進入赤月峽谷的英雄們留下了記號",
    "pt-BR": "Talvez os heróis que entraram no Vale da Lua Vermelha tenham deixado um sinal."
  },
  "npc.prose.d86ef592b6e8a84f": {
    "en": "Where do you want to make repairs?",
    "zh-TW": "你想修理哪個部位？",
    "pt-BR": "O que deseja reparar?"
  },
  "npc.prose.d8c53d3782ae3490": {
    "en": "keen instincts to dodge oncoming attacks as they tend to",
    "zh-TW": "敏銳的直覺來閃避攻擊，因為他們往往",
    "pt-BR": "instintos aguçados para desviar dos ataques, pois tendem a"
  },
  "npc.prose.d8e182964854f72c": {
    "en": "Have you ever heard about the unknown dungeon?",
    "zh-TW": "你聽說過那個未知地牢嗎？",
    "pt-BR": "Você já ouviu falar do calabouço desconhecido?"
  },
  "npc.prose.d8f980759bb59c6b": {
    "en": "their physical prowess and deadly aim allows them to instil",
    "zh-TW": "他們的體能與致命準度，足以讓",
    "pt-BR": "sua capacidade física e sua mira mortal permitem que inspirem"
  },
  "npc.prose.d9dd3a302b1664f2": {
    "en": "Travel to the Tavern and talk to him about it.",
    "zh-TW": "前往酒館，向他詢問此事。",
    "pt-BR": "Vá à taverna e converse com ele sobre isso."
  },
  "npc.prose.d9f209110957367a": {
    "en": "him some Wine to motivate him.",
    "zh-TW": "給他一些酒，讓他提起精神。",
    "pt-BR": "um pouco de vinho para motivá-lo."
  },
  "npc.prose.d9f6327601c5e15a": {
    "en": "I'm sorry you do not have a TimeStonePiece.",
    "zh-TW": "抱歉，你沒有時間石碎片。",
    "pt-BR": "Desculpe, você não tem um Fragmento da Pedra do Tempo."
  },
  "npc.prose.d9fcbc2ea3bd3a2e": {
    "en": "It is wise to know where to hunt depending on your level in",
    "zh-TW": "了解符合自己等級的狩獵地點是明智之舉，在",
    "pt-BR": "É importante saber onde caçar de acordo com seu nível em"
  },
  "npc.prose.da623acbb5966232": {
    "en": "You know. I'm already 90 years old and have lived here",
    "zh-TW": "你知道嗎？我已經 90 歲了，一直住在這裡",
    "pt-BR": "Sabe, já tenho 90 anos e morei aqui"
  },
  "npc.prose.daad6d376e3576c0": {
    "en": "If the HolySword still exists just one place occurs to me...",
    "zh-TW": "若聖劍仍然存在，我只能想到一個地方……",
    "pt-BR": "Se a Espada Sagrada ainda existir, só consigo pensar em um lugar..."
  },
  "npc.prose.db59006cd95dcb60": {
    "en": "Attacks all monsters in a certain area with meteors of fire falling from the sky.",
    "zh-TW": "召喚從天而降的火焰隕石，攻擊區域內所有怪物。",
    "pt-BR": "Ataca todos os monstros em uma área com meteoros de fogo que caem do céu."
  },
  "npc.prose.db6306d5decdfe28": {
    "en": "Here you can set your Rebirth.",
    "zh-TW": "你可以在這裡設定轉生。",
    "pt-BR": "Aqui você pode definir seu renascimento."
  },
  "npc.prose.dc3cd57c9e4b4c31": {
    "en": "If you want to know it quickly.",
    "zh-TW": "若你想盡快知道。",
    "pt-BR": "Se quiser saber rapidamente."
  },
  "npc.prose.dc90458c7d5746b9": {
    "en": "It is wise to know where to hunt with your level in Mir world.",
    "zh-TW": "在傳奇世界，了解符合自己等級的狩獵地點很重要。",
    "pt-BR": "No mundo de Mir, é importante saber onde caçar de acordo com seu nível."
  },
  "npc.prose.dc9559870be1d6ad": {
    "en": "at dead body of those (Alt+left click) and acquire meat after slicing action.",
    "zh-TW": "對著屍體按 Alt＋左鍵，割肉後即可取得肉。",
    "pt-BR": "no corpo com Alt + clique esquerdo para obter carne após o corte."
  },
  "npc.prose.dcb8a2d0889db155": {
    "en": "Will you start the challenge?",
    "zh-TW": "你要開始挑戰嗎？",
    "pt-BR": "Deseja iniciar o desafio?"
  },
  "npc.prose.dcc95184ba2ed0fb": {
    "en": "This skill is ineffective at higher level enemy.",
    "zh-TW": "此技能對更高等級的敵人無效。",
    "pt-BR": "Esta habilidade não funciona contra inimigos de nível superior."
  },
  "npc.prose.dcd0815a3fade41d": {
    "en": "You have no GoldChest for me to Exchange...",
    "zh-TW": "你沒有可供兌換的金幣寶箱……",
    "pt-BR": "Você não tem um baú de ouro para trocar..."
  },
  "npc.prose.dcd7ed34e6080701": {
    "en": "fear into anyone they hit.",
    "zh-TW": "使任何被擊中的人心生恐懼。",
    "pt-BR": "medo em qualquer um que atinjam."
  },
  "npc.prose.dd684c6d7137d7ce": {
    "en": "Reduces mob attacks (Attack Speed, DC, MC, SC)",
    "zh-TW": "降低怪物的攻擊能力（攻速、攻擊、魔法、道術）。",
    "pt-BR": "Reduz a capacidade ofensiva dos monstros (velocidade de ataque, DC, MC e SC)."
  },
  "npc.prose.dda63bbd455cdb9d": {
    "en": "Lay down a row of traps that explode on contact with an enemy.",
    "zh-TW": "放置一排陷阱，敵人碰觸後就會爆炸。",
    "pt-BR": "Coloca uma fileira de armadilhas que explodem ao tocar em um inimigo."
  },
  "npc.prose.ddb9f70c248bd28e": {
    "en": "Buff which enhances Cripple Shot and OneWithNature spells.",
    "zh-TW": "強化致殘射擊與天人合一的增益效果。",
    "pt-BR": "Um efeito que melhora Disparo Debilitante e União com a Natureza."
  },
  "npc.prose.dddc43dcfdf60677": {
    "en": "Summon a toad to fight for you for 15 seconds.",
    "zh-TW": "召喚蟾蜍為你作戰 15 秒。",
    "pt-BR": "Invoca um sapo para lutar por você durante 15 segundos."
  },
  "npc.prose.ddea5047db337e38": {
    "en": "Come back only if u do have one.",
    "zh-TW": "等你真的有了再回來。",
    "pt-BR": "Volte somente quando tiver um."
  },
  "npc.prose.de086f4f11785ae6": {
    "en": "him by doing so.",
    "zh-TW": "藉此幫助他。",
    "pt-BR": "a ele ao fazer isso."
  },
  "npc.prose.de2f1e03f6302a0f": {
    "en": "go over there and look at the statues.",
    "zh-TW": "到那裡看看那些雕像。",
    "pt-BR": "vá até lá e veja as estátuas."
  },
  "npc.prose.de358f01465e7db1": {
    "en": "a spirit of strong power into this shape.",
    "zh-TW": "將強大的靈體注入此形體。",
    "pt-BR": "um espírito poderoso nesta forma."
  },
  "npc.prose.de7b45f1853fdfc3": {
    "en": "125 people die by suicide in the UK every week.",
    "zh-TW": "英國每週有 125 人死於自殺。",
    "pt-BR": "No Reino Unido, 125 pessoas morrem por suicídio a cada semana."
  },
  "npc.prose.ded0bfbc875c7848": {
    "en": "=== Resetting Features ===",
    "zh-TW": "＝＝＝重置功能＝＝＝",
    "pt-BR": "=== Funções de redefinição ==="
  },
  "npc.prose.dee83595acc16580": {
    "en": "Nothing!",
    "zh-TW": "什麼也沒有！",
    "pt-BR": "Nada!"
  },
  "npc.prose.df9b0c4380cba44d": {
    "en": "Farewell traveler.",
    "zh-TW": "再見，旅人。",
    "pt-BR": "Adeus, viajante."
  },
  "npc.prose.dfd26fd5f686189c": {
    "en": "The following are 'cautions' in the commission sale.",
    "zh-TW": "以下是寄售時的注意事項。",
    "pt-BR": "Estes são os cuidados para a venda em consignação."
  },
  "npc.prose.e06a09578a370fba": {
    "en": "But do not forgot my prices are not low...",
    "zh-TW": "但別忘了，我的價格可不便宜……",
    "pt-BR": "Mas não se esqueça: meus preços não são baixos..."
  },
  "npc.prose.e0a3ac343078352d": {
    "en": "managed to escape from it because I held a HolySword, a priceless",
    "zh-TW": "成功逃離，因為我握著一把無價的聖劍",
    "pt-BR": "consegui escapar porque empunhava uma Espada Sagrada, uma inestimável"
  },
  "npc.prose.e0e7ca5f0ee5c7ae": {
    "en": "Would you like to rebirth?",
    "zh-TW": "你想轉生嗎？",
    "pt-BR": "Gostaria de renascer?"
  },
  "npc.prose.e13edd26bdc91d18": {
    "en": "* {npc_arg0} Attacking Guild",
    "zh-TW": "＊進攻行會：{npc_arg0}",
    "pt-BR": "* Guilda atacante: {npc_arg0}"
  },
  "npc.prose.e14ab57857366b46": {
    "en": "I have been sent here to check the living conditions of people.",
    "zh-TW": "我被派來調查人們的生活狀況。",
    "pt-BR": "Fui enviado para verificar as condições de vida das pessoas."
  },
  "npc.prose.e150c5508c821421": {
    "en": "Once you've achieved this level, go to ZumaTemple towrads the East of Mongchon",
    "zh-TW": "達到這個等級後，前往盟重東方的祖瑪寺廟",
    "pt-BR": "Ao atingir esse nível, vá ao Templo de Zuma, a leste de Mongchon."
  },
  "npc.prose.e1b791848eb85752": {
    "en": "You do know how serious that is?",
    "zh-TW": "你知道這件事有多嚴重嗎？",
    "pt-BR": "Você sabe como isso é grave?"
  },
  "npc.prose.e1b7e0106357042c": {
    "en": "You have {npc_arg0} parcels waiting for you.",
    "zh-TW": "你有 {npc_arg0} 個包裹在等你。",
    "pt-BR": "Você tem {npc_arg0} encomendas esperando."
  },
  "npc.prose.e1c3ba6f0833a890": {
    "en": "Sadly on 05/05/2023 at 4:12pm EST he passed away after a hard battle",
    "zh-TW": "令人遺憾的是，他於 2023 年 5 月 5 日美東標準時間下午 4:12，在艱苦抗病後離世",
    "pt-BR": "Infelizmente, em 05/05/2023, às 16h12 EST, ele faleceu após uma dura batalha"
  },
  "npc.prose.e1cc7316c84a7e2b": {
    "en": "Create an image and fight enemy together. Image appears when skill is",
    "zh-TW": "創造幻影一起作戰。啟用技能時，幻影會出現",
    "pt-BR": "Cria uma imagem que luta ao seu lado. A imagem aparece quando a habilidade é"
  },
  "npc.prose.e212f1acdaaf5948": {
    "en": "Me and my Squad was ambushed! becareful Traveler.",
    "zh-TW": "我和我的小隊遭到伏擊！旅人，請小心。",
    "pt-BR": "Minha equipe e eu fomos emboscados! Tome cuidado, viajante."
  },
  "npc.prose.e26eafbf18bc81e0": {
    "en": "Rebirth 3 -",
    "zh-TW": "第三次轉生－",
    "pt-BR": "Terceiro renascimento —"
  },
  "npc.prose.e2759b396600696e": {
    "en": "What would you like to sell?",
    "zh-TW": "你想出售什麼？",
    "pt-BR": "O que deseja vender?"
  },
  "npc.prose.e2865fb1ab5c51cc": {
    "en": "if you kill a member of the enemy guild you will not",
    "zh-TW": "若你殺死敵方行會成員，就不會",
    "pt-BR": "se matar um membro da guilda inimiga, você não"
  },
  "npc.prose.e2ea63c530fb6427": {
    "en": "You need to have the ScrollOfSeal to enter!",
    "zh-TW": "你需要封印卷軸才能進入！",
    "pt-BR": "Você precisa do Pergaminho de Selamento para entrar!"
  },
  "npc.prose.e2eed1b4d37624b7": {
    "en": "Fishing Equipment",
    "zh-TW": "釣魚裝備",
    "pt-BR": "Equipamentos de pesca"
  },
  "npc.prose.e31f3b0d0520b86c": {
    "en": "My advice is simple.",
    "zh-TW": "我的建議很簡單。",
    "pt-BR": "Meu conselho é simples."
  },
  "npc.prose.e37e510f977c2f9e": {
    "en": "Then will you test your power in the Infinite",
    "zh-TW": "那麼，你願意在無限挑戰中測試實力",
    "pt-BR": "Então, vai testar sua força no Desafio Infinito"
  },
  "npc.prose.e3809be88175221b": {
    "en": "Be gone, don't waste my time again!",
    "zh-TW": "走開，別再浪費我的時間！",
    "pt-BR": "Vá embora e não desperdice meu tempo de novo!"
  },
  "npc.prose.e39017b82d22b5aa": {
    "en": "Hello i'm Sigmund, the wandering warrior.",
    "zh-TW": "你好，我是西格蒙德，一名流浪戰士。",
    "pt-BR": "Olá, sou Sigmund, um guerreiro errante."
  },
  "npc.prose.e3d1415380a32f77": {
    "en": "I see you're wearing: {npc_arg0} and have {npc_arg1} PK Points.",
    "zh-TW": "我看見你戴著：{npc_arg0}，殺人值為 {npc_arg1}。",
    "pt-BR": "Vejo que você usa: {npc_arg0} e tem {npc_arg1} pontos de PK."
  },
  "npc.prose.e42368667eb0debd": {
    "en": "Basic control:",
    "zh-TW": "基本操作：",
    "pt-BR": "Controles básicos:"
  },
  "npc.prose.e44eff38786e189e": {
    "en": "Well. This is my duty. This is why i have waited for you instead of",
    "zh-TW": "嗯，這是我的職責。因此，我在這裡等你，而不是",
    "pt-BR": "Bem, esse é meu dever. Por isso esperei você em vez de"
  },
  "npc.prose.e4b1b2907527e92c": {
    "en": "Choose which Item you wish to Disassemble.",
    "zh-TW": "選擇你想分解的物品。",
    "pt-BR": "Escolha o item que deseja desmontar."
  },
  "npc.prose.e4c28b77f538fb9b": {
    "en": "Hello again traveler.. Its a fine day is it not.",
    "zh-TW": "又見面了，旅人……今天天氣真好，不是嗎？",
    "pt-BR": "Olá de novo, viajante... Está um belo dia, não é?"
  },
  "npc.prose.e52fae5a6f9381fe": {
    "en": "What would you like to purchase?",
    "zh-TW": "你想購買什麼？",
    "pt-BR": "O que gostaria de comprar?"
  },
  "npc.prose.e58840a3d1d0af97": {
    "en": "(Fundraising Campaign/http://www.lost-hours.co.uk/fundraising/luanthompson2022?fbclid=IwAR3i4257oBmQtlvLksYhwSS3kJiEn8JjZyZaaEpB29c75DyYDiNeveHou2Q)",
    "zh-TW": "（募款活動/http://www.lost-hours.co.uk/fundraising/luanthompson2022?fbclid=IwAR3i4257oBmQtlvLksYhwSS3kJiEn8JjZyZaaEpB29c75DyYDiNeveHou2Q）",
    "pt-BR": "(Campanha de arrecadação/http://www.lost-hours.co.uk/fundraising/luanthompson2022?fbclid=IwAR3i4257oBmQtlvLksYhwSS3kJiEn8JjZyZaaEpB29c75DyYDiNeveHou2Q)"
  },
  "npc.prose.e58cdede89b59028": {
    "en": "Stones.",
    "zh-TW": "石頭。",
    "pt-BR": "Pedras."
  },
  "npc.prose.e5d45f3467d7dce2": {
    "en": "follow the path North. It will lead to the SnowCavern from",
    "zh-TW": "沿著路往北走，便能抵達雪洞，從",
    "pt-BR": "siga a trilha para o norte. Ela levará à Caverna de Neve a partir de"
  },
  "npc.prose.e61f6c2c3399d60b": {
    "en": "I will be back",
    "zh-TW": "我會回來的",
    "pt-BR": "Eu voltarei."
  },
  "npc.prose.e6585501e14f8b04": {
    "en": "Specialized on undead type monsters.",
    "zh-TW": "專門對付不死系怪物。",
    "pt-BR": "Especializada em monstros mortos-vivos."
  },
  "npc.prose.e6bbf98c6bc3dc17": {
    "en": "These are the basic info. You must know as beginner.",
    "zh-TW": "這些是新手必須了解的基本資訊。",
    "pt-BR": "Estas são as informações básicas que todo iniciante precisa saber."
  },
  "npc.prose.e7064bcea74f5cb6": {
    "en": "Learn all  ,  ,  ,  ,  spells.",
    "zh-TW": "學會所有法術。",
    "pt-BR": "Aprenda todas as magias."
  },
  "npc.prose.e7d0c7754aa10a03": {
    "en": "where the members stay for privacy. Members of the",
    "zh-TW": "成員能享有隱私的地方。行會成員",
    "pt-BR": "onde os membros podem ter privacidade. Os membros da"
  },
  "npc.prose.e7d268f4907b9847": {
    "en": "//////////////////////////////////////////////////////////////////// OC",
    "zh-TW": "//////////////////////////////////////////////////////////////////// 半獸人洞穴",
    "pt-BR": "//////////////////////////////////////////////////////////////////// Caverna dos Omas"
  },
  "npc.prose.e86fad04ba492800": {
    "en": "Show me the Item you wish to Sell.",
    "zh-TW": "讓我看看你想出售的物品。",
    "pt-BR": "Mostre o item que deseja vender."
  },
  "npc.prose.e8c21b03145994df": {
    "en": "The Remains of LostSoul.",
    "zh-TW": "迷失靈魂的遺骸。",
    "pt-BR": "Os restos de uma Alma Perdida."
  },
  "npc.prose.e90ce1502d92d39f": {
    "en": "I can not afford to lose my precious men any more so I need",
    "zh-TW": "我不能再失去珍貴的部下，所以我需要",
    "pt-BR": "Não posso perder mais homens preciosos, por isso preciso"
  },
  "npc.prose.e90eff5fcbd0d46a": {
    "en": "Especially of an outstanding thief 'Abel',who's story still makes",
    "zh-TW": "尤其是那位出色的盜賊亞伯，他的故事至今仍讓人",
    "pt-BR": "Especialmente um ladrão extraordinário chamado Abel, cuja história ainda faz"
  },
  "npc.prose.e919e60ec18e3275": {
    "en": "VisceralWorm, SpittingSpider and etc at this time.",
    "zh-TW": "此時可挑戰內臟蠕蟲、噴毒蜘蛛等。",
    "pt-BR": "Vermes Viscerais, Aranhas Cuspidoras e outros nesta fase."
  },
  "npc.prose.e94d480c8feeaf7e": {
    "en": "So I disappeared from the sight of people to look for stronger monsters",
    "zh-TW": "於是，我離開眾人的視線，尋找更強大的怪物",
    "pt-BR": "Então desapareci da vista das pessoas para procurar monstros mais fortes."
  },
  "npc.prose.e95030f3ea1baa03": {
    "en": "the story",
    "zh-TW": "那個故事",
    "pt-BR": "a história"
  },
  "npc.prose.e958f0203666dd56": {
    "en": "Anything I can help with?",
    "zh-TW": "有什麼我能幫忙的嗎？",
    "pt-BR": "Posso ajudar em algo?"
  },
  "npc.prose.e96ec8356c7de6dd": {
    "en": "*Weapon  *Accessory  *Clothing  *Blacksmith  *Regent  *Peddler",
    "zh-TW": "＊武器　＊飾品　＊服裝　＊鐵匠　＊試劑　＊小販",
    "pt-BR": "*Armas  *Acessórios  *Roupas  *Ferreiro  *Reagentes  *Mascate"
  },
  "npc.prose.ea4292e80f3a751f": {
    "en": "Hero had now passed over to the gods.",
    "zh-TW": "英雄如今已回到眾神身邊。",
    "pt-BR": "O herói agora se juntou aos deuses."
  },
  "npc.prose.ea7aa8c62a51ea61": {
    "en": "guild that has its own territory do not have",
    "zh-TW": "擁有領地的行會不必",
    "pt-BR": "guilda que possui seu próprio território não precisam"
  },
  "npc.prose.eabbb96539157ea3": {
    "en": "You need to be above Level 60 to rebirth.",
    "zh-TW": "你必須超過 60 級才能轉生。",
    "pt-BR": "Você precisa estar acima do nível 60 para renascer."
  },
  "npc.prose.eb4726e900d31718": {
    "en": "You need 10,000,000 Gold to rent a guild territory and the initial",
    "zh-TW": "租用行會領地需要 10,000,000 金幣，最初的",
    "pt-BR": "Você precisa de 10.000.000 de moedas de ouro para alugar um território de guilda, e o período inicial"
  },
  "npc.prose.eb6e5bc9c4b327b5": {
    "en": "If you really want to go back to the past, possess a piece",
    "zh-TW": "若你真想回到過去，請先取得一塊",
    "pt-BR": "Se realmente quiser voltar ao passado, obtenha um fragmento"
  },
  "npc.prose.eb84a5f6a335b839": {
    "en": "reminds me of my youth.",
    "zh-TW": "讓我想起自己的青春。",
    "pt-BR": "me lembra minha juventude."
  },
  "npc.prose.eb8e28d407109eb9": {
    "en": "MineralMine at the first half of Level 20.",
    "zh-TW": "20 級前半段可前往礦坑。",
    "pt-BR": "a Mina de Minérios na primeira metade do nível 20."
  },
  "npc.prose.ebae14e10f86fafe": {
    "en": "Necklace.",
    "zh-TW": "項鍊。",
    "pt-BR": "Colar."
  },
  "npc.prose.ebaea169ad8b798f": {
    "en": "Come here, pal!",
    "zh-TW": "朋友，過來！",
    "pt-BR": "Venha aqui, amigo!"
  },
  "npc.prose.ebaeffe506946f48": {
    "en": "I'm in charge of guild territory rent and sales.",
    "zh-TW": "我負責行會領地的租賃與出售。",
    "pt-BR": "Sou responsável pelo aluguel e pela venda de territórios de guilda."
  },
  "npc.prose.ebbe20604a7b2715": {
    "en": "A pile of skull and bones.",
    "zh-TW": "一堆頭骨與骨骸。",
    "pt-BR": "Uma pilha de crânios e ossos."
  },
  "npc.prose.ec26400a2830114c": {
    "en": "You can repair various kinds of Bracelets and Gloves.",
    "zh-TW": "你可以修理各種手鐲和手套。",
    "pt-BR": "Você pode reparar vários tipos de braceletes e luvas."
  },
  "npc.prose.ec5d2ded6e26b418": {
    "en": "Are you ready?",
    "zh-TW": "你準備好了嗎？",
    "pt-BR": "Está pronto?"
  },
  "npc.prose.ec7ef5c851b10fe6": {
    "en": "quite alot about it.",
    "zh-TW": "對此相當了解。",
    "pt-BR": "bastante sobre isso."
  },
  "npc.prose.ec9bf284756a36cf": {
    "en": "Repair Weapon.",
    "zh-TW": "修理武器。",
    "pt-BR": "Reparar arma."
  },
  "npc.prose.ecd21d1fd2f1702d": {
    "en": "I am the Examiner for Bichon.",
    "zh-TW": "我是比奇的考官。",
    "pt-BR": "Sou o examinador de Bichon."
  },
  "npc.prose.ecec09f1d1e0faa7": {
    "en": "A Randomteleport scroll is a magic paper that can",
    "zh-TW": "隨機傳送卷是一種魔法紙張，可以",
    "pt-BR": "Um pergaminho de teleporte aleatório é um papel mágico que pode"
  },
  "npc.prose.ecf5315dafa80130": {
    "en": "The drinks taster even better after the job is done.",
    "zh-TW": "完成工作後，酒會更好喝。",
    "pt-BR": "A bebida fica ainda melhor depois do trabalho feito."
  },
  "npc.prose.ed2e98f55cf7844b": {
    "en": "First level up and then come and see me.",
    "zh-TW": "先提升等級，再來找我。",
    "pt-BR": "Suba de nível primeiro, depois venha me ver."
  },
  "npc.prose.ed550f7a47e67baa": {
    "en": "You must prepare yourself for he is unforgiving",
    "zh-TW": "你必須做好準備，因為他毫不留情",
    "pt-BR": "Prepare-se, pois ele não perdoa."
  },
  "npc.prose.ed686db5ece481b5": {
    "en": "Welcome to MudWall. Your good deed's around the these Land's.",
    "zh-TW": "歡迎來到土城。你在這片土地上的善行",
    "pt-BR": "Boas-vindas à Muralha de Barro. Suas boas ações por estas terras"
  },
  "npc.prose.edccbdc3b40ff6ec": {
    "en": "You currently have Rebirth 2.",
    "zh-TW": "你目前已完成第二次轉生。",
    "pt-BR": "Você possui atualmente o segundo renascimento."
  },
  "npc.prose.edf21d95cb4bba8a": {
    "en": "*Shout !                                         *Block guild chat",
    "zh-TW": "＊喊話 !　　＊屏蔽行會聊天",
    "pt-BR": "*Gritar !  *Bloquear conversa da guilda"
  },
  "npc.prose.edf26e1c53a9ccd4": {
    "en": "Iceman was a pillar of the LOMCN community since he joined",
    "zh-TW": "Iceman 自加入 LOMCN 以來，一直是社群的支柱",
    "pt-BR": "Iceman foi um pilar da comunidade LOMCN desde que ingressou"
  },
  "npc.prose.edf66c9634f6e74e": {
    "en": "Archer skill page 1:",
    "zh-TW": "弓箭手技能第 1 頁：",
    "pt-BR": "Habilidades de arqueiro, página 1:"
  },
  "npc.prose.ee24b9eea38d8847": {
    "en": "Maybe next",
    "zh-TW": "也許下次吧",
    "pt-BR": "Talvez na próxima."
  },
  "npc.prose.ee67b3225a9587fe": {
    "en": "Here, you can see all the information about ownership, trades,",
    "zh-TW": "在這裡，你可以查看所有權、交易等所有資訊，",
    "pt-BR": "Aqui você pode consultar todas as informações sobre propriedade, transações,"
  },
  "npc.prose.ee91b5ea25f12ff5": {
    "en": "with sepsis.",
    "zh-TW": "與敗血症搏鬥。",
    "pt-BR": "contra a sepse."
  },
  "npc.prose.eeec528bc27d7375": {
    "en": "But a certain villain stole it and ran away with it a long time ago,",
    "zh-TW": "但很久以前，某個惡徒偷走了它並逃跑，",
    "pt-BR": "Mas, há muito tempo, um vilão a roubou e fugiu,"
  },
  "npc.prose.ef16a25a2109443e": {
    "en": "One million Gold & the Horn of WoomaTaurus, who lives",
    "zh-TW": "一百萬金幣，以及居住於",
    "pt-BR": "Um milhão de moedas de ouro e o Chifre do Touro de Wooma, que vive"
  },
  "npc.prose.ef1dd02e6b7d05d6": {
    "en": "This place may give you a difficult time because dead Zombies will keep",
    "zh-TW": "這裡可能讓你吃盡苦頭，因為死去的殭屍會不斷",
    "pt-BR": "Este lugar pode ser difícil, pois os zumbis mortos continuam"
  },
  "npc.prose.ef57bb5ea56ed087": {
    "en": "it's detail either. People say it cayses storms to blow dragons to",
    "zh-TW": "我也不清楚細節。人們說它能引起風暴，讓龍",
    "pt-BR": "os detalhes também. Dizem que provoca tempestades e faz dragões"
  },
  "npc.prose.ef6266e6a9e49743": {
    "en": "Rebirth 2 granted.",
    "zh-TW": "已授予第二次轉生。",
    "pt-BR": "Segundo renascimento concedido."
  },
  "npc.prose.ef64d141e0f320b9": {
    "en": "You are already Rebirth 2.",
    "zh-TW": "你已完成第二次轉生。",
    "pt-BR": "Você já possui o segundo renascimento."
  },
  "npc.prose.ef69dec0086931c4": {
    "en": "My duty is to protect the general at all cost.",
    "zh-TW": "我的職責是不惜一切代價保護將軍。",
    "pt-BR": "Meu dever é proteger o general a qualquer custo."
  },
  "npc.prose.ef8099029d707eec": {
    "en": "by hurling an amulet.",
    "zh-TW": "透過投擲護身符。",
    "pt-BR": "ao lançar um talismã."
  },
  "npc.prose.ef9fa94efa4fd898": {
    "en": "you can money in return.",
    "zh-TW": "你可以換取金錢。",
    "pt-BR": "você pode receber dinheiro em troca."
  },
  "npc.prose.efb52f1808c46326": {
    "en": "You need to be above Level 80 to rebirth.",
    "zh-TW": "你必須超過 80 級才能轉生。",
    "pt-BR": "Você precisa estar acima do nível 80 para renascer."
  },
  "npc.prose.efdf6c3e390871b9": {
    "en": "Very well traveler..",
    "zh-TW": "好的，旅人……",
    "pt-BR": "Muito bem, viajante..."
  },
  "npc.prose.efe825907f524753": {
    "en": "Thankyou traveler you have saved me some time.",
    "zh-TW": "謝謝你，旅人，你替我省了些時間。",
    "pt-BR": "Obrigado, viajante. Você me poupou algum tempo."
  },
  "npc.prose.f01d9614712167a7": {
    "en": "ShellNipper, Keratoid and etc.",
    "zh-TW": "鉗蟲、角蟲等。",
    "pt-BR": "Pinçadores de Carapaça, Ceratóides e outros."
  },
  "npc.prose.f021c8d25e3d4319": {
    "en": "Hero Management page",
    "zh-TW": "英雄管理頁面",
    "pt-BR": "Página de gerenciamento de heróis"
  },
  "npc.prose.f06cf9a709c9a2e4": {
    "en": "So, please think carefully berfore trading the guild territory.",
    "zh-TW": "因此，交易行會領地前請仔細考慮。",
    "pt-BR": "Por isso, pense com cuidado antes de negociar um território de guilda."
  },
  "npc.prose.f072c56a3e9c815b": {
    "en": "( SpittingSpider, CannibalPlant, CaveMaggot )",
    "zh-TW": "（噴毒蜘蛛、食人花、洞蛆）",
    "pt-BR": "(Aranha Cuspidora, Planta Carnívora, Verme de Caverna)"
  },
  "npc.prose.f093ecbb111204f6": {
    "en": "I can't help you... Just wait a little while..",
    "zh-TW": "我幫不了你……再等一下吧……",
    "pt-BR": "Não posso ajudar... Espere mais um pouco..."
  },
  "npc.prose.f0a7aef1a99dc803": {
    "en": "Hello i'm Gilbert, the wandering warrior.",
    "zh-TW": "你好，我是吉爾伯特，一名流浪戰士。",
    "pt-BR": "Olá, sou Gilbert, um guerreiro errante."
  },
  "npc.prose.f0c0faba9f894b50": {
    "en": "Hello {npc_arg0}!",
    "zh-TW": "你好，{npc_arg0}！",
    "pt-BR": "Olá, {npc_arg0}!"
  },
  "npc.prose.f0e710e49b141144": {
    "en": "Allows you to push away mobs and people at 8 o'clock direction.",
    "zh-TW": "將怪物和玩家往八點鐘方向推開。",
    "pt-BR": "Permite empurrar monstros e pessoas na direção das oito horas."
  },
  "npc.prose.f0fcf573ab2fb8b4": {
    "en": "The toad cannot move and will explode if its master",
    "zh-TW": "蟾蜍無法移動；若主人離開，牠就會爆炸",
    "pt-BR": "O sapo não pode se mover e explodirá se seu mestre"
  },
  "npc.prose.f10ef8d33c4d0b7b": {
    "en": "Once you achieved this level, go to ZumaTemple at the East of Mongchon",
    "zh-TW": "達到這個等級後，前往盟重東方的祖瑪寺廟",
    "pt-BR": "Ao atingir esse nível, vá ao Templo de Zuma, a leste de Mongchon."
  },
  "npc.prose.f11d5258816708f7": {
    "en": "my task. I shall tell you everything.",
    "zh-TW": "我的任務。我會告訴你一切。",
    "pt-BR": "minha tarefa. Vou contar tudo."
  },
  "npc.prose.f15fd107ccaefac4": {
    "en": "selling 10 Brown Chestnuts!",
    "zh-TW": "出售 10 顆褐色栗子！",
    "pt-BR": "vendendo 10 castanhas marrons!"
  },
  "npc.prose.f160bd6bf1e486f9": {
    "en": "1) Get skill book of your level. (Beginner of Mid Level skill book is sold at the",
    "zh-TW": "1）取得符合自己等級的技能書。（初階與中階技能書在",
    "pt-BR": "1) Obtenha um livro de habilidade do seu nível. (Livros de nível inicial e intermediário são vendidos na"
  },
  "npc.prose.f17cb1a16d764513": {
    "en": "Who sent you?",
    "zh-TW": "誰派你來的？",
    "pt-BR": "Quem enviou você?"
  },
  "npc.prose.f1b5d3522a4a9d08": {
    "en": "Item's you are still able to purchase back.",
    "zh-TW": "你仍可購回的物品。",
    "pt-BR": "Itens que você ainda pode recomprar."
  },
  "npc.prose.f1bb461a8d62a51b": {
    "en": "needs doing at the moment. Come back next week and I",
    "zh-TW": "目前需要處理。下週再來，我會",
    "pt-BR": "precise ser feito agora. Volte na semana que vem e eu"
  },
  "npc.prose.f1ca2ad9b9960434": {
    "en": ",  ,  (KR)",
    "zh-TW": "， ， （韓國）",
    "pt-BR": ", , (Coreia)"
  },
  "npc.prose.f22e0833adf11c20": {
    "en": "Welcome, what can I do for you?",
    "zh-TW": "歡迎，有什麼能為你效勞？",
    "pt-BR": "Boas-vindas. Como posso ajudar?"
  },
  "npc.prose.f23d354f3122b08c": {
    "en": "Your the Traveler, I keep hearing about.",
    "zh-TW": "你就是我一直聽說的那位旅人。",
    "pt-BR": "Você é o viajante de quem tanto ouço falar."
  },
  "npc.prose.f25baa2bbfcbac12": {
    "en": "Pay 3000 Gold",
    "zh-TW": "支付 3,000 金幣",
    "pt-BR": "Pague 3.000 moedas de ouro."
  },
  "npc.prose.f2b2c6321addbfbe": {
    "en": "If you wish to return to castles or villages, look",
    "zh-TW": "若想返回城堡或村莊，請找",
    "pt-BR": "Se quiser voltar a castelos ou vilas, procure"
  },
  "npc.prose.f2beed23669e8abc": {
    "en": "Did you know material's can make many wonderful items",
    "zh-TW": "你知道材料可以製成許多奇妙的物品嗎",
    "pt-BR": "Sabia que materiais podem ser usados para criar muitos itens maravilhosos?"
  },
  "npc.prose.f2cecfe8e1badaab": {
    "en": "Thank You for your contribution. As a reward you now have access",
    "zh-TW": "感謝你的貢獻。作為獎勵，你現在可以進入",
    "pt-BR": "Obrigado pela contribuição. Como recompensa, agora você tem acesso"
  },
  "npc.prose.f35d09ef908d544a": {
    "en": "Show me the weapon that needs it.",
    "zh-TW": "讓我看看需要處理的武器。",
    "pt-BR": "Mostre a arma que precisa disso."
  },
  "npc.prose.f3803a1bec7dbec8": {
    "en": "What a mysterious pillar.",
    "zh-TW": "真是根神祕的柱子。",
    "pt-BR": "Que pilar misterioso."
  },
  "npc.prose.f3bc24e0ea09461f": {
    "en": "these lands. Do you have anything for sale?",
    "zh-TW": "這些土地。你有什麼要出售的嗎？",
    "pt-BR": "estas terras. Tem algo para vender?"
  },
  "npc.prose.f46c077920871230": {
    "en": "What item do you want to store or withdraw?",
    "zh-TW": "你想存入或取出什麼物品？",
    "pt-BR": "Que item deseja guardar ou retirar?"
  },
  "npc.prose.f4992351f22c46f3": {
    "en": "timetravel back to the past. But if your level is below 33,",
    "zh-TW": "穿越時空回到過去。但若你低於 33 級，",
    "pt-BR": "viajar no tempo para o passado. Mas, se seu nível for inferior a 33,"
  },
  "npc.prose.f4ef32138f2c1692": {
    "en": "Hello Traveler. Have you ventured into the caves?",
    "zh-TW": "你好，旅人。你去過那些洞穴了嗎？",
    "pt-BR": "Olá, viajante. Já se aventurou pelas cavernas?"
  },
  "npc.prose.f4f2fccb67cd1c0e": {
    "en": "to have secret meeting at the hidden corners of villages,",
    "zh-TW": "在村莊隱蔽的角落祕密聚會，",
    "pt-BR": "fazer reuniões secretas nos cantos escondidos das vilas,"
  },
  "npc.prose.f50e4ccde364c3f6": {
    "en": "All the enemies nearby 8 o'clock direction at once.",
    "zh-TW": "同時影響八點鐘方向附近的所有敵人。",
    "pt-BR": "Todos os inimigos próximos na direção das oito horas, de uma só vez."
  },
  "npc.prose.f52e5b6d27340647": {
    "en": "Candles are needed when it's dark. Without a candle",
    "zh-TW": "天黑時需要蠟燭。沒有蠟燭",
    "pt-BR": "Velas são necessárias no escuro. Sem uma vela,"
  },
  "npc.prose.f56bf4572a5fd31f": {
    "en": "using their powerful skills with bows to deal extraordinary",
    "zh-TW": "運用強大的弓術造成驚人的",
    "pt-BR": "usando suas poderosas habilidades com o arco para causar um extraordinário"
  },
  "npc.prose.f5b7f747230acc18": {
    "en": "I am the Captain of a Archer Regerment",
    "zh-TW": "我是弓箭手部隊的隊長",
    "pt-BR": "Sou o capitão de um regimento de arqueiros."
  },
  "npc.prose.f64ccf95319bda79": {
    "en": "I can exchange GoldBars,Bundles and Chests back in Gold.",
    "zh-TW": "我可以將金條、金磚束與金幣寶箱換回金幣。",
    "pt-BR": "Posso trocar barras, pacotes e baús de ouro por moedas de ouro."
  },
  "npc.prose.f66668f9a83cd414": {
    "en": "Level 19: SummonSkeletonton",
    "zh-TW": "19 級：召喚骷髏",
    "pt-BR": "Nível 19: Invocar Esqueleto"
  },
  "npc.prose.f674598bb690ca62": {
    "en": "Current Bounty List:",
    "zh-TW": "目前懸賞清單：",
    "pt-BR": "Lista atual de recompensas:"
  },
  "npc.prose.f67f969a09c1f092": {
    "en": "be going towards CALMs life-saving services,",
    "zh-TW": "將用於 CALM 挽救生命的服務，",
    "pt-BR": "será destinado aos serviços da CALM que salvam vidas,"
  },
  "npc.prose.f68e5d0879d60862": {
    "en": "Taoist:",
    "zh-TW": "道士：",
    "pt-BR": "Taoísta:"
  },
  "npc.prose.f6ca8db1f6876b4f": {
    "en": "the responsibility of your death would fall on me.",
    "zh-TW": "你的死亡責任就會落在我身上。",
    "pt-BR": "a responsabilidade por sua morte recairia sobre mim."
  },
  "npc.prose.f6f2319e6f7dae25": {
    "en": "Hello I'm Jason, the wandering warrior.",
    "zh-TW": "你好，我是傑森，一名流浪戰士。",
    "pt-BR": "Olá, sou Jason, um guerreiro errante."
  },
  "npc.prose.f738850120dae6eb": {
    "en": "multiple attack. But it consumes much MP.",
    "zh-TW": "多重攻擊，但會消耗大量魔力。",
    "pt-BR": "ataque múltiplo, mas consome muitos PM."
  },
  "npc.prose.f7d325e82fbd48d5": {
    "en": "I am the loyal Commander to the Emperor Far.",
    "zh-TW": "我是忠於 Far 皇帝的指揮官。",
    "pt-BR": "Sou o comandante leal ao imperador Far."
  },
  "npc.prose.f7d84c28b5026458": {
    "en": "down my spine.",
    "zh-TW": "沿著我的脊背竄下。",
    "pt-BR": "pela minha espinha."
  },
  "npc.prose.f7e48a90d5d76cfc": {
    "en": "How can I help you?",
    "zh-TW": "有什麼能幫你的？",
    "pt-BR": "Como posso ajudar?"
  },
  "npc.prose.f87276d5fd232e29": {
    "en": "period for rent is 7 days.",
    "zh-TW": "最初租期為 7 天。",
    "pt-BR": "o período de aluguel é de 7 dias."
  },
  "npc.prose.f87973bf3ca625d1": {
    "en": "Hello traveller, how may I help you?",
    "zh-TW": "你好，旅人。有什麼能幫你的？",
    "pt-BR": "Olá, viajante. Como posso ajudar?"
  },
  "npc.prose.f8b39062942e2e3f": {
    "en": "Don't get to close.. I've lost everything..",
    "zh-TW": "別靠太近……我失去了一切……",
    "pt-BR": "Não chegue muito perto... Perdi tudo..."
  },
  "npc.prose.f8c500d9376258ee": {
    "en": "available Eggs to purchase.",
    "zh-TW": "可供購買的蛋。",
    "pt-BR": "ovos disponíveis para compra."
  },
  "npc.prose.f8ffa8314274635e": {
    "en": "\"You once researched a Ancient .\"",
    "zh-TW": "「你曾研究過古代的事物。」",
    "pt-BR": "“Você já pesquisou algo antigo.”"
  },
  "npc.prose.f942167607409da2": {
    "en": "If the weapon at your hand was not expensive,",
    "zh-TW": "若你手上的武器並不昂貴，",
    "pt-BR": "Se a arma que você empunha não era cara,"
  },
  "npc.prose.f96d827bc4b0e7ad": {
    "en": "some monsters when taken such slicing meat action.",
    "zh-TW": "對某些怪物進行割肉動作時可取得。",
    "pt-BR": "de alguns monstros ao realizar a ação de cortar carne."
  },
  "npc.prose.f9929c125e4e1861": {
    "en": "be the most important conversation you ever have.",
    "zh-TW": "成為你一生中最重要的一次談話。",
    "pt-BR": "ser a conversa mais importante que você terá na vida."
  },
  "npc.prose.f9a5a3943449adf8": {
    "en": "By facing the guild master and selecting 'trade',",
    "zh-TW": "面對行會會長，選擇「交易」，",
    "pt-BR": "Ao ficar de frente para o líder da guilda e selecionar “Negociar”,"
  },
  "npc.prose.f9ca2ab178470063": {
    "en": "Please keep in mind that no special characters",
    "zh-TW": "請記住，不允許使用特殊字元",
    "pt-BR": "Lembre-se de que caracteres especiais não"
  },
  "npc.prose.f9d395eb236a859c": {
    "en": "*Private chat /Username  *Group shout !!         *Guild shout !~",
    "zh-TW": "＊私聊 /Username　＊隊伍喊話 !!　＊行會喊話 !~",
    "pt-BR": "*Conversa privada /Username  *Mensagem de grupo !!  *Mensagem de guilda !~"
  },
  "npc.prose.f9ebad2fbd59718d": {
    "en": "Something dosent seem right in this Village.",
    "zh-TW": "這個村莊似乎有些不對勁。",
    "pt-BR": "Algo não parece certo nesta vila."
  },
  "npc.prose.fa67f8e4e0b777a8": {
    "en": "PoisonShot buff will make Cripple shot produce a 3×3",
    "zh-TW": "毒箭增益會讓致殘射擊產生 3×3 的",
    "pt-BR": "O efeito Disparo Venenoso faz o Disparo Debilitante produzir uma área de 3×3"
  },
  "npc.prose.faa2bd97d1e8c131": {
    "en": "the JadeStick then taoistic defenses will come out of it,  It requires a",
    "zh-TW": "玉杖時，就會釋放道術防禦。這需要",
    "pt-BR": "no Bastão de Jade, defesas taoístas surgirão. Isso exige um"
  },
  "npc.prose.faba99ddceee7b5d": {
    "en": "Wow, you defeated them all!!",
    "zh-TW": "哇，你把牠們全都打敗了！！",
    "pt-BR": "Uau, você derrotou todos eles!!"
  },
  "npc.prose.fad664a50d7f6120": {
    "en": "Leap backwards out of danger.",
    "zh-TW": "向後跳躍，脫離危險。",
    "pt-BR": "Salta para trás para escapar do perigo."
  },
  "npc.prose.faf1d94b52544271": {
    "en": "My Father entered the Mines looking for possible Loot!",
    "zh-TW": "我的父親進入礦坑，尋找可能的戰利品！",
    "pt-BR": "Meu pai entrou nas minas à procura de espólio!"
  },
  "npc.prose.fafa79317c613135": {
    "en": "constipation one day...",
    "zh-TW": "有一天會便祕……",
    "pt-BR": "prisão de ventre algum dia..."
  },
  "npc.prose.fb664a62bf039fe1": {
    "en": "the Emperors good deeds.",
    "zh-TW": "皇帝的善行。",
    "pt-BR": "as boas ações do imperador."
  },
  "npc.prose.fb74e25e5aab95c2": {
    "en": "see you then...",
    "zh-TW": "到時再見……",
    "pt-BR": "até lá..."
  },
  "npc.prose.fb9349c90df979da": {
    "en": "Level 1~25.",
    "zh-TW": "1～25 級。",
    "pt-BR": "Níveis 1 a 25."
  },
  "npc.prose.fbb014183a6bf1fe": {
    "en": "Specialized skill on Undead type monsters.",
    "zh-TW": "專門對付不死系怪物的技能。",
    "pt-BR": "Habilidade especializada em monstros mortos-vivos."
  },
  "npc.prose.fbef4a1ca8dd172f": {
    "en": "The vampire pet can leach HP to its master.",
    "zh-TW": "吸血寵物可以吸取生命，回復給主人。",
    "pt-BR": "O mascote vampiro pode drenar PV para seu mestre."
  },
  "npc.prose.fc4fa1e18cdd1af5": {
    "en": "Can you call yourself a true Mirian fighter?",
    "zh-TW": "你能稱自己為真正的瑪法勇士嗎？",
    "pt-BR": "Você pode se considerar um verdadeiro guerreiro de Mir?"
  },
  "npc.prose.fcbda71dcebe622e": {
    "en": "Summon an elemental ring around the archer that deals damage",
    "zh-TW": "在弓箭手周圍召喚元素環，造成傷害",
    "pt-BR": "Invoca um anel elemental ao redor do arqueiro, causando dano"
  },
  "npc.prose.fcc843cda25ac773": {
    "en": "Heal several players at once 3X3 square max. 9 people.",
    "zh-TW": "同時治療 3×3 格內的多名玩家，最多 9 人。",
    "pt-BR": "Cura vários jogadores em uma área de 3×3 casas, até 9 pessoas."
  },
  "npc.prose.fd11e7fb737d1707": {
    "en": "This place may give you a difficult time because killed Zombie will keep",
    "zh-TW": "這裡可能讓你吃盡苦頭，因為被殺死的殭屍會不斷",
    "pt-BR": "Este lugar pode ser difícil, pois os zumbis mortos continuam"
  },
  "npc.prose.fd1a793c2bada7fb": {
    "en": "the transaction is complete. After a successful transaction,",
    "zh-TW": "交易即告完成。交易成功後，",
    "pt-BR": "a transação é concluída. Após uma transação bem-sucedida,"
  },
  "npc.prose.fd5bbb8f8ec0353d": {
    "en": "*Ban private chat                                *Withdraw guild",
    "zh-TW": "＊屏蔽私聊　　＊退出行會",
    "pt-BR": "*Bloquear conversa privada  *Sair da guilda"
  },
  "npc.prose.fddf9507e13efde1": {
    "en": "Caves:  ,  ,",
    "zh-TW": "洞穴：",
    "pt-BR": "Cavernas:"
  },
  "npc.prose.fe0fb259888ce585": {
    "en": "Run to Sabuk wall as fast as you can",
    "zh-TW": "盡快跑向沙巴克城",
    "pt-BR": "Corra para a muralha de Sabuk o mais rápido possível."
  },
  "npc.prose.fe19c884cbe2663c": {
    "en": "AOE poison attack. VampBuff will make Cripple shot",
    "zh-TW": "範圍毒攻擊。吸血增益會使致殘射擊",
    "pt-BR": "Ataque venenoso de área. O efeito Vampiro faz o Disparo Debilitante"
  },
  "npc.prose.fe28162a005021ca": {
    "en": "making yourself familiar with variety skills & combat.",
    "zh-TW": "熟悉各種技能與戰鬥方式。",
    "pt-BR": "familiarizar-se com várias habilidades e com o combate."
  },
  "npc.prose.fe36ec8f36f53f94": {
    "en": "a guild territory?",
    "zh-TW": "一處行會領地？",
    "pt-BR": "um território de guilda?"
  },
  "npc.prose.fe4c977aa473bc22": {
    "en": "The evil within is great. But the Riches are greater.",
    "zh-TW": "裡面的邪惡十分強大，但財富更加驚人。",
    "pt-BR": "O mal lá dentro é grande, mas as riquezas são ainda maiores."
  },
  "npc.prose.fe5cfcd25c2eecd9": {
    "en": "Remember, Warrior skills are all melee type.",
    "zh-TW": "記住，戰士技能全都屬於近戰。",
    "pt-BR": "Lembre-se: todas as habilidades de guerreiro são de combate corpo a corpo."
  },
  "npc.prose.fee4dd64a7b8facc": {
    "en": "raft driving is most recommended.",
    "zh-TW": "最推薦搭乘木筏。",
    "pt-BR": "viajar de jangada é a opção mais recomendada."
  },
  "npc.prose.fee73248c9549ca6": {
    "en": "Welcome {npc_arg0},",
    "zh-TW": "歡迎，{npc_arg0}，",
    "pt-BR": "Boas-vindas, {npc_arg0},"
  },
  "npc.prose.fefd318b5cb007b5": {
    "en": "far away from town.",
    "zh-TW": "遠離城鎮。",
    "pt-BR": "longe da cidade."
  },
  "npc.prose.ff2ddea76bd9ee2a": {
    "en": "After paying the fee of 1,000,000 Gold,",
    "zh-TW": "支付 1,000,000 金幣後，",
    "pt-BR": "Após pagar a taxa de 1.000.000 de moedas de ouro,"
  },
  "npc.prose.ff2fc6afb162983b": {
    "en": "I have heard that they might come randomly from monsters",
    "zh-TW": "我聽說它們可能從怪物身上隨機取得",
    "pt-BR": "Ouvi dizer que podem ser obtidos aleatoriamente de monstros."
  },
  "npc.prose.ff689d225ea7573f": {
    "en": "Summon a Totem that spawns a swarm of snakes that aggro",
    "zh-TW": "召喚圖騰，產生一群蛇並吸引",
    "pt-BR": "Invoca um totem que gera um enxame de cobras que atrai"
  },
  "npc.prose.ff7fdb72f5550adc": {
    "en": "and CastleGi-Ryoong are the main hunting fields.",
    "zh-TW": "與寄隆城是主要狩獵區。",
    "pt-BR": "e o Castelo de Gi-Ryoong são as principais áreas de caça."
  },
  "npc.prose.ffe016832b1b9fc6": {
    "en": "rumours some kind of Ancient Creature lives within.",
    "zh-TW": "傳聞有某種遠古生物居住在裡面。",
    "pt-BR": "Dizem que uma criatura antiga vive lá dentro."
  },
  "npc.prose.fff9fbce0eff0406": {
    "en": "If other has priority, it will take some time to root the item.",
    "zh-TW": "若他人有優先權，就要稍等才能撿取物品。",
    "pt-BR": "Se outra pessoa tiver prioridade, será preciso esperar um pouco para coletar o item."
  },
  "npc.prose.e76d0bc0e98b2ad5": {
    "en": "‎",
    "zh-TW": "‎",
    "pt-BR": "‎",
    "sameValueReason": "Source is a directionality control mark, with no translatable words."
  },
  "npc.prose.f34a570db4e09470": {
    "en": "SET [526] 1",
    "zh-TW": "SET [526] 1",
    "pt-BR": "SET [526] 1",
    "sameValueReason": "Literal authored script command accidentally present in prose; kept exact and never executed by this catalog."
  },
  "npc.prose.f6a16e6570270f61": {
    "en": "|  |",
    "zh-TW": "|  |",
    "pt-BR": "|  |",
    "sameValueReason": "Layout punctuation only; no translatable prose."
  },
  "npc.prose.fd088b833d31eca6": {
    "en": "{npc_arg0}",
    "zh-TW": "{npc_arg0}",
    "pt-BR": "{npc_arg0}",
    "sameValueReason": "Opaque runtime parameter only; do not translate interpolated player data."
  }
};
// END REVIEWED PROSE
const source = JSON.parse(fs.readFileSync(sourcePath, 'utf8')).entries;
const catalogs = ['common', 'content', 'quest'].flatMap(name => JSON.parse(fs.readFileSync(path.join(root, `packages/game-data/data/native-i18n/${name}.json`), 'utf8')).entries);
const known = new Map();
for (const entry of catalogs) for (const alias of [entry.en, ...(entry.aliases ?? [])]) known.set(alias, entry);
function translatedName(value) {
  const entry = known.get(value);
  return entry && entry['zh-TW'] !== value && entry['pt-BR'] !== value ? entry : null;
}
function template(value) {
  let match = value.match(/^Level\s+(\d+): (.+)$/);
  if (match) { const name = translatedName(match[2]); if (name) return { 'zh-TW': `${match[1]} 級：${name['zh-TW']}`, 'pt-BR': `Nível ${match[1]}: ${name['pt-BR']}` }; }
  match = value.match(/^(.+) \(Level (\d+)\)$/);
  if (match) { const name = translatedName(match[1]); if (name) return { 'zh-TW': `${name['zh-TW']}（${match[2]} 級）`, 'pt-BR': `${name['pt-BR']} (nível ${match[2]})` }; }
  const entry = translatedName(value);
  if (entry) return { 'zh-TW': entry['zh-TW'], 'pt-BR': entry['pt-BR'] };
  return null;
}
function parameters(text) { return [...text.matchAll(/\{(npc_arg\d+)\}/g)].map(m => m[1]).sort(); }
const entries = [], missing = [];
let runtimeVariantCount = 0;
for (const entry of source) {
  const copy = authored[entry.key];
  if (copy && copy.en !== entry.en) throw new Error(`Reviewed source changed: ${entry.key}`);
  const value = copy ?? template(entry.en);
  if (!value) { missing.push({ key: entry.key, en: entry.en }); continue; }
  for (const locale of ['zh-TW', 'pt-BR']) {
    if (!value[locale]?.trim()) throw new Error(`Empty ${entry.key}/${locale}`);
    if (JSON.stringify(parameters(entry.en)) !== JSON.stringify(parameters(value[locale]))) throw new Error(`Parameter mismatch ${entry.key}/${locale}`);
    if (value[locale] === entry.en && !copy?.sameValueReason) throw new Error(`Unclassified unchanged prose ${entry.key}/${locale}`);
  }
  entries.push({ key: entry.key, en: entry.en, 'zh-TW': value['zh-TW'], 'pt-BR': value['pt-BR'], aliases: entry.aliases });
  for (const variant of entry.runtimeVariants ?? []) {
    const translated = {};
    const sourceParameters = new Set(parameters(entry.en));
    for (const omitted of variant.omittedParameters) {
      if (!sourceParameters.has(omitted)) throw new Error(`Unknown omitted parameter ${variant.key}/${omitted}`);
    }
    for (const locale of ['zh-TW', 'pt-BR']) {
      let text = value[locale];
      for (const omitted of variant.omittedParameters) text = text.replaceAll(`{${omitted}}`, '');
      translated[locale] = text.trim();
      if (!translated[locale] || JSON.stringify(parameters(variant.en)) !== JSON.stringify(parameters(translated[locale]))) {
        throw new Error(`Invalid runtime-empty translation ${variant.key}/${locale}`);
      }
    }
    entries.push({ key: variant.key, en: variant.en, ...translated, aliases: variant.aliases });
    runtimeVariantCount++;
  }
}
const body = JSON.stringify({ entries }, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (!fs.existsSync(outputPath) || fs.readFileSync(outputPath, 'utf8') !== body) throw new Error('NPC prose catalog is stale');
} else { fs.writeFileSync(outputPath, body); }
console.log(JSON.stringify({ sourceCount: source.length, translated: source.length - missing.length, runtimeVariants: runtimeVariantCount, entries: entries.length, missing: missing.length, complete: missing.length === 0 }));
if (process.argv.includes('--missing')) console.log(JSON.stringify(missing, null, 2));
if (process.argv.includes('--check') && missing.length) process.exitCode = 1;
