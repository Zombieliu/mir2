# Dubhe Node Desktop

面向家庭节点运营者的一键式 Dubhe Node 客户端。桌面界面只连接本机
Supervisor，节点身份和管理令牌保存在操作系统密钥库中。

当前功能：

- 首次启动创建或加载 Ed25519 节点身份；
- 查看本机 Supervisor、Zone Host 和 Session 状态；
- 安全开启或暂停新 Session；
- 展示家庭网络出站 Relay 和生产 Beta 准备状态。

## 本地开发

```bash
npm install
npm run tauri dev
```

仅构建前端：

```bash
npm run build
```

构建当前平台安装包：

```bash
npm run tauri build
```

完整节点需要先安装并运行 `home_agent_supervisor`。桌面前端不会接触
Supervisor Bearer Token；所有管理请求都由 Rust/Tauri 后端从系统密钥库
加载令牌后发往 `127.0.0.1`。
