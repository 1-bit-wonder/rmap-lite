# rmap-lite 🦀

A lightweight, high-performance concurrent port scanner written in Rust.

`rmap-lite` is a learning project focused on asynchronous I/O and efficient resource management. It uses the `Tokio` runtime to scan thousands of ports in seconds without overwhelming system resources.

## ✨ Features

* **Asynchronous Scanning:** Built with `Tokio` and `Futures` for non-blocking network I/O.
* **Concurrency Control:** Uses `buffer_unordered` to limit active connections, preventing OS file descriptor exhaustion.
* **Smart Timeouts:** Configurable timeouts to ensure the scanner doesn't hang on "filtered" ports or firewalls.
* **User-Friendly CLI:** Powered by `Clap` for easy configuration.
* **Progress Tracking:** Real-time feedback with a dynamic progress bar and ETA via `indicatif`.

## 🚀 Installation

Ensure you have the Rust toolchain installed, then clone and build:

```bash
git clone https://github.com/1-bit-wonder/rmap-lite.git
cd rmap-lite
cargo build --release

```

## 🛠 Usage

Basic scan of localhost:

```bash
./target/release/rmap-lite --target 127.0.0.1

```

Scanning a specific range with custom concurrency and timeout:

```bash
./target/release/rmap-lite \
  --target google.com \
  --port-from 1 \
  --port-to 1000 \
  --concurrency 2000 \
  --timeout 500

```

### Options

| Flag | Description | Default |
| --- | --- | --- |
| `-t, --target` | The IP address or hostname to scan | `127.0.0.1` |
| `--port-from` | Starting port of the range | `1` |
| `--port-to` | Ending port of the range | `1024` |
| `-c, --concurrency` | Max simultaneous connections | `1000` |
| `--timeout` | Connection timeout in milliseconds | `1500` |

## 📚 Learning Goals

This project was built to explore:

* Rust's `async/await` ecosystem.
* Managing streams with the `futures` crate.
* Safe error handling and DNS resolution in a networked environment.
