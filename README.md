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

| System | Commands |
|---|---|
| Debian / Ubuntu | `sudo apt install -y build-essential curl` then the rustup line below |
| Fedora | `sudo dnf install -y gcc curl` then the rustup line below |
| Arch | `sudo pacman -S base-devel rustup && rustup default stable` |
| macOS | `xcode-select --install` then the rustup line below |
| Windows (PowerShell) | `winget install Rustlang.Rustup` (rustup tells you if the Microsoft C++ Build Tools are missing and where to get them) |

The rustup line for Linux and macOS (then open a new terminal):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Check with `cargo --version`.

### 5. Build and run the proof of concept

```sh
cd poc
cargo run --release -- serve --lan
```

The program prints two addresses with an access token. Open the first one in a browser on this computer. Open the "same network" one on a phone connected to the same Wi-Fi. Leave out `--lan` to make the viewer reachable from this computer only; use `--lan` only on a network you trust. Windows may ask whether to allow network access: allow it for private networks.

More commands (`bench`, `check`, tests) and what the demo does and does not show are in [`poc/README.md`](poc/README.md). The real simulation is not built yet; see `docs/roadmap.md`.

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
| `poc/` | Proof of concept: native Rust simulation, browser viewer, phone over the local network |
| `LICENSE` | MIT license |

## Branching

The default branch is `master`.

## Language

All project content (code, comments, docs, UI text, commit messages) is written in English.

## License

Released under the [MIT License](LICENSE).

Map data imported from OpenStreetMap is not covered by this license: it is licensed under the Open Database License (ODbL) and requires the attribution "© OpenStreetMap contributors". Keep such data clearly separated from the project's own files.
