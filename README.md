<table>
  <tr>
    <td>

<img src="https://raw.githubusercontent.com/onurtas1993/images/refs/heads/main/icon.ico" width="100%"/>
    </td>
    <td>

# SimEngine

SimEngine is a plugin-based Rust simulation engine for composing independent simulations('plugins') with typed instruments, data flows, and state-driven execution.
    </td>
  </tr>
</table>

[![Test Status](https://github.com/onurtas1993/simengine/actions/workflows/ci.yml/badge.svg)](https://github.com/onurtas1993/simengine/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/simengine.svg)](https://crates.io/crates/simengine)
[![API](https://docs.rs/simengine/badge.svg)](https://docs.rs/simengine)
[![Book](https://img.shields.io/badge/book-online-blue.svg)](https://<username>.github.io/simengine/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2024-orange.svg)](https://www.rust-lang.org/)

This repository is packaged as a single Rust crate named `simengine`. It includes:

- CLI/runtime
- Manifest, instrument, and flow validation
- Plugin ABI/API
- TCP instrument flow between simulations
- Engine and simulation state management


## Install

```bash
cargo install simengine
```

## Commands

Run a manifest:

```bash
simengine run config.json
```

Validate a manifest without running it:

```bash
simengine check config.json
```

When `framework.max_frames` is omitted, the engine runs indefinitely unless it enters a terminal state.

## Runtime States

SimEngine has fixed engine states:

```text
START
READY
RUNNING
FINISHED
ERROR
```

Each simulation has fixed internal states:

```text
_START
_READY
_RUNNING
_FINISHED
_ERROR
```

These states are built into the engine. They are not declared or customized in `config.json`.

The engine starts in `START`. Each simulation starts in `_START`. During `Simulation::create`, a plugin should report whether it initialized successfully:

```rust
ctx.set_state("_READY");
```

or:

```rust
ctx.set_state("_ERROR");
```

The engine evaluates `state_transitions` after simulations are loaded and during the run loop. `pre_step`, `step`, and `post_step` are called only while the engine state is `RUNNING`. `ERROR` and `FINISHED` are terminal runtime states.

## Minimum One-Machine Example

SimEngine loads three files from the same directory:

```text
config.json
instruments.json
instrument_flows.json
```

This example runs two simulations on one machine:

- `temperature-reader` produces `temperature_c`
- `fan-controller` consumes `temperature_c`

`config.json` declares the runtime, simulations, plugins, and state transitions:

```json
{
  "framework": {
    "fps": 60
  },
  "simulations": [
    {
      "name": "temperature-reader",
      "endpoint": "127.0.0.1:7001",
      "plugin": "temperature_reader.dll"
    },
    {
      "name": "fan-controller",
      "endpoint": "127.0.0.1:7002",
      "plugin": "fan_controller.dll"
    }
  ],
  "state_transitions": [
    {
      "when": {
        "all": [
          { "sim": "temperature-reader", "state": "_READY" },
          { "sim": "fan-controller", "state": "_READY" }
        ]
      },
      "engine": "RUNNING"
    },
    {
      "when": {
        "any": [
          { "sim": "temperature-reader", "state": "_ERROR" },
          { "sim": "fan-controller", "state": "_ERROR" }
        ]
      },
      "engine": "ERROR"
    }
  ]
}
```

`instruments.json` declares typed values once:

```json
[
  {
    "value": "temperature_c",
    "type": "float32"
  }
]
```

`instrument_flows.json` declares where instrument values move:

```json
[
  {
    "instrument": "temperature_c",
    "from": "127.0.0.1:7001",
    "to": "127.0.0.1:7002"
  }
]
```

Currently supported primitive types:

```text
float32
```

## State Transitions

`state_transitions` is a top-level manifest field. Each rule has:

- `when`: a condition using either `all` or `any`
- `engine`: the engine state to enter when the condition matches

Example:

```json
{
  "when": {
    "all": [
      { "sim": "producer", "state": "_READY" },
      { "sim": "consumer", "state": "_READY" }
    ]
  },
  "engine": "RUNNING"
}
```

Validation rejects:

- unknown simulation names in state transitions
- empty transition conditions
- transition conditions that use both `all` and `any`
- unknown engine or simulation states

State transitions are one-way orchestration rules:

```text
simulation states -> engine state
```

The engine uses its state to decide whether lifecycle methods should run. It does not normally force simulation states.

## Plugin API v2

SimEngine plugin ABI version is currently:

```rust
pub const SIMENGINE_API_VERSION: u32 = 2;
```

Plugins implement `Simulation` and are exported with `simengine_plugin!`.

Example simulation: XPlane DataRef Sender

```rust
use simengine::plugin_api::{Simulation, SimulationContext};
use simengine::simengine_plugin;

use std::net::{SocketAddr, UdpSocket};

const DREF_HEADER: &[u8; 5] = b"DREF\0";
const DATAREF_NAME_LEN: usize = 500;

const XPLANE_ADDR: &str = "127.0.0.1:49000";
const SENDER_BIND_ADDR: &str = "0.0.0.0:49006";

struct WritableDatarefSpec {
    input_name: &'static str,
    dataref: &'static str,
    min_value: f32,
    max_value: f32,
}

struct XplaneDatarefsSender {
    ctx: SimulationContext,
    socket: UdpSocket,
    xplane_addr: SocketAddr,
    has_entered_running: bool,
}

impl XplaneDatarefsSender {
    fn writable_datarefs() -> Vec<WritableDatarefSpec> {
        vec![
            WritableDatarefSpec {
                input_name: "yoke_pitch_ratio",
                dataref: "sim/cockpit2/controls/yoke_pitch_ratio",
                min_value: -1.0,
                max_value: 1.0,
            },
            WritableDatarefSpec {
                input_name: "yoke_roll_ratio",
                dataref: "sim/cockpit2/controls/yoke_roll_ratio",
                min_value: -1.0,
                max_value: 1.0,
            },
            WritableDatarefSpec {
                input_name: "throttle_ratio",
                dataref: "sim/cockpit2/engine/actuators/throttle_ratio_all",
                min_value: 0.0,
                max_value: 1.0,
            },
        ]
    }

    fn clamp(value: f32, min_value: f32, max_value: f32) -> f32 {
        value.max(min_value).min(max_value)
    }

    fn build_dref_packet(value: f32, dataref: &str) -> Vec<u8> {
        let mut packet = Vec::with_capacity(5 + 4 + DATAREF_NAME_LEN);

        packet.extend_from_slice(DREF_HEADER);
        packet.extend_from_slice(&value.to_le_bytes());

        let mut name = [0u8; DATAREF_NAME_LEN];
        let bytes = dataref.as_bytes();
        let len = bytes.len().min(DATAREF_NAME_LEN - 1);
        name[..len].copy_from_slice(&bytes[..len]);

        packet.extend_from_slice(&name);
        packet
    }

    fn send_dataref(&self, dataref: &str, value: f32) -> bool {
        let packet = Self::build_dref_packet(value, dataref);

        match self.socket.send_to(&packet, self.xplane_addr) {
            Ok(_) => true,
            Err(err) => {
                self.ctx.info(format!(
                    "failed to send DREF {} value={}: {}",
                    dataref, value, err
                ));
                false
            }
        }
    }
}

impl Simulation for XplaneDatarefsSender {
    fn create(ctx: SimulationContext, _config_json: &str) -> Self {
        let xplane_addr: SocketAddr = match XPLANE_ADDR.parse() {
            Ok(addr) => addr,
            Err(err) => {
                ctx.set_state("_ERROR");
                panic!("invalid hardcoded XPLANE_ADDR: {err}");
            }
        };

        let socket = match UdpSocket::bind(SENDER_BIND_ADDR) {
            Ok(socket) => socket,
            Err(err) => {
                ctx.set_state("_ERROR");
                panic!("failed to bind X-Plane DREF sender UDP socket: {err}");
            }
        };

        ctx.info(format!(
            "xplane-datarefs-sender created: bind={} xplane={}",
            SENDER_BIND_ADDR, XPLANE_ADDR
        ));

        ctx.set_state("_READY");

        Self {
            ctx,
            socket,
            xplane_addr,
            has_entered_running: false,
        }
    }

    fn step(&mut self, _dt_seconds: f64) {
        if !self.has_entered_running {
            self.ctx.set_state("_RUNNING");
            self.has_entered_running = true;
        }

        for spec in Self::writable_datarefs() {
            let Some(value) = self.ctx.get_input_f32(spec.input_name) else {
                continue;
            };

            let value = Self::clamp(value, spec.min_value, spec.max_value);

            if !self.send_dataref(spec.dataref, value) {
                self.ctx.set_state("_ERROR");
                return;
            }
        }
    }

    fn destroy(&mut self) {
        self.send_dataref("sim/cockpit2/controls/yoke_pitch_ratio", 0.0);
        self.send_dataref("sim/cockpit2/controls/yoke_roll_ratio", 0.0);
        self.send_dataref("sim/cockpit2/engine/actuators/throttle_ratio_all", 0.0);
        self.ctx.info("xplane-datarefs-sender destroyed");
    }
}

simengine_plugin!(XplaneDatarefsSender);
```

Available context helpers include:

```rust
ctx.info("message");
ctx.set_state("_READY");
ctx.set_output_f32("output_name", value);
ctx.get_input_f32("input_name");
```

## Instrument Flows

Instrument flows can target simulations in the same engine process or endpoints hosted by another `simengine` process.

Local flow:

```json
{
  "instrument": "temperature_c",
  "from": "127.0.0.1:7001",
  "to": "127.0.0.1:7002"
}
```

Remote flow:

```json
{
  "instrument": "temperature_c",
  "from": "127.0.0.1:7001",
  "to": "127.0.0.2:7001"
}
```

Flow sources must be local to `config.json`. Flow targets may be local or remote. The instrument must exist in `instruments.json`, and the same instrument name is used at both ends of the flow.

## Case Study

The best way to learn SimEngine is by exploring a working example. For a complete case study, see the xplane-autopilot project below, which demonstrates a flight simulation built on top of SimEngine.

https://github.com/onurtas1993/xplane-autopilot/releases