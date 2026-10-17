# Daily Grind City Simulation

A top-down 2D/2.5D city simulation in which residents with individual personalities live through a 24-hour cycle: commuting, working, eating, shopping, skipping work, and sometimes crashing cars. It is a semi-serious, game-like research tool for studying where and when congestion forms and why.

**Status:** design phase, with a small proof of concept in `poc/`: a native Rust simulation, a browser viewer, and a phone watching it over the local network.

## Key ideas

- Fixed-step, deterministic simulation. Rendering is decoupled and optional.
- Uniform fidelity across the whole city, so incidents can happen anywhere.
- Residents have persistent traits, short-term needs and moods, and binding schedules (work, school, shifts).
- Lane-following movement by default, free-body movement for loss of control and deliberate acts.
- Editable road graph with traffic lights, z-levels (overpasses), emergency services and fires.
- Everything configurable through JSON; a map editor and OpenStreetMap import are planned.

## Getting started

The project is in the design phase. You can get the repository onto any system, check the data files, and run a small proof of concept.

### 1. Install Git

| System | Command |
|---|---|
| Debian / Ubuntu | `sudo apt update && sudo apt install -y git` |
| Fedora | `sudo dnf install -y git` |
| Arch | `sudo pacman -S git` |
| macOS | `brew install git` (or `xcode-select --install`) |
| Windows (PowerShell) | `winget install --id Git.Git -e` |

### 2. Get the code

```sh
git clone https://github.com/anttiraisala/daily-grind-city-simulation.git
cd daily-grind-city-simulation
```

With the GitHub CLI instead: `gh repo clone anttiraisala/daily-grind-city-simulation`.

Update an existing copy later with `git pull`.

Without Git (Linux, macOS, or Windows 10+ with `curl` and `tar`):

```sh
curl -L -o daily-grind-city-simulation.tar.gz https://github.com/anttiraisala/daily-grind-city-simulation/archive/refs/heads/master.tar.gz
tar -xzf daily-grind-city-simulation.tar.gz
cd daily-grind-city-simulation-master
```

### 3. Check the example data files

Needs Python 3 (any recent version).

Linux and macOS:

```sh
python3 -m json.tool schema/scenario.example.json > /dev/null && echo "scenario OK"
python3 -m json.tool schema/map.example.json > /dev/null && echo "map OK"
```

Windows (PowerShell):

```powershell
python -m json.tool schema\scenario.example.json > $null; if ($?) { "scenario OK" }
python -m json.tool schema\map.example.json > $null; if ($?) { "map OK" }
```

### 4. Install Rust (for the proof of concept)

You need a C toolchain (the Rust linker uses it) and Rust itself. Pick your system.

**Debian / Ubuntu**

```sh
sudo apt update
sudo apt install -y build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Fedora**

```sh
sudo dnf install -y gcc curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Arch**

```sh
sudo pacman -S --needed base-devel rustup
rustup default stable
```

**macOS**

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Windows (PowerShell, not tested by the maintainer yet)**

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id Rustlang.Rustup -e
```

The first command installs the Microsoft C++ build tools (several GB, takes a while). Prefer native Windows over WSL: a phone cannot reach a server inside WSL without extra port forwarding.

The rustup installer asks a question: press `1` and Enter for the default installation. Afterwards **open a new terminal window** (or run `source "$HOME/.cargo/env"` on Linux and macOS), then check:

```sh
rustc --version
cargo --version
```

Both must print a version number. If you see "command not found", the terminal has not picked up the new PATH: open a new one.

### 5. Build, test and run the proof of concept

All commands below run in the `poc` folder of the repository:

```sh
cd poc
```

**5a. Run the tests** (builds the program on the first run, takes under a minute):

```sh
cargo test --release
```

Expected: `test result: ok. 10 passed`.

**5b. Check determinism** (same seed must give the same run):

```sh
cargo run --release -- check --agents 20000
```

Expected last line: `PASS: same seed gives the same run, a different seed gives a different run`.

**5c. Start the live simulation, viewer on this computer only:**

```sh
cargo run --release -- serve
```

The program prints something like:

```
Viewer (this computer): http://localhost:8080/?t=AbCdEf123456
The token in the URL is required for all API calls. Press Ctrl+C to stop.
```

Copy the whole address, including `?t=...`, into a browser on the same computer. Without the token the viewer shows an error. The token changes on every start; to keep it fixed use `--token mysecret`.

What you should see: a map of intersections and moving dots. The clock starts at 00:00 and runs 600 times faster than real time, so the morning rush hour (07:00) arrives after about 42 seconds. Use the speed buttons for faster or slower, **Pause** to freeze, drag to pan, scroll to zoom, click a dot to inspect a resident, **Follow** to track them, and **Slow-traffic heat** to colour streets by how slowly cars moved.

Stop it with `Ctrl+C` in the terminal.

**5d. Watch from a phone on the same Wi-Fi:**

```sh
cargo run --release -- serve --lan
```

It prints a second address, `Viewer (same network, e.g. phone): http://192.168.x.x:8080/?t=...`. Type that address into the phone's browser. Use `--lan` only on a network you trust (home Wi-Fi, not a cafe): the connection is unencrypted and the token is the only protection.

