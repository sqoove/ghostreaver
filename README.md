# GhostReaver - Solana Trading/Snipping Bot

![Python Version](https://img.shields.io/badge/rust-1.89%2B-blue)
![License](https://img.shields.io/badge/license-MIT-green)

A fully functional Solana trading bot compatible with major DEXs, including Bonk, PumpFun, PumpSwap and Raydium. High-performance, **Rust**-based Solana event streaming and parsing pipeline with an opinionated, pluggable **trading monitor** and a lightweight **Postgresql** state store. GhostReaver connects to a Geyser (Yellowstone gRPC) endpoint, decodes on-chain instructions for popular Solana DEX protocols (Bonk, PumpFun, PumpSwap, Raydium AMM/CLMM/CPMM), unifies them into typed events, and persists token/market state locally for automated strategies or downstream analytics.

> **Data reset on startup**: by default the app **deletes the `./database` directory on launch** and recreates it. If you want to persist historical data between runs, remove or modify the call to `SolanaStreamer::removedatabase("database");` in `src/main.rs`.

* * *

## Table of contents

- [Features](#features)
- [Architecture](#architecture)
- [Quickstart](#quickstart)
- [Configuration](#configuration)
    - [`config/endpoint.yaml`](#configendpointyaml)
    - [`config/wallet.yaml`](#configwalletyaml)
    - [`config/bot.yaml`](#configbotyaml)
- [Runtime & logs](#runtime--logs)
- [How the pipeline works](#how-the-pipeline-works)
- [Extending parsers / adding a protocol](#extending-parsers--adding-a-protocol)
- [Development](#development)
- [Security & disclaimers](#security--disclaimers)
- [Project layout](#project-layout)
- [License](#license)

* * *

## Features

- **Real-time Geyser streaming** via `yellowstone_grpc_client` with gRPC **backpressure**, batching and optional metrics.
- **Strongly-typed event decoding** for Solana DEX protocols:
    - **PumpFun** – token creation + trades
    - **PumpSwap** – pool creation, buy, sell, withdraw
    - **Raydium** – AMM v4 (initialize, deposit/withdraw/swap), **CLMM**, **CPMM**
    - **Bonk** – initialize & trade flows
- **Unified event interface** (`UnifiedEvent`) + `eventsmatch!` macro for ergonomic routing.
- **RPC utility client** for light account reads and health checks.
- **Trading monitor** (paper/live) with thresholds, trailing stop, partial take-profit, time-based exit, and per-protocol constraints.
- **Config-driven** (YAML) endpoint, wallet, and strategy settings.
- Highly concurrent **Tokio** pipeline; event batching and controlled flushing under load.
- Clear separation of `streaming` (ingest), `events` (decode), `trading` (logic), and `utils` (IO, configs, storage).

* * *

## Architecture

At a glance:

- **`streaming/yellowstone.rs`** – high-level client wrapping Yellowstone gRPC with presets:
    - `new_high_performance`, `new_low_latency`, `subscribe_events_immediate(...)`.
- **`streaming/grpc/*`** – connection, subscription, event processor, batching, metrics.
- **`streaming/events/*`** – protocol parsers + common types & filters; `EventParser` trait & factory.
- **`trading/*`** – pools & price math per protocol, a **monitor** orchestrating entries/exits using config thresholds.
- **`utils/storage.rs`** – Postgresql creation, indices, async writers, ticks buffering, trade bookkeeping.
- **`utils/loader.rs`** – loads `endpoint.yaml`, `wallet.yaml`, `bot.yaml` into typed structs.
- **`globals/*`** – constants (timeouts, batch sizes), well-known program IDs, system addresses.
- **`main.rs`** – wiring: loads configs, clears DB dir, builds gRPC client, sets filters, installs the `eventsmatch!` dispatcher and spawns per-event tasks that write to storage and feed the monitor.

* * *

## Quickstart

### Prerequisites

- **Rust** stable (edition **2024**) – install via <https://rustup.rs>
- **Postgresql** (libsqlite3). Linux (Debian/Ubuntu): `sudo apt-get install postgresql postgresql-contrib`
- Build toolchain: `pkg-config`, a C compiler (`build-essential` on Debian/Ubuntu)

### Build & run

```bash
# from the project root
cargo build --release

# edit configs (see below), then run
RUST_LOG=info cargo run --release
````

> The app logs your wallet public key and connects to the configured RPC + Yellowstone gRPC (Geyser). On start it clears `./database` and creates `database/storage.sqlite` with the required tables and indices.

* * *

## Configuration

All configs live in `config/` and are **YAML**. The provided files are self-documented.

### `config/endpoint.yaml`

```yaml
endpoint:
  rpc: "http://127.0.0.1:8899"       # Solana RPC (Helius/QuickNode/Alchemy or local validator)
  geyser: "http://127.0.0.1:10000"   # Yellowstone gRPC (Geyser) endpoint
  xtoken: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" # Optional auth header for your Geyser gateway
```

### `config/wallet.yaml`

```yaml
wallet:
  publicaddr: "YourPublicKeyBase58"
  privatekey: "YourPrivateKeyBase58OrJSONSeed"
```

> **Never commit secrets.** Add `config/wallet.yaml` to your `.gitignore`. Consider using a KMS or encrypted file.

### `config/bot.yaml`

Contains three main blocks (names may evolve):

* **`main`** – global toggles (e.g. `status`, `sandbox`, `balance`, `opentrades`, `maxtrades`).
* **`monitoring`** – which programs to track (`programs: "all"` or a specific program id), `retries`.
* **`orders`** – per-trade sizing & risk:

    * `amount` (SOL), `buyslippage`, `sellslippage`
    * `stoploss`, `takeprofit`
    * `partialtrigger`, `partialsell`
    * `trailingtrigger`, `trailingsell`, `trailingstop`
    * `timeclose` (ms), `attempts`, `dropmax`

* **Per-protocol rules** (min/max liquidity, timeouts, max observed txs), e.g.:

| Protocol      | minpool | maxpool | exit(ms) | max txs |
| ------------- | ------: | ------: | -------: | ------: |
| `bonk`        |       0 |    1e12 |    60000 |    1e12 |
| `pumpfun`     |       0 |    1e12 |    60000 |    1e12 |
| `pumpswap`    |       0 |    1e12 |    60000 |    1e12 |
| `raydiumamm`  |       0 |    1e12 |    60000 |    1e12 |
| `raydiumclmm` |       0 |    1e12 |    60000 |    1e12 |
| `raydiumcpmm` |       0 |    1e12 |    60000 |    1e12 |

> The monitor computes **per-program thresholds** using `monitor::progthresholds` and enforces trailing/partial/timeout logic as prices update.

* * *

## Runtime & logs

* Logging is via `env_logger` with default filter `info,sqlx=warn`. Override with `RUST_LOG`:

  ```bash
  RUST_LOG=debug,sqlx=info cargo run --release
  ```
* SQLx internal logging is enabled by setting `SQLX_LOG=info` (see `main.rs` sets it).
* Metrics (EPS, processed counts) can be enabled in the Yellowstone client config if desired.

* * *

## How the pipeline works

1. **Connect** to RPC (health check) and Yellowstone gRPC (configurable presets).
2. **Subscribe** with **transaction & account filters** to the set of supported program IDs:

    * PumpFun, PumpSwap, Bonk, Raydium (AMM v4, CLMM, CPMM). See `src/main.rs` for the exact list.
3. **Filter** event types (e.g. buys/sells, pool creation, account state) using `EventTypeFilter::include([...])`.
4. **Decode** each matching update via protocol-specific parsers implementing `EventParser`, producing a `Box<dyn UnifiedEvent>`.
5. **Dispatch** via the `eventsmatch!` macro, spawning Tokio tasks to:

    * **Insert** new tokens/pools (create/initialize events) into `tokens`.
    * **Update** prices, vaults, supply, spreads, tx counts (trade/swap events).
    * **Append** price **ticks** and feed the **trading monitor**, which may open/close/partial-close positions (paper or live).

6. **Persist** ticks/market opens/closes; maintain `trades`, `signature`, `wallet` aggregates.

Batching and **backpressure** protect the pipeline under load; behavior is tuned through `streaming/common/config.rs` and `globals/constants.rs`.

* * *

## Extending parsers / adding a protocol

1. **Create a parser** under `src/streaming/events/protocols/<your_proto>/parser.rs`.

    * Implement the `EventParser` trait, returning a vector of `GenericEventParseConfig` entries for discriminators you care about.
    * Produce strongly-typed events implementing `UnifiedEvent`.
2. **Register** in `events::factory` and the `Protocol` enum.
3. **Add** your program ID to the subscription filters in `main.rs`.
4. **Handle** your new events in the `eventsmatch!` block, writing to storage as needed.

See existing implementations for **PumpSwap**, **PumpFun**, **Raydium** flavors, and **Bonk** for references.

* * *

## Development

* Format & lint:

  ```bash
  cargo fmt
  cargo clippy --all-targets -- -D warnings
  ```

* Run against a **local validator** (optional) and a **local Geyser** (Yellowstone gRPC) or a hosted provider.
* Useful constants (timeouts, buffer sizes) live in `globals/constants.rs`.
* The `database` directory is recreated each run; remove that behavior if you need persistence.

* * *

## Security & disclaimers

* **Keys**: keep `config/wallet.yaml` private. Consider keeping it out of the repo and loading from a secure path/secret store.
* **Trading risk**: the monitor implements stop-loss, partials, and trailing logic, but markets are volatile. **Use `sandbox: true`** while testing. Nothing here is financial advice.
* **Safety**: validate all endpoints (RPC/Geyser) you use. Prefer https/tls where possible.

* * *

## Project layout

```
ghostreaver/
├─ Cargo.toml
├─ README.md
├─ config/
│  ├─ endpoint.yaml      # RPC + Geyser
│  ├─ wallet.yaml        # Public/Private keys
│  └─ bot.yaml           # Strategy/monitor config
├─ src/
│  ├─ main.rs
│  ├─ lib.rs
│  ├─ globals/
│  ├─ core/              # RPC client
│  ├─ streaming/
│  │  ├─ yellowstone.rs
│  │  ├─ grpc/           # connection, subscription, processor, types
│  │  └─ events/         # common/, core/, protocols/{pumpfun,pumpswap,raydium*,bonk,mutil}
│  ├─ trading/           # pools, monitor, scanner, shared
│  ├─ utils/             # loader, storage, scripts (stats helpers)
│  └─ schema/            # serde models (e.g., TradeInfo)
└─ database/             # created at runtime (storage.sqlite)
```

* * *

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the repository.
2. Create a new branch (`git checkout -b feature/your-feature`).
3. Make your changes and commit them (`git commit -m "Add your feature"`).
4. Push to your branch (`git push origin feature/your-feature`).
5. Open a pull request with a clear description of your changes.

Ensure your code follows PEP 8 style guidelines and includes appropriate tests.

* * *

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

* * *

## Contact

For any issues, suggestions, or questions regarding the project, please open a new issue on the official GitHub repository or reach out directly to the maintainer through the [GitHub Issues](issues) page for further assistance and follow-up.