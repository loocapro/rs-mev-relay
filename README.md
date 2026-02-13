# mev-relay

A Rust MEV relay implementing the Proposer–Builder Separation (PBS) architecture.

### Disclaimer

**Use at your own risk.** Maintainers do not take responsibility for production use or deployment decisions.

- **Vintage** — July 2024; some dependencies may be outdated.
- **Purpose** — Educational only; reference for architecture and clean code.
- **Trust** — The relay enforces a trust assumption with validators: proposer API access is gated by an API key (query parameter `api_key`) validated against `--auth.url`, and only registered validators may submit blinded blocks. See [Proposer authentication](#proposer-authentication).
- **Testing** — This relay has been tested on a local devnet and has successfully proposed blocks.

## CI and testing

Every pull request runs:

- **check** – `cargo check`
- **test** – `cargo test --all` with coverage (grcov); the job log prints a coverage summary
- **fmt** – `cargo fmt --check`
- **clippy** – `cargo clippy --workspace --tests --all-features` (warnings as errors)

## Dependencies

To run the relay locally you need protobuf:

```bash
brew install protobuf
```

On **macOS (Apple Silicon)**, also install libffi and pkg-config so the build can use the system libffi:

```bash
brew install libffi pkg-config
```

## Run Locally

The relay needs a Beacon node HTTP endpoint. By default it uses `http://localhost:3500`; override with `--beacon.url`. You must provide your own BLS secret key and configure auth (see below). **Never use example keys in production.**

```bash
RUST_LOG=info cargo run --bin relay -- \
   --builders.enabled <BUILDER_PUBKEY_HEX> \
   --bls-secret-key <YOUR_BLS_SECRET_KEY> \
   --auth.url <YOUR_AUTH_API_BASE_URL>
```

Example for mainnet (replace placeholders with your values):

```bash
RUST_LOG=info cargo run --bin relay -- \
   --builders.enabled 0x<builder_pubkey> \
   --bls-secret-key 0x<your_bls_secret_key> \
   --auth.url https://your-auth-provider.example/
```

By default the relay runs for mainnet.

## Networks

Use the `--chain` argument:

- `--chain mainnet`
- `--chain dev`
- `--chain goerli`
- `--chain holesky`

Example:

```bash
RUST_LOG=info cargo run --bin relay -- --chain holesky
```

The Consensus Layer (CL) you connect to must have the debug API enabled. Fork info is fetched from the [Beacon API](https://ethereum.github.io/beacon-APIs/#/Debug/getStateV2). If the debug API is unavailable, you can set fork data explicitly:

```
--fork-data.genesis-version <GENESIS_FORK_VERSION>
--fork-data.current-version <CURRENT_FORK_VERSION>
--fork-data.genesis-validators-root <GENESIS_VALIDATORS_ROOT>
```

## Proposer authentication

Proposer API endpoints require an API key validated by an external auth service. Set `--auth.url` to your auth provider’s base URL (e.g. the endpoint that accepts a secret and returns user/team info). Validators use the `api_key` query parameter when calling the proposer API; the relay forwards validation to `--auth.url`.

## CLI options

List all options:

```bash
cargo run --bin relay -- --help
```

## Building and running with Docker

Build the image from this repo:

```bash
docker build -t mev-relay .
docker run mev-relay --help
```

Run with your own config (BLS key, auth URL, builders, beacon URL, etc.) via env or command args. **No pre-built public image is provided; deploy and operate the relay at your own responsibility.**

## Verification

The proposer API exposes `GET /eth/v1/builder/status`. Example:

```bash
curl http://localhost:9063/eth/v1/builder/status
```

(Use the port set by `--http.port`, default 9063.)

## Metrics and monitoring

Metrics are served in Prometheus format. Default port is 9090 (override with `--metrics.port`). A sample Grafana dashboard is available under `assets/dashboard.json`.
