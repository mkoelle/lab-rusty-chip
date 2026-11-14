# RUSTY CHIP

A collection of ESP32 projects written in Rust for Robot and IoT club.

Target hardware: ESP32 (Xtensa) dev boards with a CP2102 USB-serial chip, e.g. ESP32-WROOM-32 USB-C boards.
Projects use `std` via [`esp-idf-svc`](https://github.com/esp-rs/esp-idf-svc).

## Repository Layout

```text
projects/<name>/   one firmware project per folder (own Cargo.toml, toolchain, Wokwi diagram)
crates/            hardware-independent Rust crates, unit-tested on your computer
mise.toml          pinned tools and the tasks below
```

Keep logic that doesn't touch pins (state machines, debouncing, parsing) in `crates/` so it can be tested with plain `cargo test`.
See [`crates/stoplight-logic`](crates/stoplight-logic/src/lib.rs) for an example.

## Setup (macOS)

You need git and python3 (`xcode-select --install` provides both), [mise](https://mise.jdx.dev) (`brew install mise`, then follow its shell activation step), and [rustup](https://rustup.rs):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then, from the repo root:

```sh
mise trust           # allow this repo's mise.toml
mise install         # espup, espflash, ldproxy, wokwi-cli at pinned versions
mise run setup       # Xtensa Rust toolchain for the ESP32 (one time, a few minutes)
mise run build blinky
```

The first build of any project downloads ESP-IDF and its own cmake/ninja into `~/.espressif` (several minutes).
Every later project reuses it.

mise loads the toolchain environment automatically whenever you `cd` into the repo; no `export-esp.sh` in your shell profile needed.

### Updating tool versions

All versions are pinned in [mise.toml](mise.toml).
When bumping `ESP_TOOLCHAIN_VERSION`, change it in [.github/workflows/rust_ci.yml](.github/workflows/rust_ci.yml) too, then re-run `mise run setup`.

### Behind a corporate proxy

Point the tools at your company's CA bundle instead of disabling certificate checks:

```sh
export SSL_CERT_FILE=/path/to/corporate-ca.pem
export REQUESTS_CA_BUNDLE="$SSL_CERT_FILE"   # python / ESP-IDF tools
export CURL_CA_BUNDLE="$SSL_CERT_FILE"
```

## Everyday commands

Run `mise tasks` for the full list.

| Command | What it does |
|---|---|
| `mise run build <project>` | Build firmware |
| `mise run flash <project>` | Flash a board and open the serial monitor |
| `mise run monitor` | Serial monitor only |
| `mise run sim <project>` | Build and run in the Wokwi simulator |
| `mise run test-sim <project>` | Run the project's Wokwi scenario test |
| `mise run test` | Unit tests for `crates/` |
| `mise run check` | fmt + clippy everywhere (what CI runs) |

Or work inside a project folder with plain `cargo build` / `cargo run`.

## Connecting a board

1. Use a **data** USB-C cable; many cables are charge-only and the board will never show up.
2. Check the port appears: `ls /dev/cu.*` should list `cu.usbserial-XXXX` or `cu.SLAB_USBtoUART`.
   If nothing appears with a known-good cable, install the [Silicon Labs CP210x VCP driver](https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers) and allow it in System Settings → Privacy & Security.
3. `mise run flash <project>`.
   If it hangs at `Connecting...`, hold **BOOT**, tap **EN/RST**, release **BOOT**.
4. With several boards plugged in, pick one: `ESPFLASH_PORT=/dev/cu.usbserial-XXXX mise run flash <project>`.

## Simulation with Wokwi

Each project has a `diagram.json` (the virtual circuit) and `wokwi.toml` (which firmware to load).
A free Wokwi account is enough.

- **VS Code:** install the recommended Wokwi extension, build the project, then run `Wokwi: Start Simulator` with `diagram.json` open.
- **Command line:** create a token at [wokwi.com/dashboard/ci](https://wokwi.com/dashboard/ci), `export WOKWI_CLI_TOKEN=...`, then `mise run sim <project>`.
- **Automated tests:** a `scenario.test.yaml` in a project presses buttons and checks serial output and pin levels.
  `mise run test-sim stoplight_button` runs it locally; CI runs it when the `WOKWI_CLI_TOKEN` repository secret is set.

New circuits can be drawn at [wokwi.com/projects/new/esp32](https://wokwi.com/projects/new/esp32) and the `diagram.json` copied into the project.

## Editor

Open the repo root in VS Code and install the recommended extensions.
[.vscode/settings.json](.vscode/settings.json) tells rust-analyzer about every project; add new projects to `rust-analyzer.linkedProjects`.

## Adding a project

```sh
mise use -g cargo:cargo-generate
cd projects
cargo generate esp-rs/esp-idf-template cargo   # choose esp32, std
```

Then copy `diagram.json`, `wokwi.toml` and the `ESP_IDF_TOOLS_INSTALL_DIR` line in `.cargo/config.toml` from an existing project, and add it to `.vscode/settings.json`.

## Windows

Use WSL2 and follow the setup above inside it, installing `sudo apt install git python3 python3-venv libudev-dev` first.
USB devices are passed into WSL with [usbipd-win](https://github.com/dorssel/usbipd-win); without it the board won't be visible for flashing.

- https://pfesenmeier.github.io/wsl2-and-embedded-development/
- https://developer.mamezou-tech.com/en/blogs/2025/05/19/using-rust-02/

## References

- [The Rust on ESP Book](https://docs.esp-rs.org/book/)
- [Embedded Rust (std) on Espressif training](https://docs.esp-rs.org/std-training)
- [esp-idf-svc examples](https://github.com/esp-rs/esp-idf-svc/tree/master/examples)
- [Wokwi CI and scenario docs](https://docs.wokwi.com/wokwi-ci/getting-started)
- [Beginner Rust ESP32 snake game](https://jamesmcm.github.io/blog/beginner-rust-esp32-lcdsnake/)
