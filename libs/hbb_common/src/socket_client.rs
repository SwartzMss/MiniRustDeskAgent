use crate::{
    config::{Config, NetworkType},
    tcp::FramedStream,
    udp::FramedSocket,
    ResultType,
};
use anyhow::Context;
use std::net::SocketAddr;
use tokio::net::ToSocketAddrs;
use tokio_socks::{IntoTargetAddr, TargetAddr};

#[inline]
pub fn check_port<T: std::string::ToString>(host: T, port: i32) -> String {
    let host = host.to_string();
    if crate::is_ipv6_str(&host) {
        if host.starts_with('[') {
            return host;
        }
        return format!("[{host}]:{port}");
    }
    if !host.contains(':') {
        return format!("{host}:{port}");
    }
    host
}

#[inline]
pub fn increase_port<T: std::string::ToString>(host: T, offset: i32) -> String {
    let host = host.to_string();
    if crate::is_ipv6_str(&host) {
        if host.starts_with('[') {
            let tmp: Vec<&str> = host.split("]:").collect();
            if tmp.len() == 2 {
                let port: i32 = tmp[1].parse().unwrap_or(0);
                if port > 0 {
                    return format!("{}]:{}", tmp[0], port + offset);
                }
            }
        }
    } else if host.contains(':') {
        let tmp: Vec<&str> = host.split(':').collect();
        if tmp.len() == 2 {
            let port: i32 = tmp[1].parse().unwrap_or(0);
            if port > 0 {
                return format!("{}:{}", tmp[0], port + offset);
            }
        }
    }
    host
}

pub fn test_if_valid_server(host: &str, test_with_proxy: bool) -> String {
    let host = check_port(host, 0);
    use std::net::ToSocketAddrs;

    if test_with_proxy && NetworkType::ProxySocks == Config::get_network_type() {
        test_if_valid_server_for_proxy_(&host)
    } else {
        match host.to_socket_addrs() {
            Err(err) => err.to_string(),
            Ok(_) => "".to_owned(),
        }
    }
}

#[inline]
pub fn test_if_valid_server_for_proxy_(host: &str) -> String {
    // `&host.into_target_addr()` is defined in `tokio-socs`, but is a common pattern for testing,
    // it can be used for both `socks` and `http` proxy.
    match &host.into_target_addr() {
        Err(err) => err.to_string(),
        Ok(_) => "".to_owned(),
    }
}

pub trait IsResolvedSocketAddr {
    fn resolve(&self) -> Option<&SocketAddr>;
}

impl IsResolvedSocketAddr for SocketAddr {
    fn resolve(&self) -> Option<&SocketAddr> {
        Some(self)
    }
}

impl IsResolvedSocketAddr for String {
    fn resolve(&self) -> Option<&SocketAddr> {
        None
    }
}

impl IsResolvedSocketAddr for &str {
    fn resolve(&self) -> Option<&SocketAddr> {
        None
    }
}

#[inline]
pub async fn connect_tcp<
    't,
    T: IntoTargetAddr<'t> + ToSocketAddrs + IsResolvedSocketAddr + std::fmt::Display,
>(
    target: T,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    connect_tcp_local(target, None, ms_timeout).await
}

pub async fn connect_tcp_local<
    't,
    T: IntoTargetAddr<'t> + ToSocketAddrs + IsResolvedSocketAddr + std::fmt::Display,
>(
    target: T,
    local: Option<SocketAddr>,
    ms_timeout: u64,
) -> ResultType<FramedStream> {
    if let Some(conf) = Config::get_socks() {
        return FramedStream::connect(target, local, &conf, ms_timeout).await;
    }
    if let Some(target) = target.resolve() {
        if let Some(local) = local {
            if local.is_ipv6() && target.is_ipv4() {
                let target = query_nip_io(target).await?;
                return FramedStream::new(target, Some(local), ms_timeout).await;
            }
        }
    }
    FramedStream::new(target, local, ms_timeout).await
}

#[inline]
pub fn is_ipv4(target: &TargetAddr<'_>) -> bool {
    match target {
        TargetAddr::Ip(addr) => addr.is_ipv4(),
        _ => true,
    }
}

#[inline]
pub async fn query_nip_io(addr: &SocketAddr) -> ResultType<SocketAddr> {
    tokio::net::lookup_host(format!("{}.nip.io:{}", addr.ip(), addr.port()))
        .await?
        .find(|x| x.is_ipv6())
        .context("Failed to get ipv6 from nip.io")
}

#[inline]
pub fn ipv4_to_ipv6(addr: String, ipv4: bool) -> String {
    if !ipv4 && crate::is_ipv4_str(&addr) {
        if let Some(ip) = addr.split(':').next() {
            return addr.replace(ip, &format!("{ip}.nip.io"));
        }
    }
    addr
}

async fn test_target(target: &str) -> ResultType<SocketAddr> {
    if let Ok(Ok(s)) = super::timeout(1000, tokio::net::TcpStream::connect(target)).await {
        if let Ok(addr) = s.peer_addr() {
            return Ok(addr);
        }
    }
    tokio::net::lookup_host(target)
        .await?
        .next()
        .context(format!("Failed to look up host for {target}"))
}

#[inline]
pub async fn new_udp_for(
    target: &str,
    ms_timeout: u64,
) -> ResultType<(FramedSocket, TargetAddr<'static>)> {
    let (ipv4, target) = if NetworkType::Direct == Config::get_network_type() {
        let addr = test_target(target).await?;
        (addr.is_ipv4(), addr.into_target_addr()?)
    } else {
        (true, target.into_target_addr()?)
    };
    Ok((
        new_udp(Config::get_any_listen_addr(ipv4), ms_timeout).await?,
        target.to_owned(),
    ))
}

async fn new_udp<T: ToSocketAddrs>(local: T, ms_timeout: u64) -> ResultType<FramedSocket> {
    match Config::get_socks() {
        None => Ok(FramedSocket::new(local).await?),
        Some(conf) => {
            let socket = FramedSocket::new_proxy(
                conf.proxy.as_str(),
                local,
                conf.username.as_str(),
                conf.password.as_str(),
                ms_timeout,
            )
            .await?;
            Ok(socket)
        }
    }
}

pub async fn rebind_udp_for(
    target: &str,
) -> ResultType<Option<(FramedSocket, TargetAddr<'static>)>> {
    if Config::get_network_type() != NetworkType::Direct {
        return Ok(None);
    }
    let addr = test_target(target).await?;
    let v4 = addr.is_ipv4();
    Ok(Some((
        FramedSocket::new(Config::get_any_listen_addr(v4)).await?,
        addr.into_target_addr()?.to_owned(),
    )))
}
