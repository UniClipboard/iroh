use std::{pin::Pin, sync::Arc};

use iroh_base::{EndpointAddr, TransportAddr};

use crate::endpoint::{connection::Connection, quic::VarInt};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Outcome of [`EndpointHooks::before_connect`]
#[derive(Debug)]
pub enum BeforeConnectOutcome {
    /// Accept the connect attempt.
    Accept,
    /// Reject the connect attempt.
    Reject,
}

/// Outcome of [`EndpointHooks::after_handshake`]
#[derive(Debug)]
pub enum AfterHandshakeOutcome {
    /// Accept the connection.
    Accept,
    /// Reject and close the connection.
    ///
    /// See [`Connection::close`] for details on `error_code` and `reason`.
    ///
    /// [`Connection::close`]: crate::endpoint::Connection::close
    Reject {
        /// Error code to send with the connection close frame.
        error_code: VarInt,
        /// Close reason to send with the connection close frame.
        reason: Vec<u8>,
    },
}

impl AfterHandshakeOutcome {
    /// Returns [`Self::Accept`].
    pub fn accept() -> Self {
        Self::Accept
    }

    /// Returns [`Self::Reject`].
    pub fn reject(&self, error_code: VarInt, reason: &[u8]) -> Self {
        Self::Reject {
            error_code,
            reason: reason.to_vec(),
        }
    }
}

/// EndpointHooks intercept the connection establishment process of an [`Endpoint`].
///
/// Use [`Builder::hooks`] to install hooks onto an endpoint.
///
/// For each hook, all installed hooks are invoked in the order they were installed on
/// the endpoint builder. If a hook returns `Accept`, processing continues with the next
/// hook. If a hook returns `Reject`, processing is aborted and further hooks
/// are not invoked for this hook point.
///
/// ## Notes to implementers
///
/// As hooks are stored on the endpoint, you must make sure to never store an [`Endpoint`]
/// on the hook struct itself, as this would create reference counting loop and cause the
/// endpoint to never be dropped, leaking memory.
///
/// [`Endpoint`]: crate::Endpoint
/// [`Builder::hooks`]: crate::endpoint::Builder::hooks
pub trait EndpointHooks: std::fmt::Debug + Send + Sync {
    /// 在传输重新绑定和 QUIC 通知之前观察本机网络变化。
    ///
    /// 现有网络监视器报告的重大变化和次要变化均触发此回调，包括接口前缀变化。
    /// 此回调不在发送轮询中运行。
    /// 实现必须快速返回，并将所有 I/O 安排在此回调之外。
    fn on_network_change(&self) {}

    /// 判断是否允许向此传输地址发送对端数据报。
    ///
    /// 发送者在解析映射地址后调用此方法，覆盖初始候选、认证后的 NAT 穿透探测和入站学到的路径。
    /// 返回 false 会静默丢弃数据报，不会使 endpoint 失败。
    /// 此方法不筛选发现流量、relay 建立流量或其他控制流量。
    /// 实现必须快速返回，且不得持有 endpoint。
    fn allow_transport_send(&self, _remote: &TransportAddr) -> bool {
        true
    }

