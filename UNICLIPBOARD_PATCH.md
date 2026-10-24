# UniClipboard DNS 来源修复

基线：n0-computer/iroh `43e232fbcbf11aa9cf03a78ac83f52296f01d6b3`。保留 Engine ADR-034 已审查的路径队列去重、IPv6 relay URL 与 endpoint header 修复；不修改 Rust 源码和网络日志调用点。

将工作区直接 simple-dns 依赖固定为 UniClipboard/simple-dns `efdb13b9473131f7172b790b4e8497825aff2890`，将 iroh-dns 的 n0-dns-resolver 固定为 UniClipboard/n0-dns-resolver `b15fdb6abd50714fd551b4fd93a9b5728457442d`。解析器 fork 只传递相同 simple-dns 修复，不改变解析设置与重试策略。

退出条件：上游发布同时包含 ADR-034 基线和 RFC 6891 OPT TTL 修复的完整依赖链。Engine 与宿主继续按 ADR-034 同步四个 iroh 根补丁，不能只更新 Engine 的锁文件或另加宿主 DNS 补丁。
