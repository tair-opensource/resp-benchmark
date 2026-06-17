# resp-benchmark

<p align="center">
  <strong>A high-performance benchmark tool for Redis, Valkey, Tair, and any RESP-compatible database</strong>
</p>

<p align="center">
  Built with Rust for maximum throughput &bull; Python bindings for ease of use
</p>

<p align="center">
  <a href="https://pypi.org/project/resp-benchmark/">
    <img src="https://img.shields.io/pypi/v/resp-benchmark?color=%231772b4&label=PyPI&logo=pypi" alt="PyPI Version">
  </a>
  <a href="https://pypi.org/project/resp-benchmark/">
    <img src="https://img.shields.io/pypi/dw/resp-benchmark?color=%231ba784&label=Downloads&logo=pypi" alt="PyPI Downloads">
  </a>
  <a href="https://github.com/tair-opensource/resp-benchmark/blob/main/LICENSE">
    <img src="https://img.shields.io/github/license/tair-opensource/resp-benchmark?color=blue" alt="License">
  </a>
  <a href="https://github.com/tair-opensource/resp-benchmark/stargazers">
    <img src="https://img.shields.io/github/stars/tair-opensource/resp-benchmark?style=social" alt="GitHub Stars">
  </a>
</p>

<p align="center">
  English | <a href="README_zh.md">中文</a>
</p>

