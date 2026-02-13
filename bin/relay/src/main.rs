//! This is the main entry point for the relay service.
use crate::{cli::CliArgs, service::RelayService};
use beacon::{
    api::BeaconNodeApi,
    events::{stream::BeaconEventsStream, BeaconEventsClient},
    service::{handle::BeaconHandle, BeaconService},
    BeaconConnection,
};
use clap::Parser;
use futures_util::StreamExt;
use relay_auth::MbsAuth;
use relay_grpc_server::server::GrpcServer;
use relay_http_server::server::{metrics::MetricsServer, RelayHttpServer};
use relay_primitives::{
    beacon::events::BeaconEvent,
    blst::{
        public_key::BlsPublicKey,
        signer::{BlsSigner, ForkDatas, ForkVersion},
    },
    hex,
    revm_primitives::HashMap,
    storage::{head_slot::HeadSlot, payload_attributes::PayloadAttributes},
    TaskSpawner, TokioTaskExecutor,
};
use relay_storage::in_memory::InMemoryStorage;
use std::{
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use tokio::time::Instant;
use tracing::info;
use tracing::log::error;
use tree_hash::Hash256;

/// This is the main entry point for the relay service.
mod service;

/// Cli arguments for the relay service.
mod cli;

#[tokio::main]
async fn main() -> Result<(), StartUpErrors> {
    tracing_subscriber::fmt::init();

    let task_spawner = TokioTaskExecutor::default();

    let args = CliArgs::parse();
    let grpc_addr = args.grpc_addr();
    let http_addr = args.http_addr();
    let fork_version = args.genesis_fork_version()?;
    let default_fork_data = args.fork_data();
    let bls_secret_key = args.bls_secret_key();

    // Start the metrics server.
    let metrics_port = args.metrics_port();
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), metrics_port);
    task_spawner.spawn_critical(
        "metrics-server",
        Box::pin(async move {
            if let Err(e) = MetricsServer::spawn(addr).await {
                error!("Failed to start metrics server: {}", e);
            }
        }),
    );

    // Validate enabled builders
    let enabled_builders = args.enabled_builders();
    if enabled_builders.is_empty() {
        error!("Empty builder whitelist, relay cant be started.");
        return Err(StartUpErrors::EmptyBuilders);
    }

    // Start becon service and beacon event stream
    let beacon_url = args.beacon_url;
    let api = Arc::new(BeaconNodeApi::new(&beacon_url));
    let beacon_events = BeaconEventsClient::new(&beacon_url);
    let (beacon_service, beacon_handle) = BeaconService::<BeaconNodeApi>::new(api);
    task_spawner.spawn_critical("beacon-service", Box::pin(beacon_service));

    // Setup fork data
    let fork_data = setup_fork_data(beacon_handle.clone(), fork_version)
        .await
        .unwrap_or_else(|err| {
            info!(
                ?err,
                "Beacon node debug rpc namespace not activated, falling back to CLI fork data."
            );
            default_fork_data
        });

    let builder_domain = hex::encode(fork_data.compute_builder_domain());
    let proposer_domain = hex::encode(fork_data.compute_proposer_domain());
    info!(?builder_domain, ?proposer_domain, "Fork data setup.");

    // Setup storage
    let storage = setup_storage(
        beacon_events.stream().await,
        enabled_builders,
        fork_data.clone(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Error while setting up storage: {:?}", e);
        StartUpErrors::SetupStorage(e)
    })?;

    // Spawn GRPC server
    task_spawner.spawn_critical(
        "grpc-server",
        Box::pin(GrpcServer::start(
            grpc_addr,
            storage.clone(),
            fork_data.clone(),
        )),
    );

    // Spawn HTTP server
    let signer = BlsSigner::new(bls_secret_key, fork_data.clone());
    let auth = MbsAuth::new(args.mbs_auth_url)?;
    task_spawner.spawn_critical(
        "http-server",
        Box::pin(RelayHttpServer::start(
            http_addr,
            storage.clone(),
            beacon_handle.clone(),
            signer,
            auth,
        )),
    );

    info!("The MEV Relay is synced with the beacon chain.");
    let relay = RelayService::new(
        Arc::new(beacon_events),
        storage,
        beacon_handle,
        args.slots_per_epoch,
    )
    .await;

    relay.await;

    Ok(())
}

/// Errors that can occur while parsing the CLI arguments.
#[derive(Debug, thiserror::Error)]
pub(crate) enum StartUpErrors {
    #[error("Empty builder whitelist, relay cant be started.")]
    EmptyBuilders,
    #[error("{0}")]
    Unauthorized(#[from] relay_auth::AuthError),
    #[error("{0}")]
    InvalidChain(std::string::String),
    #[error("{0}")]
    SetupStorage(eyre::Report),
}

/// Listen for the first slots and head events and setup the storage.
async fn setup_storage(
    mut beacon_events_stream: BeaconEventsStream,
    wlist_builders: Vec<BlsPublicKey>,
    fork_data: ForkDatas,
) -> eyre::Result<InMemoryStorage> {
    let started = Instant::now();

    let mut payload_attributes_event = None;
    let mut head_event = None;

    // Listen for the first slots and head events, if the connection is not established, it will retry.
    while payload_attributes_event.is_none() || head_event.is_none() {
        match beacon_events_stream.next().await {
            Some(BeaconEvent::PayloadAttribute(event)) if payload_attributes_event.is_none() => {
                payload_attributes_event = Some(event);
            }
            Some(BeaconEvent::Head(event)) if head_event.is_none() => {
                head_event = Some(event);
            }
            _ => {} // Skip other events and no need to handle reconnection as it is handled by the beacon events stream.
        }
    }

    // Since we check for `None` in the loop condition, it's safe to unwrap here.
    let attr = payload_attributes_event.unwrap().data;
    let slot = head_event.unwrap().slot;
    let proposal_slot = attr.proposal_slot;
    let elapsed = started.elapsed();

    info!(
        ?slot,
        ?proposal_slot,
        ?elapsed,
        "Fetched latest head slot and payload attributes."
    );

    let duties = Vec::new();
    let regs = HashMap::new();

    Ok(InMemoryStorage::new(
        HeadSlot::from(slot),
        PayloadAttributes::from(attr),
        wlist_builders,
        fork_data,
        regs,
        duties,
    ))
}

async fn setup_fork_data(
    beacon_handle: BeaconHandle,
    genesis_fork_version: ForkVersion,
) -> eyre::Result<ForkDatas> {
    let fork_data = beacon_handle.get_fork_data().await?;

    let genesis_fork_version: [u8; 4] = genesis_fork_version.into();
    let current_version: [u8; 4] = ForkVersion::from_str(&fork_data.data.fork.current_version)
        .map_err(|e| eyre::eyre!(e))?
        .into();
    let genesis_validators_root =
        Hash256::from_str(&fork_data.data.genesis_validators_root).map_err(|e| eyre::eyre!(e))?;

    let fork_datas = ForkDatas::from_genesis_and_current_version(
        genesis_fork_version,
        current_version,
        genesis_validators_root,
    );

    Ok(fork_datas)
}
