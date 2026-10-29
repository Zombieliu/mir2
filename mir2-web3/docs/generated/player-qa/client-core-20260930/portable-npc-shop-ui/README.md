# Progress65 · portable/runtime NPC shop shared host

用户要求继续代码、暂不操作界面，本批严格限源码、纯数据/ECS CPU布局测试与编译检查。整体goal保持active，本批不代表完整商店、Web接线、实际JS回调、生产组合或玩家体验验收。

## 实现

- portable与Native使用同一Rust普通Gold/unlimited/panel0商店controller/planner/painter；混合特殊货币、有限库存或resale目录整体走legacy。
- 选择、数量、分页、Buy、Close及Sell交接携带完整identity、source/control proof和精确指针gesture。输入先排队，不修改共同状态；Runtime Last同步sink前后再次核验ingress状态、sink generation及精确pending，接受后才应用共同reducer。过期、拒绝、抛错及同步重入不能修改旧/新状态。
- Core authority和control revision为规范非零decimal JSON String，保留完整u64；renderer仅携带既有Core只读反馈，无新producer/token/slot/receipt/ACK。Entered/Flushed/Unknown仍为本地传输状态，未冒充购买成功。
- 原始完整shop/inventory输入先由既有公共购买planner校验，缺字段不能靠serde默认值补成authority。原始堆叠/来源image index决定icon加载路径。
- 独立NPC能力对象及7个ABI出口；已有Quest/HUD/Character/Spells/Storage精确能力对象未扩展。实际字体/skins/icons、测量控件树、clip、完整584×334范围以及既有shared canvas就绪后才能发布ready。仅新NPC根关闭物理pixel rounding，原严格布局公差不变。较窄触屏整体legacy，移动竖屏适配仍open。

## 实际有限检查

| Source02命令过滤/配置 | 实际结果 |
| --- | --- |
| client-bevy portable-quest-ui / portable_npc_shop_ui | 19 passed, 0 failed/ignored |
| runtime web-quest-ui / npc_shop_ui_host | 17 passed, 0 failed/ignored |
| runtime web-quest-ui / runtime_ui_capabilities | 6 passed, 0 failed/ignored |
| client-bevy native-ui / npc_shop_ui | 20 passed, 0 failed/ignored |
| runtime wasm32-unknown-unknown / webgl2-shared-ui | cargo check exit0 |

共62次跨配置测试执行，不是62个新增独立场景；本批19+17=36项新增测试。CPU绘制检查加载仓库内真实字体并调用Bevy text/UI layout，用虚拟Camera target metadata验证desktop1、touch1.28和DPR2及完整notice；没有真实窗口、renderer或设备。另有受控ComputedNode指针夹具，不能当作真实布局证据。

Source01首轮实际编译失败：测试107–108两次Option::unwrap取走命令，零测试执行。M13按Root单文件合同改为两处as_ref().unwrap()（+18B），完整逆向等于原before；全部其他字节/19项测试/断言保留。Source01 snapshot、result和stderr及Guard before/after均不可替换为通过结果。随后上述五组均在Source02实际重跑。

每次Cargo固定1.95.0、locked/offline/jobs1、incremental0、RUST_MIN_STACK64MiB、test-threads1，串行运行；复用已有有限runner，仅此前两段字面参数/临时路径改动，父Node timeout0。六次（含旧失败）实际C盘≥50GiB守卫，PolicyB完成completed/exited/disposed及outer exit/close，最终五组最低实际C181298548736B；18个已知自有Guard/Cargo/probe PID实际查询均absent，未查询受保护旧客户端。

## 来源与保护

- 最终组合548（547源码/配置+1公共字体）、Rust812（811+同一字体），union974；原Progress64输入按7路径合同增加4文件及改3既有文件，541组合/805Rust保护输入不变。Source02相对Source01仅上述测试文件变，973唯一保护输入不变。
- 三个既有文件（含C2库注册）的完整before与精确逆向保留；最终图在全部执行后实际重哈希0 mismatch。M14对产品、36测试、两轮输入图、有限runner与两轮配置准入独立只读复核blocker0。
- 生产b17 Core manifest/JS/WASM实际重哈希未变；旧Windows/renderer/Next/独立包不能代表61–65新商店代码已构建或运行。无production builder publish。
- 原始证据在 C:/mir2-cross-platform-storage-20261002/repo-qa-portable-npc-shop-ui-01：root-source-snapshot02.json、rust-snapshot02.json、root-test-repair-audit02.json、root-issued-configs02.json、各02.result.json/完整stdout/stderr及cargo-receipts、root-finite-verification02.json与docs full before/after。

## 下一项与未关闭范围

先接Web host/hook、真实Page与Shell唯一控件树、指针/键盘/触屏/游戏手柄输入隔离及现有持久Core dispatcher。Host准备期间保留整体legacy，arming双边输入禁止，只有实际ready才切换；同Core channel继续执行现有prepare、最后一次UI proof/Core entry及exact socket.send。

完整Sell/Repair/BUYBACK/USED、无专用购买ACK恢复、服务端容量/元数据插入一致性、移动竖屏/触屏真机、登录战斗保存重登和最终frontend/Candidate仍open。实际JS sink、窗口、浏览器、WebAssembly Instance、HTTP/server/network探测、账号/存档与剪贴板均未操作。
