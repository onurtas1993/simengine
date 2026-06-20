# SimEngine

SimEngine is a plugin-based simulation runtime.

This repository is packaged as a single crate named `simengine`.
It includes:

- CLI/runtime
- Manifest/config validation
- Plugin ABI/API
- Network abstraction

Install after publishing:

```bash
cargo install simengine
```

Run:

```bash
simengine run simconfig.json
```

Validate config:

```bash
simengine check simconfig.json
```

When `framework.max_frames` is omitted from `simconfig.json`, the runtime runs indefinitely.