If the printed address looks wrong, find your computer's address yourself:

| System | Command |
|---|---|
| Linux | `hostname -I` |
| macOS | `ipconfig getifaddr en0` |
| Windows | `ipconfig` (look for "IPv4 Address") |

**5e. Useful options** (combine freely):

```sh
cargo run --release -- serve --lan --agents 300000     # bigger city (slower, more traffic)
cargo run --release -- serve --agents 20000 --speed 60 # small city, slower clock
cargo run --release -- serve --port 9000               # another port
cargo run --release -- serve --seed 7                  # different random city and people
cargo run --release -- bench --agents 100000           # headless speed test, no browser
cargo run --release -- help                            # all options
```

After the first build you can also run the compiled program directly: `./target/release/city-poc serve --lan` (on Windows `.\target\release\city-poc.exe serve --lan`).

**5f. Update later:**

```sh
git pull
```

### 6. Troubleshooting

| Symptom | Cause and fix |
|---|---|
| `cargo: command not found` | Open a new terminal, or run `source "$HOME/.cargo/env"`. |
| `linker 'cc' not found` (Linux/macOS) | Install the C toolchain from step 4 (`build-essential`, `gcc`, `xcode-select --install`). |
| `link.exe not found` (Windows) | Install the Visual Studio Build Tools from step 4 and reopen PowerShell. |
| `Address already in use` | Something else uses port 8080. Add `--port 8090`. |
| Browser shows "missing or wrong token" | Open the full address from the terminal, including `?t=...`. The token changes on every start. |
| Phone cannot connect | The phone and computer must be on the same Wi-Fi, and the server must run with `--lan`. Guest networks and some routers isolate devices from each other. Turn off VPNs. Check the computer's firewall (below). |
| Page is blank or dots are missing | Zoom out or reload. At 00:00 nobody has left home yet: raise the speed or wait until about 06:30. |
| Simulation feels slow | Use fewer residents: `--agents 20000`. The clock shows the real speed. |

Firewall, only if the phone still cannot connect:

- Ubuntu with ufw: `sudo ufw allow 8080/tcp` (remove later with `sudo ufw delete allow 8080/tcp`).
- Fedora: `sudo firewall-cmd --add-port=8080/tcp` (temporary until reboot).
- macOS: click Allow in the dialog that appears on first start.
- Windows: click Allow (private networks) in the Defender dialog.

More about the demo, its API and its limits: [`poc/README.md`](poc/README.md). The real simulation is not built yet; see `docs/roadmap.md`.

## Repository layout

| Path | Contents |
|---|---|
| `docs/design-plan.md` | Decisions so far, known gaps, roadmap and the numbered ideas backlog |
| `docs/roadmap.md` | Phased roadmap with task-level status |
| `docs/progress-log.md` | Dated log of what was done, decided and found |
| `docs/notes.md` | Informal thoughts, principles, hunches and open questions |
| `schema/types.ts` | Core data structures (draft) |
| `schema/scenario.example.json` | Example scenario: population, traits, schedules, events |
| `schema/map.example.json` | Example map: roads, traffic lights, buildings |
| `crates/` | Rust workspace for the real simulation: `core` (headless), `cli` (runner), `server` (viewer link); skeleton only |
| `poc/` | Proof of concept: native Rust simulation, browser viewer, phone over the local network |
| `LICENSE` | MIT license |

## Branching

The default branch is `master`.

## Language

All project content (code, comments, docs, UI text, commit messages) is written in English.

## License

Released under the [MIT License](LICENSE).

Map data imported from OpenStreetMap is not covered by this license: it is licensed under the Open Database License (ODbL) and requires the attribution "© OpenStreetMap contributors". Keep such data clearly separated from the project's own files.
