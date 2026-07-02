//! rustls connector and `webpki-roots` loader. Builds a `ClientConfig`
//! configured for client-mode TLS with Mozilla's CA roots.

use std::sync::Arc;

use rustls::ClientConfig;
use tokio_rustls::TlsConnector;

/// Build a `TlsConnector` configured with `webpki-roots`. The same connector
/// is reused across requests.
pub fn build_tls_connector() -> TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}