    /// Intercept outgoing connections before they are started.
    ///
    /// This is called whenever a new outgoing connection is initiated via [`Endpoint::connect`]
    /// or [`Endpoint::connect_with_opts`].
    ///
    /// If any hook returns [`BeforeConnectOutcome::Reject`], the connection attempt is aborted
    /// before any packets are sent to the remote.
    ///
    /// [`Endpoint::connect`]: crate::Endpoint::connect
    /// [`Endpoint::connect_with_opts`]: crate::Endpoint::connect_with_opts
    fn before_connect<'a>(
        &'a self,
        _remote_addr: &'a EndpointAddr,
        _alpn: &'a [u8],
    ) -> impl Future<Output = BeforeConnectOutcome> + Send + 'a {
        async { BeforeConnectOutcome::Accept }
    }

    /// Intercept both incoming and outgoing connections once the TLS handshake has completed.
    ///
    /// At this point in time, we know the remote's endpoint id and ALPN. If any hook returns
    /// [`AfterHandshakeOutcome::Reject`], the connection is closed with the provided error code
    /// and reason.
    ///
    /// The hook receives the [`Connection`] by reference so that implementations can read any
    /// information they need synchronously. Do not clone the [`Connection`] out of the hook:
    /// holding a strong [`Connection`] handle keeps the connection alive and disables
    /// close-on-drop, which the primary use sites of the connection might rely on. If you need to
    /// keep a reference for later (for example to wait for the connection to close, or to
    /// look up the connection in a map), call [`Connection::weak_handle`] and store the
    /// resulting [`WeakConnectionHandle`] instead.
    ///
    /// [`WeakConnectionHandle`]: crate::endpoint::WeakConnectionHandle
    fn after_handshake<'a>(
        &'a self,
        _conn: &'a Connection,
    ) -> impl Future<Output = AfterHandshakeOutcome> + Send + 'a {
        async { AfterHandshakeOutcome::accept() }
    }
}

pub(crate) trait DynEndpointHooks: std::fmt::Debug + Send + Sync {
    fn on_network_change(&self);
    fn allow_transport_send(&self, remote: &TransportAddr) -> bool;
    fn before_connect<'a>(
        &'a self,
        remote_addr: &'a EndpointAddr,
        alpn: &'a [u8],
    ) -> BoxFuture<'a, BeforeConnectOutcome>;
    fn after_handshake<'a>(&'a self, conn: &'a Connection) -> BoxFuture<'a, AfterHandshakeOutcome>;
}

impl<T: EndpointHooks> DynEndpointHooks for T {
    fn on_network_change(&self) {
        EndpointHooks::on_network_change(self)
    }

    fn allow_transport_send(&self, remote: &TransportAddr) -> bool {
        EndpointHooks::allow_transport_send(self, remote)
    }

    fn before_connect<'a>(
        &'a self,
        remote_addr: &'a EndpointAddr,
        alpn: &'a [u8],
    ) -> BoxFuture<'a, BeforeConnectOutcome> {
        Box::pin(EndpointHooks::before_connect(self, remote_addr, alpn))
    }

    fn after_handshake<'a>(&'a self, conn: &'a Connection) -> BoxFuture<'a, AfterHandshakeOutcome> {
        Box::pin(EndpointHooks::after_handshake(self, conn))
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct EndpointHooksList {
    inner: Vec<Arc<dyn DynEndpointHooks>>,
}

impl EndpointHooksList {
    pub(crate) fn on_network_change(&self) {
        for hook in &self.inner {
            hook.on_network_change();
        }
    }

    pub(crate) fn allow_transport_send(&self, remote: &TransportAddr) -> bool {
        self.inner
            .iter()
            .all(|hook| hook.allow_transport_send(remote))
    }

    pub(super) fn push(&mut self, hook: impl EndpointHooks + 'static) {
        let hook: Arc<dyn DynEndpointHooks> = Arc::new(hook);
        self.inner.push(hook);
    }

    pub(super) async fn before_connect(
        &self,
        remote_addr: &EndpointAddr,
        alpn: &[u8],
    ) -> BeforeConnectOutcome {
        for hook in self.inner.iter() {
            match hook.before_connect(remote_addr, alpn).await {
                BeforeConnectOutcome::Accept => continue,
                reject @ BeforeConnectOutcome::Reject => {
                    return reject;
                }
            }
        }
        BeforeConnectOutcome::Accept
    }

    pub(super) async fn after_handshake(&self, conn: &Connection) -> AfterHandshakeOutcome {
        for hook in self.inner.iter() {
            match hook.after_handshake(conn).await {
                AfterHandshakeOutcome::Accept => continue,
                reject @ AfterHandshakeOutcome::Reject { .. } => {
                    return reject;
                }
            }
        }
        AfterHandshakeOutcome::Accept
    }
}
