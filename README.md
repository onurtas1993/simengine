# SimEngine

SimEngine is a headless simulation runtime. The executable loads simulation DLLs, runs them at a configured FPS, and lets simulations exchange primitive input/output values.

## Debug workflow on Windows

From the repository root:

```powershell
cargo build
cargo run -p simengine -- run configs\basic.local.json
```

The default config runs at `fps = 2` so you can clearly see the frame timing. Change this value in:

```text
configs\basic.local.json
```

Example:

```json
"framework": {
  "fps": 10,
  "log_level": "info",
  "max_frames": 10
}
```

## What the example does

`basic-sim` produces an output variable:

```text
counter: int32
```

`basic-sim2` consumes that value as an input:

```text
counter <- basic-sim.counter
```

Then `basic-sim2` produces:

```text
double_counter: int32
```

The simulation code uses clean names, not topics:

```rust
self.ctx.set_output_i32("counter", self.counter);

if let Some(counter) = self.ctx.get_input_i32("counter") {
    self.ctx.set_output_i32("double_counter", counter * 2);
}
```

The runner internally stores fully qualified variable names like:

```text
basic-sim.counter
basic-sim2.double_counter
```

## Important paths

```text
crates/simengine                  runner executable
crates/simengine-core             manifest/config validation
crates/simengine-plugin-api       clean plugin API + hidden ABI glue
examples/basic-sim                first simulation DLL
examples/basic-sim2               second simulation DLL
configs/basic.local.json          local debug manifest
```

## Useful commands

```powershell
cargo check
cargo build
cargo run -p simengine -- check configs\basic.local.json
cargo run -p simengine -- run configs\basic.local.json
```
