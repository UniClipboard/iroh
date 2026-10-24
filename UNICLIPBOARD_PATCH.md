# UniClipboard DNS 来源修复

基线：n0-computer/iroh `43e232fbcbf11aa9cf03a78ac83f52296f01d6b3`。保留 Engine ADR-034 已审查的路径队列去重、IPv6 relay URL 与 endpoint header 修复；不修改 Rust 源码和网络日志调用点。

将工作区直接 simple-dns 依赖固定为 UniClipboard/simple-dns `efdb13b9473131f7172b790b4e8497825aff2890`，将 iroh-dns 的 n0-dns-resolver 固定为 UniClipboard/n0-dns-resolver `b15fdb6abd50714fd551b4fd93a9b5728457442d`。解析器 fork 只传递相同 simple-dns 修复，不改变解析设置与重试策略。

## 网络栈依赖闭包

本仓集中维护 Engine 所用网络栈；内部真实路径依赖随 Engine 的 Git submodule 传递，宿主不需要 crates.io 根补丁。

新增源码快照保留原许可证、测试与来源说明，仅修改依赖清单：
- iroh-blobs 0.103.0：UniClipboard/iroh-blobs `152c42a9217f1377a998b271e0052311e8ada789`，保留其 UNICLIPBOARD_PATCH.md 的四项修复。
- iroh-tickets 1.0.0：crates.io 发布源码。
- iroh-util 0.6.0：crates.io 发布源码。

退出条件：上游发布包含路径队列、EDNS 与 blobs 修复的完整兼容依赖链；整体升级并验证无根补丁下游消费者后移除对应维护补丁。