---

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Command Syntax](#command-syntax)
- [Lua Script Support](#lua-script-support)
- [Command Line Options](#command-line-options)
- [Advanced Features](#advanced-features)
- [Performance Tips](#performance-tips)
- [Examples](#examples)
- [Python Library API](#python-library-api)
- [Contributing](#contributing)
- [License](#license)

## Installation

```bash
pip install resp-benchmark
```

Requires Python 3.9+. Prebuilt wheels are available for macOS and Linux.

## Quick Start

### Command Line

```bash
# Basic benchmark: SET random keys with 64-byte values for 10 seconds
resp-benchmark -s 10 "SET {key uniform 100000} {value 64}"

# Load 1M key-value pairs, then benchmark reads
resp-benchmark --load -n 1000000 "SET {key sequence 100000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 100000}"

# 128 connections, pipeline depth 10, run 30 seconds
resp-benchmark -c 128 -P 10 -s 30 "SET {key uniform 1000000} {value 128}"

# Using Lua script inline
resp-benchmark --lua -s 10 "local key = bench.key(10000, 'uniform', 'test'); function generate() return {'SET', key(), bench.value(64)()} end"

# Using Lua script file
resp-benchmark --lua-file -s 10 workloads/hello.lua
```

### Python Library

```python
from resp_benchmark import Benchmark

bm = Benchmark(host="127.0.0.1", port=6379)

# Load test data
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128
)

# Run benchmark
result = bm.bench(
    command="GET {key uniform 1000000}",
    seconds=30,
    connections=64
)

print(f"QPS: {result.qps}")
print(f"Avg Latency: {result.avg_latency_ms}ms")
print(f"P99 Latency: {result.p99_latency_ms}ms")
```

## Command Syntax

resp-benchmark uses a placeholder system to generate varied, realistic test data. Placeholders are enclosed in `{}` within the command string.

### Key Placeholders

| Placeholder | Description | Example |
|---|---|---|
| `{key uniform N}` | Random key from `key_0000000000` to `key_{N-1}`, each equally likely | `{key uniform 100000}` → `key_0000042371` |
| `{key sequence N}` | Sequential key from 0 to N-1, wrapping around. Ideal for loading data because each key is written exactly once before repeating. | `{key sequence 100000}` → `key_0000000000`, `key_0000000001`, ... |
| `{key zipfian N}` | Zipfian distribution (exponent 1.03): a small fraction of keys receive most requests, simulating real-world cache access patterns. | `{key zipfian 100000}` → frequently `key_0000000001`, rarely `key_0000099999` |

### Value Placeholders

| Placeholder | Description | Example |
|---|---|---|
| `{value N}` | Random alphanumeric string of N bytes, different on every request | `{value 64}` → `a8x9mK2pQ7...` (64 bytes) |
| `{rand N}` | Random integer from 0 to N-1 | `{rand 1000}` → `742` |
| `{range N W}` | Two integers: a random start in [0, N-1] and start+W (clamped to N-1). Useful for range queries. | `{range 1000 10}` → `45 55` |

### Example Commands

```bash
# String operations
SET {key uniform 1000000} {value 64}
GET {key uniform 1000000}
INCR {key uniform 100000}

# List operations
LPUSH {key uniform 1000} {value 64}
LINDEX {key uniform 1000} {rand 100}

# Set operations
SADD {key uniform 1000} {value 64}
SISMEMBER {key uniform 1000} {value 64}

# Sorted Set operations
ZADD {key uniform 1000} {rand 1000} {value 64}
ZRANGEBYSCORE {key uniform 1000} {range 1000 100}

# Hash operations
HSET {key uniform 1000} {key uniform 100} {value 64}
HGET {key uniform 1000} {key uniform 100}
```

## Lua Script Support

For commands that require dynamic logic (conditional branching, computed fields, JSON payloads), use Lua scripts instead of simple placeholders.

### Lua API

The global `bench` object provides generator functions:

**`bench.key(range, distribution, name)`** — Creates a key generator. `distribution` is `"uniform"`, `"sequence"`, or `"zipfian"`. `name` identifies the generator; generators with the same name share state across threads, ensuring sequence generators don't produce duplicates.

**`bench.value(size)`** — Creates a generator that produces a random alphanumeric string of `size` bytes on each call.

**`bench.rand(range)`** — Creates a generator that returns a random integer in [0, range-1] on each call.

**`json.encode(data)`** — Converts a Lua table to a JSON string.

### Script Structure

Every Lua script must define a global `generate` function that returns an array of strings representing a Redis command:

```lua
function generate()
    return { "COMMAND", "arg1", "arg2", ... }
end
```

### Examples

#### Basic SET

```lua
local key_gen = bench.key(10000, "uniform", "my_key")
local value_gen = bench.value(64)
function generate()
    return { "SET", key_gen(), value_gen() }
end
```

#### Mixed Read/Write (50/50)

```lua
local key_gen = bench.key(10000, "uniform", "cond_key")
local value_gen = bench.value(64)
local rand_gen = bench.rand(100)
function generate()
    local key = key_gen()
    local value = value_gen()
    local num = rand_gen()

    if num < 50 then
        return { "SET", key, value }
    else
        return { "GET", key }
    end
end
```

#### Hash Operations

```lua
local key_gen = bench.key(1000, "uniform", "hash_key")
local field_gen = bench.key(100, "zipfian", "hash_field")
local value_gen = bench.value(32)
function generate()
    return { "HSET", key_gen(), field_gen(), value_gen() }
end
```

#### JSON Payloads

```lua
local key_gen = bench.key(1000, "uniform", "json_key")
local id_rand = bench.rand(10000)
local name_rand = bench.rand(1000)
local score_rand = bench.rand(100)
function generate()
    local data = {
        id = id_rand(),
        name = "user_" .. name_rand(),
        score = score_rand()
    }
    local json_str = json.encode(data)
    return { "SET", key_gen(), json_str }
end
```

## Command Line Options

| Option | Description | Default |
|---|---|---|
| `-h` | Server hostname | 127.0.0.1 |
| `-p` | Server port | 6379 |
| `-u` | Username for ACL authentication | "" |
| `-a` | Password for authentication | "" |
| `-c` | Number of connections (0 = auto-scaling) | 0 |
| `-n` | Total number of requests (0 = unlimited) | 0 |
| `-s` | Duration in seconds (0 = unlimited) | 0 |
| `-t` | Target QPS (0 = unlimited) | 0 |
| `-P` | Pipeline depth | 1 |
| `--cores` | CPU cores to use (comma-separated, e.g. `0,1,2,3`) | all |
| `--cluster` | Enable Redis cluster mode | false |
| `--load` | Load data only, skip benchmarking | false |
| `--short-connection` | Create a new connection for each command | false |
| `--lua` | Treat the command string as a Lua script | false |
| `--lua-file` | Treat the command string as a path to a Lua script file | false |
| `-q` | Quiet mode: suppress progress output | false |

## Advanced Features

### Connection Auto-scaling

When `-c 0` (the default), resp-benchmark starts with a small number of connections and progressively doubles them. Once doubling no longer increases throughput (QPS growth < 30%), the connection count is locked in and the real benchmark begins. This finds the optimal connection count automatically.

```bash
resp-benchmark -c 0 -s 30 "GET {key uniform 100000}"
```

### CPU Core Affinity

Pin benchmark threads to specific CPU cores to reduce context-switch jitter and get more stable results:

```bash
resp-benchmark --cores 0,1,2,3 -s 10 "SET {key uniform 100000} {value 64}"
```

### Rate Limiting

Control request rate for gradual load testing. Useful for finding the QPS threshold where latency spikes:

```bash
resp-benchmark -t 10000 -s 30 "SET {key uniform 100000} {value 64}"
```

### Pipeline

Send multiple commands per round-trip to maximize throughput. Higher pipeline depth reduces per-command latency overhead but increases per-batch latency:

```bash
resp-benchmark -P 10 -c 128 -s 30 "SET {key uniform 100000} {value 64}"
```

### Cluster Mode

Automatically distribute requests across all cluster nodes based on key hash slots:

```bash
resp-benchmark --cluster -h cluster-endpoint -p 7000 -s 30 "SET {key uniform 100000} {value 64}"
```

### Short Connection Mode

Create and close a TCP connection for every single command. This measures connection establishment overhead, which is important when benchmarking proxies (like Twemproxy, Codis) or connection pooling layers.

Limitations: not compatible with `--load`; pipeline must be 1.

```bash
resp-benchmark --short-connection -c 50 -s 10 "PING"
```

## Performance Tips

1. **Pre-load data**: Use `--load` with `sequence` keys to populate test data before read benchmarks, so reads don't hit empty keys.
2. **Start with auto connections**: Let `-c 0` find the optimal count, then hardcode it for reproducible runs.
3. **Match key distribution to your workload**: Use `uniform` for cache scenarios, `zipfian` for realistic production traffic, `sequence` for bulk imports.
4. **Pipeline for throughput, no pipeline for latency**: Use `-P 10` when measuring maximum throughput; use `-P 1` when measuring per-command latency.
5. **Clean state between tests**: Run `FLUSHALL` between test runs to avoid interference.

### Typical Workflow

```bash
# 1. Clear existing data
redis-cli FLUSHALL

# 2. Load 1M key-value pairs
resp-benchmark --load -c 256 -P 10 -n 1000000 "SET {key sequence 1000000} {value 64}"

# 3. Benchmark different access patterns
resp-benchmark -c 128 -s 30 "GET {key uniform 1000000}"    # Random access
resp-benchmark -c 128 -s 30 "GET {key zipfian 1000000}"    # Hot-key access
```

## Examples

### String Operations

```bash
resp-benchmark --load -n 1000000 "SET {key sequence 1000000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 1000000}"
resp-benchmark -s 10 "SET {key uniform 10000} {value 1024}"
resp-benchmark -s 10 "INCR {key uniform 10000}"
```

### List Operations

```bash
resp-benchmark --load -n 1000000 "LPUSH {key sequence 1000} {value 64}"
resp-benchmark -s 10 "LINDEX {key uniform 1000} {rand 1000}"
resp-benchmark -s 10 "LRANGE {key uniform 1000} {range 1000 10}"
```

### Set Operations

```bash
resp-benchmark --load -n 1000000 "SADD {key sequence 1000} {key sequence 1000}"
resp-benchmark -s 10 "SISMEMBER {key uniform 1000} {key uniform 1000}"
```

### Sorted Set Operations

```bash
resp-benchmark --load -n 1000000 "ZADD {key sequence 1000} {rand 10000} {key sequence 1000}"
resp-benchmark -s 10 "ZSCORE {key uniform 1000} {key uniform 1000}"
resp-benchmark -s 10 "ZRANGEBYSCORE {key uniform 1000} {range 10000 100}"
```

### Hash Operations

```bash
resp-benchmark --load -n 1000000 "HSET {key sequence 1000} {key sequence 100} {value 64}"
resp-benchmark -s 10 "HGET {key uniform 1000} {key uniform 100}"
```

### EVALSHA

```bash
redis-cli SCRIPT LOAD "return redis.call('SET', KEYS[1], ARGV[1])"
resp-benchmark -s 10 "EVALSHA d8f2fad9f8e86a53d2a6ebd960b33c4972cacc37 1 {key uniform 100000} {value 64}"
```

## Python Library API

### Initialization

```python
from resp_benchmark import Benchmark

# Basic connection
bm = Benchmark(host="127.0.0.1", port=6379)

# With authentication
bm = Benchmark(host="redis.example.com", port=6379, username="user", password="pass")

# Cluster mode
bm = Benchmark(host="cluster-endpoint", port=7000, cluster=True)

# Pin to specific CPU cores
bm = Benchmark(host="127.0.0.1", port=6379, cores="0,1,2,3")
```

### Loading Data

```python
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128,
    pipeline=10
)
```

### Benchmarking

```python
# Time-based
result = bm.bench(command="GET {key uniform 1000000}", seconds=30, connections=64)

# Count-based
result = bm.bench(command="GET {key uniform 1000000}", count=1000000, connections=64)

# With pipeline
result = bm.bench(command="SET {key uniform 1000000} {value 64}", seconds=30, connections=64, pipeline=10)

# Short connection
result = bm.bench(command="PING", seconds=30, connections=50, short_connection=True)

# Lua script
lua_script = """
local user_id = bench.key(10000, "uniform", "user_id")
local data = bench.value(64)
function generate()
    return { "SET", user_id(), data() }
end
"""
result = bm.bench(command=lua_script, seconds=30, connections=64, use_lua=True)
```

### Result Analysis

```python
print(f"QPS: {result.qps:.2f}")
print(f"Avg Latency: {result.avg_latency_ms:.2f}ms")
print(f"P99 Latency: {result.p99_latency_ms:.2f}ms")
print(f"Connections: {result.connections}")
```

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
