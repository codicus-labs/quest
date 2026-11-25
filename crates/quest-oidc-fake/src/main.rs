use std::net::SocketAddr;
use std::process::ExitCode;

use clap::Parser;
use quest_oidc_fake::{FakeConfig, FakeProvider};
use tracing_subscriber::EnvFilter;

/// Development OpenID Connect provider for quest-server (users anna and ben, no passwords).
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Address to listen on; the issuer is http://<bind>.
    #[arg(long, env = "QUEST_OIDC_FAKE_BIND", default_value = "127.0.0.1:8792")]
    bind: SocketAddr,
    /// Sign this user in without showing the picker (e.g. anna), for scripted runs.
    #[arg(long, env = "QUEST_OIDC_FAKE_AUTO_LOGIN")]
    auto_login: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();
    let args = Args::parse();
    let config = FakeConfig { bind: args.bind, auto_login: args.auto_login, ..FakeConfig::default() };
    let provider = match FakeProvider::start(config).await {
        Ok(provider) => provider,
        Err(error) => {
            tracing::error!(%error, bind = %args.bind, "cannot listen; pick another address with --bind");
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(
        "OIDC fake listening: QUEST_OIDC_ISSUER={} QUEST_OIDC_CLIENT_ID={} QUEST_OIDC_CLIENT_SECRET={}",
        provider.issuer(),
        provider.client_id(),
        provider.client_secret()
    );
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "cannot wait for Ctrl-C; shutting down");
    }
    provider.shutdown().await;
    ExitCode::SUCCESS
}
