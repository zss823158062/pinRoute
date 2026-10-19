# Pinroute 📌

> 让指定的 Windows 程序单独走代理（零侵入、不注入 DLL、不修改目标文件）。

Pinroute 是一款基于 Tauri (Rust + Vue 3) 与 WinDivert 网络驱动技术的开源 Windows 工具。用户只需选择指定的 exe 程序，目标程序的所有 TCP 连接将自动透明重定向到配置的上游代理线路（SOCKS5/HTTP），而操作系统和其它程序的网络流量完全保持原样直连，互不干扰。

---

## 🌟 核心特性 (v0.1 MVP)

- **零侵入目标程序**：无需向目标进程注入 DLL 或 Hook API，通过系统底层网络包透明重定向实现。
- **高精度进程匹配**：
  - **F-01**：原生文件选择器挑选 `.exe` 快速添加。
  - **F-03**：支持精确路径、目录前缀、通配符（`*` / `?`）匹配模式。
  - **F-05**：支持派生子进程匹配（进程树追踪），完美适配多进程架构程序（如 Chromium、Electron、VS Code、Claude Desktop 等）。
  - **F-06**：直接从当前正在运行的进程列表中选择添加。
- **SOCKS5 上游代理**：
  - **F-10 / F-11**：支持填写主机、端口及用户名/密码认证。
  - **F-13**：一键"测试连接"，直观反馈网络延迟与连通状态。
- **防 DNS 污染与泄露 (F-24)**：
  - 内置首包 TLS SNI 嗅探解析器。当目标连接发起 TLS 握手时，自动提取真实域名交由上游代理在远端进行 DNS 解析，彻底杜绝本地 DNS 污染。
- **QUIC / UDP 443 自动回退 (F-27)**：
  - 针对命中程序的 UDP 443 流量执行丢弃策略，促使 Chromium/Electron 核心应用快速回退至稳定的 TCP 代理通道。
- **安全环路保护 (F-22)**：
  - 自身进程 PID 与直连上游代理节点的流量自动排除，避免出现网络重定向环路。
- **实时连接日志 (F-40)**：
  - 实时滚动展示命中进程、目标地址、解析域名、连接状态与协议类型。
- **权限与配置持久化 (F-31 / F-32)**：
  - 自动检测 Windows 管理员权限并提供一键提权重启。
  - 本地 JSON 格式持久化存储，重启后规则与代理节点配置自动保留。

---

## 🏗️ 架构概览

```
[前端界面 Vue 3 + TailwindCSS]
       │  Tauri IPC (invoke / event)
[Rust 后端系统]
  ├── 配置模块 (AppConfig - JSON 持久化)
  ├── 进程管理 (ProcessManager - 进程枚举 / 进程树追踪 / 规则匹配)
  ├── 拦截层 (WinDivertInterceptor - 双向 NAT / UDP 443 丢弃 / 环路排除)
  ├── 本地中继 (RelayServer - tokio TCP Listener)
  ├── 域名嗅探 (TLS ClientHello SNI 解析器)
  └── 代理客户端 (Socks5Client - 远程解析 / 认证 / 双向透明中继)
```

---

## 🚀 快速开始与构建

### 1. 环境准备
- 操作系统：Windows 10 (21H2+) / Windows 11 (x64)
- [Rust](https://rustup.rs/) (>= 1.70)
- [Node.js](https://nodejs.org/) (>= 18) 与 [pnpm](https://pnpm.io/)
- 驱动支持：项目中已内置签名版 `WinDivert.dll` 与 `WinDivert64.sys`

### 2. 安装前端依赖
```powershell
pnpm install
```

### 3. 开发模式运行
> 注意：WinDivert 网络驱动需要以**管理员权限**运行终端。
```powershell
pnpm tauri dev
```

### 4. 生产构建打包
```powershell
pnpm build
cd src-tauri
cargo build --release
```
编译生成的二进制执行程序及驱动文件将输出在 `src-tauri/target/release/` 目录下。

---

## 📄 开源许可证

本项目基于 [MIT 许可证](LICENSE) 开源发布。
网络驱动依赖包含 [WinDivert](https://reqrypt.org/windivert.html)（以动态链接库形式分发，遵循 LGPLv3/GPLv2 许可）。

## 社区链接
[Linux Do](https://linux.do)
