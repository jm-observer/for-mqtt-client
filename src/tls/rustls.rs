use crate::tls::{TlsConfig, VerifyClient, VerifyServer};
use anyhow::{anyhow, Result};
use log::trace;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, ClientConfig, Error, PrivateKey, ServerName};

use std::sync::Arc;
use std::time::SystemTime;

use tokio_rustls::{webpki, TlsConnector};
use webpki::DnsNameRef;

pub fn init_rustls(tls_config: TlsConfig) -> Result<TlsConnector> {
    let TlsConfig {
        verify_server,
        verify_client,
    } = tls_config;

    let roots = rustls_native_certs::load_native_certs()?;

    let builder = match verify_server {
        VerifyServer::CA { verify_dns_name } => {
            let verifier = Arc::new(PkiVerifier::new(roots, verify_dns_name));
            ClientConfig::builder()
                .with_safe_defaults()
                .with_custom_certificate_verifier(verifier)
        }
        VerifyServer::SelfSigned {
            verify_dns_name,
            ca_file,
        } => {
            let roots: Vec<rustls_native_certs::Certificate> = ca_file
                .load()?
                .into_iter()
                .map(|x| rustls_native_certs::Certificate(x))
                .collect();
            // debug!("{}", roots.len());
            let verifier = Arc::new(PkiVerifier::new(roots, verify_dns_name));
            ClientConfig::builder()
                .with_safe_defaults()
                .with_custom_certificate_verifier(verifier)
        }
    };
    Ok(TlsConnector::from(Arc::new(match verify_client {
        VerifyClient::No => builder.with_no_client_auth(),
        VerifyClient::Verify {
            certificate_file,
            key_file,
        } => {
            let certs = certificate_file
                .load()?
                .into_iter()
                .map(|x| Certificate(x))
                .collect();
            let key = key_file.load()?.ok_or(anyhow!("key data invalid"))?;

            builder.with_single_cert(certs, PrivateKey(key))?
        }
    })))
}

/// Default `ServerCertVerifier`, see the trait impl for more information.
#[allow(unreachable_pub)]
#[cfg_attr(docsrs, doc(cfg(feature = "dangerous_configuration")))]
pub struct PkiVerifier {
    roots: Vec<rustls_native_certs::Certificate>,
    verify_dns_name: bool,
}

#[allow(unreachable_pub)]
impl PkiVerifier {
    pub fn new(roots: Vec<rustls_native_certs::Certificate>, verify_dns_name: bool) -> Self {
        Self {
            roots,
            verify_dns_name,
        }
    }
}

impl ServerCertVerifier for PkiVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &Certificate,
        intermediates: &[Certificate],
        server_name: &ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        ocsp_response: &[u8],
        now: SystemTime,
    ) -> Result<ServerCertVerified, Error> {
        let cert = webpki::EndEntityCert::try_from(end_entity.0.as_ref()).map_err(pki_error)?;

        let chain: Vec<&[u8]> = intermediates.iter().map(|cert| cert.0.as_ref()).collect();
        let trustroots: Vec<webpki::TrustAnchor> = self
            .roots
            .iter()
            .filter_map(|x| match webpki::TrustAnchor::try_from_cert_der(&x.0) {
                Ok(anchor) => Some(anchor),
                Err(_) => None,
            })
            .collect();
        let webpki_now = webpki::Time::try_from(now).map_err(|_| Error::FailedToGetCurrentTime)?;

        let cert = cert
            .verify_is_valid_tls_server_cert(
                SUPPORTED_SIG_ALGS,
                &webpki::TlsServerTrustAnchors(&trustroots),
                &chain,
                webpki_now,
            )
            .map_err(pki_error)
            .map(|_| cert)?;

        if !ocsp_response.is_empty() {
            trace!("Unvalidated OCSP response: {:?}", ocsp_response.to_vec());
        }

        if self.verify_dns_name {
            let dns_name = match server_name {
                ServerName::DnsName(dns_name) => dns_name,
                ServerName::IpAddress(_) => {
                    return Err(Error::UnsupportedNameType);
                }
                _ => {
                    return Err(Error::UnsupportedNameType);
                }
            };
            let dns_name_ref = DnsNameRef::try_from_ascii_str(dns_name.as_ref())
                .map_err(|_| Error::UnsupportedNameType)?;
            cert.verify_is_valid_for_dns_name(dns_name_ref)
                .map_err(pki_error)
                .map(|_| ServerCertVerified::assertion())
        } else {
            Ok(ServerCertVerified::assertion())
        }
    }
}

fn pki_error(error: webpki::Error) -> Error {
    use webpki::Error::*;
    match error {
        BadDer | BadDerTime => Error::InvalidCertificateEncoding,
        InvalidSignatureForPublicKey => Error::InvalidCertificateSignature,
        UnsupportedSignatureAlgorithm | UnsupportedSignatureAlgorithmForPublicKey => {
            Error::InvalidCertificateSignatureType
        }
        e => Error::InvalidCertificateData(format!("invalid peer certificate: {}", e)),
    }
}

type SignatureAlgorithms = &'static [&'static webpki::SignatureAlgorithm];
/// Which signature verification mechanisms we support.  No particular
/// order.
static SUPPORTED_SIG_ALGS: SignatureAlgorithms = &[
    &webpki::ECDSA_P256_SHA256,
    &webpki::ECDSA_P256_SHA384,
    &webpki::ECDSA_P384_SHA256,
    &webpki::ECDSA_P384_SHA384,
    &webpki::ED25519,
    &webpki::RSA_PSS_2048_8192_SHA256_LEGACY_KEY,
    &webpki::RSA_PSS_2048_8192_SHA384_LEGACY_KEY,
    &webpki::RSA_PSS_2048_8192_SHA512_LEGACY_KEY,
    &webpki::RSA_PKCS1_2048_8192_SHA256,
    &webpki::RSA_PKCS1_2048_8192_SHA384,
    &webpki::RSA_PKCS1_2048_8192_SHA512,
    &webpki::RSA_PKCS1_3072_8192_SHA384,
];
