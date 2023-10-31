const KEY: &'static str = include_str!("../key");
const CRT: &'static str = include_str!("../crt");

use higgs::{client::Client, *};
use quinn::{Endpoint, ServerConfig};
use std::{env, sync::Arc};

#[tokio::main]
async fn main() {
    let server_addr = format!("0.0.0.0:{}", env::var("PORT").unwrap())
        .parse()
        .unwrap();

    let workload = Arc::new(WorkLoad::default());

    let Some(rustls_pemfile::Item::PKCS8Key(key)) =
        rustls_pemfile::read_one(&mut KEY.as_bytes().to_vec().as_slice())
            .ok()
            .flatten()
    else {
        panic!("invalid private key");
    };
    let private_key = rustls::PrivateKey(key);
    let certificate = vec![rustls::Certificate(CRT.as_bytes().to_vec())];

    let mut server_config = ServerConfig::with_single_cert(certificate, private_key).unwrap();

    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_bidi_streams(8192u32.into());
    transport_config.max_concurrent_uni_streams(8192u32.into());

    let endpoint = Endpoint::server(server_config, server_addr).unwrap();

    loop {
        while let Ok(connection) = endpoint.accept().await.unwrap().await {
            Client::new(connection, workload.clone()).start().await;
        }
    }
}
