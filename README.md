<table>
  <tr>
    <td>

<img src="icon.ico" width="128"/>
    </td>
    <td>

# SimEngine

SimEngine is a plugin-based simulation runtime for composing independent simulations through typed inputs, outputs, routes, and runtime state transitions.
    </td>
  </tr>
</table>

This repository is packaged as a single Rust crate named `simengine`. It includes:

- CLI/runtime
- Manifest/config validation
- Plugin ABI/API
- TCP routing between simulations
- Engine and simulation state management


## Install

```bash
cargo install simengine
```

## Commands

Run a manifest:

```bash
simengine run simconfig.json
```

Validate a manifest without running it:

```bash
simengine check simconfig.json
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

These states are built into the engine. They are not declared or customized in `simconfig.json`.

The engine starts in `START`. Each simulation starts in `_START`. During `Simulation::create`, a plugin should report whether it initialized successfully:

```rust
ctx.set_state("_READY");
```

or:

```rust
ctx.set_state("_ERROR");
```

The engine evaluates `state_transitions` after simulations are loaded and during the run loop. `pre_step`, `step`, and `post_step` are called only while the engine state is `RUNNING`. `ERROR` and `FINISHED` are terminal runtime states.

## Manifest format - from a working environment

Machine 1:

```json
{
  "framework": {
    "fps": 60
  },
  "simulations": [
    {
      "name": "xplane-health-check",
      "endpoint": "127.0.0.1:7000",
      "plugin": "xplane_health_check.dll",
      "inputs": [],
      "outputs": []
    },
    {
      "name": "xplane-datarefs-reader",
      "endpoint": "127.0.0.1:7001",
      "plugin": "xplane_datarefs_reader.dll",
      "inputs": [],
      "outputs": [
        { "name": "altitude_ft", "type": "float32" },
        { "name": "heading_deg", "type": "float32" },
        { "name": "roll_deg", "type": "float32" },
        { "name": "pitch_deg", "type": "float32" },
        { "name": "airspeed_kt", "type": "float32" },
        { "name": "vertical_speed_fpm", "type": "float32" }
      ]
    },
    {
      "name": "xplane-datarefs-sender",
      "endpoint": "127.0.0.1:7002",
      "plugin": "xplane_datarefs_sender.dll",
      "inputs": [
        { "name": "yoke_pitch_ratio", "type": "float32" },
        { "name": "yoke_roll_ratio", "type": "float32" },
        { "name": "throttle_ratio", "type": "float32" }
      ],
      "outputs": []
    }
  ],
  "routes": [
    {
      "from": { "endpoint": "127.0.0.1:7001", "output": "altitude_ft" },
      "to": { "endpoint": "127.0.0.2:7001", "input": "altitude_ft" }
    },
    {
      "from": { "endpoint": "127.0.0.1:7001", "output": "heading_deg" },
      "to": { "endpoint": "127.0.0.2:7001", "input": "heading_deg" }
    },
    {
      "from": { "endpoint": "127.0.0.1:7001", "output": "roll_deg" },
      "to": { "endpoint": "127.0.0.2:7001", "input": "roll_deg" }
    },
    {
      "from": { "endpoint": "127.0.0.1:7001", "output": "pitch_deg" },
      "to": { "endpoint": "127.0.0.2:7001", "input": "pitch_deg" }
    },
    {
      "from": { "endpoint": "127.0.0.1:7001", "output": "airspeed_kt" },
      "to": { "endpoint": "127.0.0.2:7001", "input": "airspeed_kt" }
    },
    {
      "from": { "endpoint": "127.0.0.1:7001", "output": "vertical_speed_fpm" },
      "to": { "endpoint": "127.0.0.2:7001", "input": "vertical_speed_fpm" }
    }
  ],
  "state_transitions": [
    {
      "when": {
        "all": [
          { "sim": "xplane-health-check", "state": "_READY" },
          { "sim": "xplane-datarefs-reader", "state": "_READY" },
          { "sim": "xplane-datarefs-sender", "state": "_READY" }
        ]
      },
      "engine": "RUNNING"
    },
    {
      "when": {
        "any": [
          { "sim": "xplane-health-check", "state": "_ERROR" },
          { "sim": "xplane-datarefs-reader", "state": "_ERROR" },
          { "sim": "xplane-datarefs-sender", "state": "_ERROR" }
        ]
      },
      "engine": "ERROR"
    }
  ]
}
```

Machine 2:

```json
{
  "framework": {
    "fps": 60
  },
  "simulations": [
    {
      "name": "basic-autopilot",
      "endpoint": "127.0.0.2:7001",
      "plugin": "basic_autopilot.dll",
      "inputs": [
        { "name": "altitude_ft", "type": "float32" },
        { "name": "heading_deg", "type": "float32" },
        { "name": "roll_deg", "type": "float32" },
        { "name": "pitch_deg", "type": "float32" },
        { "name": "airspeed_kt", "type": "float32" },
        { "name": "vertical_speed_fpm", "type": "float32" }
      ],
      "outputs": [
        { "name": "yoke_pitch_ratio", "type": "float32" },
        { "name": "yoke_roll_ratio", "type": "float32" },
        { "name": "throttle_ratio", "type": "float32" }
      ]
    }
  ],
  "routes": [
    {
      "from": { "endpoint": "127.0.0.2:7001", "output": "yoke_pitch_ratio" },
      "to": { "endpoint": "127.0.0.1:7002", "input": "yoke_pitch_ratio" }
    },
    {
      "from": { "endpoint": "127.0.0.2:7001", "output": "yoke_roll_ratio" },
      "to": { "endpoint": "127.0.0.1:7002", "input": "yoke_roll_ratio" }
    },
    {
      "from": { "endpoint": "127.0.0.2:7001", "output": "throttle_ratio" },
      "to": { "endpoint": "127.0.0.1:7002", "input": "throttle_ratio" }
    }
  ],
  "state_transitions": [
    {
      "when": {
        "all": [
          { "sim": "basic-autopilot", "state": "_READY" }
        ]
      },
      "engine": "RUNNING"
    },
    {
      "when": {
        "any": [
          { "sim": "basic-autopilot", "state": "_ERROR" }
        ]
      },
      "engine": "ERROR"
    }
  ]
}
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

## Distributed Routing

Routes can target simulations in the same engine process or endpoints hosted by another `simengine` process.

Local route:

```json
{
  "from": { "endpoint": "127.0.0.1:7001", "output": "value" },
  "to": { "endpoint": "127.0.0.1:7002", "input": "value" }
}
```

Remote route:

```json
{
  "from": { "endpoint": "127.0.0.1:7001", "output": "value" },
  "to": { "endpoint": "127.0.0.2:7001", "input": "value" }
}
```

Route sources must be local to the manifest. Route targets may be local or remote. If a route target is local, validation checks that the input exists and that the types match.
