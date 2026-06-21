# SimEngine

SimEngine is a plugin-based simulation runtime.

This repository is packaged as a single crate named `simengine`.
It includes:

- CLI/runtime
- Manifest/config validation
- Plugin ABI/API
- Network abstraction

Install:

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

When `framework.max_frames` is omitted from `simconfig.json`, the simulation engine runs indefinitely.

Below is an example of 2 simulations:
1) a simulation which checks the current cpu usage and publishes the value.
2) another simulation which takes the cpu usage as parameter and outputs an alert if the usage is above 80%.

These 2 simulations are running on 2 difference machines in the same network at 1 fps speed. Machine 1 runs `system-monitor` and Machine 2 runs `cpu-alert`.

Machine 1:
```json
{
  "framework": {
    "fps": 1
  },
  "simulations": [
    {
      "name": "system-monitor",
      "endpoint": "127.0.0.1:7001",
      "plugin": "system_monitor.dll",
      "inputs": [],
      "outputs": [
        { "name": "cpu_usage", "type": "int32" },
        { "name": "memory_usage", "type": "int32" }
      ]
    }
  ],
  "links": [
    {
      "from": {
        "endpoint": "127.0.0.1:7001",
        "output": "cpu_usage"
      },
      "to": {
        "endpoint": "127.0.0.2:7001",
        "input": "cpu_usage"
      }
    }
  ]
}

```

Machine 2:
```json
{
  "framework": {
    "fps": 1
  },
  "simulations": [
    {
      "name": "cpu-alert",
      "endpoint": "127.0.0.2:7001",
      "plugin": "cpu_alert.dll",
      "inputs": [
        { "name": "cpu_usage", "type": "int32" }
      ],
      "outputs": [
        { "name": "alert_active", "type": "int32" }
      ]
    }
  ],
  "links": [
    {
      "from": {
        "endpoint": "127.0.0.1:7001",
        "output": "cpu_usage"
      },
      "to": {
        "endpoint": "127.0.0.2:7001",
        "input": "cpu_usage"
      }
    }
  ]
}

```
