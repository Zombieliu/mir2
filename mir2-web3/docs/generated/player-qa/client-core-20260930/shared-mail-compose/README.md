# Shared Mail compose — M14a source and focused checks

2026-10-03。最新用户要求：继续代码，暂不操作界面。

M14a 已接受源码及命令行回归；整套 M14 还未完成。Native reducer/实际 Submit 与 Web Page 通过独立的小型 WASM 包装层调用同一 dependency-free Rust 写信规则。正文按500 UTF16单位整段校验，CRLF/CR转LF并过滤控制字符，超限保留草稿，发送统一trim且保留gold及有序真实UID语义。

Web 现在由 Page 保存完整草稿，发送前先占用单一flight，socket最后一刻校验owner/presentation/body凭据；明确未送可以继续编辑，进入socket后结果未知保持等待。MailSent失败保留草稿，成功才清理/关闭；同连接换角色保留旧ACK响应槽，其迟到回执不能关闭新owner的写信窗。

Root独立核验Source04共462文件及当前归档：460个既存文件中18变更、442匹配，新增2模块。Source03完整461行仍逐字相同；Source04仅新增根Cargo.toml的workspace排除项。最后独立静态审查无已确认P0/P1/P2。

实际执行通过：

- Node Mail36/36，含实际AST提取mailResult/两wrapper、Page发送/socket/ACK和组件submit/save，不是只查字符串。
- Rust core2、strict platform edge2、explicit ignored producer-consumer replay1、reducer2、Native97、portable26；合计130次通过执行，不宣称130个唯一测试。
- Node生成9条真实包装调用记录（send7/message2），实际inputJson/outputJson与DTO逐条一致，显式编译后Rust消费全部9条语义输出通过。默认ignored没有计入通过。
- Native check和非incremental TypeScript均实际exit0；九个成功job的before/after1091、1093或1094个guard全匹配。Root实际观察20个已知子进程/volume probe PID均已退出。

Source03 Node/core/strict-host/explicit replay结果在Source04沿用：这些461个源码及其消费者保持精确相同，未重复生成producer。原Source03 reducer实际exit101且0测试执行（multiple workspace roots）保留；Root只将apps/game-client/client-core加进根workspace exclude，保留其独立workspace、ui-core成员、其余依赖/features/locks，然后Source04 reducer实际2/2通过。Root换行替换断言和缺失config预检失败也保留，均未执行Cargo。

本批没有GUI、浏览器、设备或真实客户端操作，也没有新EXE、小WASM、主3WASM、Next或thin-package生产构建验收。M13构建包仍是上一批证据。为减少反复全包构建，M14b/c源码与focused检查完成后统一执行一次生产流水线，保持shared raw32505856 B、package377487360 B等既定预算，不能沿用旧余量推断本批通过。

接下来：M14b真实carried UID附件/锁、stamp/server MailCost单请求及迟到报价隔离；M14c共同compose painter和portable text/IME adapter。界面、真实登录/战斗/ACK、保存重登、公共资源、移动设备、Candidate及完整goal都保持OPEN，不报告完成百分比。

证据：[Root source acceptance](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-compose-01/root-source-acceptance01.json)（12010 B，SHA256604b6552026e96e37dfd05671350dddc7b0135d914ec92e9a1e7451facdbf306）；[Source04](C:/mir2-cross-platform-storage-20261002/repo-qa-shared-mail-compose-01/root-full-candidate04.json)（236149 B，ca457dfc4057f84bf2cf655099d83dcb395fca5439540508afd875c7ebaeee32）；Source03/REPORT03/原失败及实际job记录均wx保留。
