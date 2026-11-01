# UniClipboard DNS 来源修复

基线：n0-computer/iroh `43e232fbcbf11aa9cf03a78ac83f52296f01d6b3`。保留 Engine ADR-034 已审查的路径队列去重、IPv6 relay URL 与 endpoint header 修复。

将工作区直接 simple-dns 依赖固定为 UniClipboard/simple-dns `efdb13b9473131f7172b790b4e8497825aff2890`，将 iroh-dns 的 n0-dns-resolver 固定为 UniClipboard/n0-dns-resolver `b15fdb6abd50714fd551b4fd93a9b5728457442d`。解析器 fork 只传递相同 simple-dns 修复，不改变解析设置与重试策略。

## 网络栈依赖闭包

本仓集中维护 Engine 所用网络栈；内部真实路径依赖随 Engine 的 Git submodule 传递，宿主不需要 crates.io 根补丁。

新增源码快照保留原许可证、测试与来源说明，仅修改依赖清单：
- iroh-blobs 0.103.0：UniClipboard/iroh-blobs `152c42a9217f1377a998b271e0052311e8ada789`，保留其 UNICLIPBOARD_PATCH.md 的四项修复。
- iroh-tickets 1.0.0：crates.io 发布源码，归档 SHA-256 `da53233419ca36bf521ed45683b7748366f9b233032891eefc2d70567a84ac54`。
- iroh-util 0.6.0：crates.io 发布源码，归档 SHA-256 `20e41eb982f15230c55f0a70a74a514360e1f565b07861924fd0e8db172b3d00`。

退出条件：上游发布包含路径队列、EDNS 与 blobs 修复的完整兼容依赖链；整体升级并验证无根补丁下游消费者后移除对应维护补丁。

Engine 的路径依赖格式检查覆盖 blobs，因此同步格式化 Downloader 初始化表达式；无行为或 iroh 调用点变化。

## Peer-datagram egress hook (PR #343 follow-up)

`EndpointHooks::allow_transport_send` applies a synchronous, default-allow destination policy at `TransportsSender::poll_send`. The hook sees real transport addresses after mapping. The same sender carries the hook through initial candidate fan-out and authenticated QUIC path traffic. Rejection silently drops the datagram, as an unavailable transport does, without terminating the QUIC endpoint. Existing hook implementations keep their default behavior. Shared hook ownership uses `Arc`; hooks must not retain an endpoint.

The hook does not classify IP addresses. Engine's adapter owns the LAN-only classification and current-subnet reads. The hook does not filter discovery, relay establishment, or other control traffic. Remove this patch after an upstream transport-policy hook covers these same send paths and the Engine E2E policy matrix passes with that hook.

## Local network-change invalidation (TS-01)

`EndpointHooks::on_network_change` is a default-no-op synchronous notification. The existing socket actor invokes it for both major and minor network changes before transport rebinding and QUIC notification. Engine uses it only to invalidate a shared subnet snapshot and notify its owned refresh worker. The callback performs no interface enumeration or I/O and retains no Endpoint. Existing netmon equality includes interface prefixes, so the notification does not depend on a changed advertised EndpointAddr. Default P2P behavior and the final-send hook remain unchanged.


## PR343 local network recovery

The socket actor refreshes native direct candidates after both major and minor interface changes, after rebinding and before QUIC notification. This avoids suppressing a second LAN-only change when the external report watcher receives an equal value.

The remote actor re-runs the existing path selector after retiring one connection while others remain. Mixed QUIC Initial sends retain the selected remote first and also use known alternative candidates. Existing send-task limits, concurrent fanout, timeouts and final transport policy still apply. Established packet selection is unchanged.

Iroh uses the netwatch0.19.3 source in `netwatch/` through a path dependency. Its maintenance note records exact upstream provenance, licenses and the complete-state/Linux-prefix notification correction. No root Cargo patch table or additional network monitor is required.

## Return-path permission before QUIC receive admission

`Socket::process_datagrams` also applies the existing transport permission predicate to the real remote source before mapping and Noq consumption. A path whose response is forbidden must not allocate pending QUIC handshake state before an allowed alternative copy of the same Initial arrives. Noq indexes these Initial copies by destination CID and does not permit a server to migrate that handshake before confirmation. Rejected receive slots have zero length; adjacent slots and GRO metadata keep their existing ownership.

This uses the current egress classification, not identity or reachability inference. The final send check remains authoritative. The hook must be a repeatable, side-effect-free permission predicate because both receive and send paths invoke it. Its default remains allow. This does not evict existing connections, relax QUIC authentication, classify discovery/control traffic, add retry work or provide an instantaneous packet barrier. Remove this correction with an equivalent upstream policy boundary only after the asymmetric subnet-revocation E2E and default-P2P/transport regressions pass.
