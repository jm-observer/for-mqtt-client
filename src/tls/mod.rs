use anyhow::Result;
use std::path::{Path, PathBuf};

pub mod native_tls;
pub mod rustls;

#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    verify_server: VerifyServer,
    verify_client: VerifyClient,
}

impl TlsConfig {
    pub fn set_server_ca_pem_file(mut self, ca_file: PathBuf) -> Self {
        self.verify_server = VerifyServer::SelfSigned {
            verify_dns_name: self.verify_server.verify_dns_name(),
            ca_file: CertificateFile::Pem(ca_file),
        };
        self
    }
    pub fn set_verify_dns_name(mut self, verify: bool) -> Self {
        self.verify_server.set_verify_dns_name(verify);
        self
    }
}

#[derive(Debug, Clone)]
pub enum VerifyServer {
    CA {
        verify_dns_name: bool,
    },
    SelfSigned {
        verify_dns_name: bool,
        ca_file: CertificateFile,
    },
}

impl VerifyServer {
    pub fn verify_dns_name(&self) -> bool {
        match self {
            VerifyServer::CA { verify_dns_name } => *verify_dns_name,
            VerifyServer::SelfSigned {
                verify_dns_name, ..
            } => *verify_dns_name,
        }
    }
    pub fn set_verify_dns_name(&mut self, verify: bool) {
        match self {
            VerifyServer::CA { verify_dns_name } => *verify_dns_name = verify,
            VerifyServer::SelfSigned {
                verify_dns_name, ..
            } => *verify_dns_name = verify,
        }
    }
}

#[derive(Debug, Clone)]
pub enum VerifyClient {
    No,
    Verify {
        certificate_file: CertificateFile,
        key_file: PrivateKeyFile,
    },
}

impl Default for VerifyClient {
    fn default() -> Self {
        Self::No
    }
}
impl Default for VerifyServer {
    fn default() -> Self {
        Self::CA {
            verify_dns_name: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum CertificateFile {
    Pem(PathBuf),
}

impl CertificateFile {
    pub fn load(&self) -> Result<Vec<Vec<u8>>> {
        match self {
            CertificateFile::Pem(path) => load_pem_certs(path),
        }
    }
}
#[derive(Debug, Clone)]
pub enum PrivateKeyFile {
    Rsa(PathBuf),
    Pkcs8(PathBuf),
}

impl PrivateKeyFile {
    pub fn load(&self) -> Result<Option<Vec<u8>>> {
        let mut datas = match self {
            PrivateKeyFile::Rsa(path) => load_rsa_key(path)?,
            PrivateKeyFile::Pkcs8(path) => load_pkcs8_key(path)?,
        };
        if datas.len() >= 1 {
            Ok(Some(datas.remove(0)))
        } else {
            Ok(None)
        }
    }
}

fn load_rsa_key(path: impl AsRef<Path>) -> Result<Vec<Vec<u8>>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    Ok(rustls_pemfile::rsa_private_keys(&mut reader)?)
}

fn load_pkcs8_key(path: impl AsRef<Path>) -> Result<Vec<Vec<u8>>> {
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    Ok(rustls_pemfile::pkcs8_private_keys(&mut reader)?)
}

fn load_pem_certs(path: &Path) -> Result<Vec<Vec<u8>>> {
    let f = std::fs::File::open(&path)?;
    let mut f = std::io::BufReader::new(f);

    Ok(rustls_pemfile::certs(&mut f)?)
}
