//! TLS 证书生成模块
//!
//! 使用 rcgen 生成自签名证书，用于 TLS 隧道和测试。

use anyhow::{Context, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// 生成自签名证书和私钥
fn generate_self_signed_cert() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    use rcgen::{CertificateParams, KeyPair, SignatureAlgorithm};

    let key_pair = KeyPair::generate_for(&SignatureAlgorithm::ECDSA_P256)
        .context("生成密钥对失败")?;

    let mut params = CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
        .map_err(|e| anyhow::anyhow!("证书参数创建失败: {}", e))?;

    // 设置证书有效期
    let now = SystemTime::now();
    params.not_before = now;
    params.not_after = now + Duration::from_secs(365 * 24 * 3600); // 1 年
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::DigitalSignature,
        rcgen::KeyUsagePurpose::KeyEncipherment,
        rcgen::KeyUsagePurpose::KeyCertSign,
    ];

    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| anyhow::anyhow!("自签名证书生成失败: {}", e))?;

    let cert_der = cert.der().clone();
    let key_der = PrivatePkcs8KeyDer::from(key_pair.serialize_der());
    let private_key = PrivateKeyDer::Pkcs8(key_der);

    Ok((cert_der, private_key))
}

/// 生成服务端 TLS 配置（使用自签名证书）
pub fn generate_self_signed_server_config() -> Result<Arc<rustls::ServerConfig>> {
    let (cert, key) = generate_self_signed_cert()?;

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .context("服务端 TLS 配置创建失败")?;

    Ok(Arc::new(config))
}

/// 生成服务端 TLS 配置（mTLS 模式，要求客户端证书）
pub fn generate_mtls_server_config(
    client_ca_cert: CertificateDer<'static>,
) -> Result<Arc<rustls::ServerConfig>> {
    let (cert, key) = generate_self_signed_cert()?;

    let mut root_store = rustls::RootCertStore::empty();
    root_store
        .add(client_ca_cert)
        .context("添加 CA 证书失败")?;

    let config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(
            rustls::server::WebPkiClientVerifier::builder(root_store.into())
                .build()
                .context("构建客户端验证器失败")?,
        )
        .with_single_cert(vec![cert], key)
        .context("mTLS 服务端配置创建失败")?;

    Ok(Arc::new(config))
}

/// 生成客户端 TLS 配置（验证服务端证书）
pub fn generate_client_config(
    server_ca_cert: Option<CertificateDer<'static>>,
) -> Result<Arc<rustls::ClientConfig>> {
    let mut root_store = rustls::RootCertStore::empty();

    if let Some(ca_cert) = server_ca_cert {
        root_store
            .add(ca_cert)
            .context("添加 CA 证书失败")?;
    } else {
        // 使用系统根证书
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(Arc::new(config))
}

/// 生成客户端 TLS 配置（mTLS 模式）
pub fn generate_mtls_client_config(
    client_cert: CertificateDer<'static>,
    client_key: PrivateKeyDer<'static>,
    server_ca_cert: Option<CertificateDer<'static>>,
) -> Result<Arc<rustls::ClientConfig>> {
    let mut root_store = rustls::RootCertStore::empty();

    if let Some(ca_cert) = server_ca_cert {
        root_store
            .add(ca_cert)
            .context("添加 CA 证书失败")?;
    } else {
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_client_auth_cert(vec![client_cert], client_key)
        .map_err(|e| anyhow::anyhow!("客户端 mTLS 配置失败: {}", e))?;

    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_self_signed_cert() {
        let result = generate_self_signed_cert();
        assert!(result.is_ok());
        let (cert, _key) = result.unwrap();
        assert!(!cert.is_empty());
    }

    #[test]
    fn test_generate_server_config() {
        let result = generate_self_signed_server_config();
        assert!(result.is_ok());
    }
}
