use axum::{
    routing::{get, post},
    extract::{
        Extension,
        Path,
        Json,
    },
    http::{HeaderMap},
    Router,
};

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use russh::server::{Msg, Session};
use russh::*;
use russh_keys::*;
use tokio::sync::Mutex;
use tokio::try_join;
use std::future::IntoFuture;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(root))
        .route("/:org/:repo/objects/batch", post(objects_batch));

    let http_addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
    let http_server = axum::serve(listener, app).into_future();
    println!("HTTP Listening on {}", http_addr);

    let ssh_addr = ("0.0.0.0", 3001);
    let ssh_config = russh::server::Config {
        inactivity_timeout: Some(std::time::Duration::from_secs(3600)),
        auth_rejection_time: std::time::Duration::from_secs(3),
        auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
        keys: vec![russh_keys::key::KeyPair::generate_ed25519().unwrap()],
        ..Default::default()
    };
    let ssh_config = Arc::new(ssh_config);
    let sh = Server {
        clients: Arc::new(Mutex::new(HashMap::new())),
        id: 0,
    };
    let ssh_server = russh::server::run(ssh_config, ssh_addr, sh);
    println!("SSH Listening on {:?}", ssh_addr);

    try_join!(http_server, ssh_server);
}

async fn root() -> &'static str {
    "Hubless is runnng"
}

async fn objects_batch(
    headers: HeaderMap,
    Path((org, repo)): Path<(String, String)>,
    Json(body): Json<serde_json::Value>
) {
    println!("Got batch request for {}/{}", org, repo);
    println!("Body: {}", body);
    println!("Headers: {:?}", headers);
}

#[derive(Clone)]
struct Server {
    clients: Arc<Mutex<HashMap<(usize, ChannelId), russh::server::Handle>>>,
    id: usize,
}

impl Server {
    async fn post(&mut self, data: CryptoVec) {
        let mut clients = self.clients.lock().await;
        for ((id, channel), ref mut s) in clients.iter_mut() {
            if *id != self.id {
                let _ = s.data(*channel, data.clone()).await;
            }
        }
    }
}

impl server::Server for Server {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        s
    }
}

#[async_trait]
impl server::Handler for Server {
    type Error = anyhow::Error;

    async fn channel_open_session(
        self,
        channel: Channel<Msg>,
        session: Session,
    ) -> Result<(Self, bool, Session), Self::Error> {
        {
            let mut clients = self.clients.lock().await;
            clients.insert((self.id, channel.id()), session.handle());
        }
        Ok((self, true, session))
    }

    async fn auth_publickey(
        self,
        _: &str,
        _: &key::PublicKey,
    ) -> Result<(Self, server::Auth), Self::Error> {
        Ok((self, server::Auth::Accept))
    }

    async fn data(
        mut self,
        channel: ChannelId,
        data: &[u8],
        mut session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        let data = CryptoVec::from(format!("Got data: {}\r\n", String::from_utf8_lossy(data)));
        self.post(data.clone()).await;
        session.data(channel, data);
        Ok((self, session))
    }

    async fn tcpip_forward(
        self,
        address: &str,
        port: &mut u32,
        session: Session,
    ) -> Result<(Self, bool, Session), Self::Error> {
        let handle = session.handle();
        let address = address.to_string();
        let port = *port;
        tokio::spawn(async move {
            let channel = handle
                .channel_open_forwarded_tcpip(address, port, "1.2.3.4", 1234)
                .await
                .unwrap();
            let _ = channel.data(&b"Hello from a forwarded port"[..]).await;
            let _ = channel.eof().await;
        });
        Ok((self, true, session))
    }
}