# resp-benchmark

[![Python - Version](https://img.shields.io/badge/python-%3E%3D3.8-brightgreen)](https://www.python.org/doc/versions/)
[![PyPI - Version](https://img.shields.io/pypi/v/resp-benchmark?color=%231772b4)](https://pypi.org/project/resp-benchmark/)
[![PyPI - Downloads](https://img.shields.io/pypi/dw/resp-benchmark?color=%231ba784)](https://pypi.org/project/resp-benchmark/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/tair-opensource/resp-benchmark/blob/main/LICENSE)

English | [中文](README.md)

A high-performance benchmark tool for testing databases that support the RESP (Redis Serialization Protocol), built with Rust and Python bindings. Designed to provide accurate and realistic performance measurements for Redis, Valkey, Tair, and other RESP-compatible databases.

## Features

- **High Performance**: Built with Rust core for maximum throughput and minimal overhead
- **Flexible Commands**: Support for custom command templates with intelligent placeholders
- **Realistic Testing**: Generates varied data to simulate real-world usage patterns
- **Multi-threaded**: Leverages multiple CPU cores for maximum performance
- **Connection Management**: Intelligent connection pooling and auto-scaling
- **Rate Limiting**: Built-in QPS throttling for controlled testing
- **Pipeline Support**: Configurable pipeline depth for bulk operations
- **Cluster Support**: Native Redis cluster mode support
- **Python Integration**: Use as both CLI tool and Python library
- **Comprehensive Metrics**: Detailed latency histograms and performance statistics
- **Lua Script Support**: Use Lua scripts to generate complex test commands

## Installation

Requires Python 3.9 or higher.

```bash
pip install resp-benchmark
```

## Quick Start

### Command Line Usage

```bash
# Basic benchmark
resp-benchmark -s 10 "SET {key uniform 100000} {value 64}"

# Load data then benchmark
resp-benchmark --load -n 1000000 "SET {key sequence 100000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 100000}"

# With custom connections and pipeline
resp-benchmark -c 128 -P 10 -s 30 "SET {key uniform 1000000} {value 128}"

# Using Lua script
resp-benchmark --lua -s 10 "local key = bench.key(10000, 'uniform', 'test'); function generate() return {'SET', key(), bench.value(64)()} end"

# Using Lua script file
resp-benchmark --lua-file -s 10 workloads/hello.lua
```

### Python Library Usage

```python
from resp_benchmark import Benchmark

# Initialize benchmark
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

# Using Lua script
lua_script = """
local user_id = bench.key(10000, "uniform", "user_id")
local data = bench.value(64)
function generate()
    key = user_id()
    value = data()
    return { "SET", key, value }
end
"""
result = bm.bench(command=lua_script, seconds=30, connections=64, use_lua=True)
```

## Command Syntax

resp-benchmark uses a powerful placeholder system to generate varied and realistic test data:

### Key Placeholders

- **`{key uniform N}`**: Random key from 0 to N-1
  - Example: `{key uniform 100000}` → `key_0000099999`
  
- **`{key sequence N}`**: Sequential keys from 0 to N-1 (ideal for loading)
  - Example: `{key sequence 100000}` → `key_0000000000`, `key_0000000001`, ...
  
- **`{key zipfian N}`**: Zipfian distribution keys (simulates real-world access patterns)
  - Example: `{key zipfian 100000}` → follows Zipfian distribution with exponent 1.03

### Value Placeholders

- **`{value N}`**: Random string of N bytes
  - Example: `{value 64}` → `a8x9mK2p...` (64 bytes)
  
- **`{rand N}`**: Random number from 0 to N-1
  - Example: `{rand 1000}` → `742`
  
- **`{range N W}`**: Two numbers within range N with difference W
  - Example: `{range 100 10}` → `45 55`

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

resp-benchmark now supports using Lua scripts to generate more complex test commands.

### Lua API

In Lua scripts, you can access the following functions through the global `bench` object:

#### `bench.key(range, distribution, name)`
Creates a key generator:
- `range`: Key range (0 to range-1)
- `distribution`: Distribution type ("uniform", "sequence", "zipfian")
- `name`: Generator name (used to share state between multiple script instances). Generators with the same name will share state.

Returns a callable function that generates the next key each time it's called.

#### `bench.value(size)`
Creates a value generator:
- `size`: Length of the random string to generate (in bytes)

Returns a callable function that generates a random string of the specified length each time it's called.

#### `bench.rand(range)`
Creates a random number generator:
- `range`: Random number range (0 to range-1)

Returns a callable function that generates a random integer each time it's called.

#### `json.encode(data)`
Converts a Lua table to a JSON string.

### Lua Script Requirements

Each Lua script must define a global function named `generate` that takes no parameters and returns an array of strings representing a Redis command.

```lua
function generate()
    -- Command generation logic
    return { "COMMAND", "arg1", "arg2", ... }
end
```

### Lua Examples

#### Basic Example
```lua
local key_gen = bench.key(10000, "uniform", "my_key")
local value_gen = bench.value(64)
function generate()
    return { "SET", key_gen(), value_gen() }
end
```

#### Conditional Logic Example
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

#### Complex Data Structure Example
```lua
local key_gen = bench.key(1000, "uniform", "hash_key")
local field_gen = bench.key(100, "zipfian", "hash_field")
local value_gen = bench.value(32)
function generate()
    local key = key_gen()
    local field = field_gen()
    local value = value_gen()
    
    return { "HSET", key, field, value }
end
```

#### JSON Encoding Example
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
    local key = key_gen()
    
    return { "SET", key, json_str }
end
```

## Command Line Options

| Option | Description | Default |
|--------|-------------|---------|
| `-h` | Server hostname | 127.0.0.1 |
| `-p` | Server port | 6379 |
| `-u` | Username for authentication | "" |
| `-a` | Password for authentication | "" |
| `-c` | Number of connections (0 for auto) | 0 |
| `-n` | Total number of requests (0 for unlimited) | 0 |
| `-s` | Duration in seconds (0 for unlimited) | 0 |
| `-t` | Target QPS (0 for unlimited) | 0 |
| `-P` | Pipeline depth | 1 |
| `--cores` | CPU cores to use (comma-separated) | all |
| `--cluster` | Enable cluster mode | false |
| `--load` | Load data only, no benchmark | false |
| `--lua` | Use Lua script for generating random command | false |
| `--lua-file` | Use Lua script file for generating random command | false |

## Advanced Features

### Connection Auto-scaling

When `-c 0` is specified, resp-benchmark automatically determines the optimal number of connections based on system resources and target QPS.

### CPU Core Affinity

Bind benchmark threads to specific CPU cores for consistent performance:

```bash
# Use cores 0, 1, 2, 3
resp-benchmark --cores 0,1,2,3 -s 10 "SET {key uniform 100000} {value 64}"
```

### Rate Limiting

Control the request rate for gradual load testing:

```bash
# Target 10,000 QPS
resp-benchmark -t 10000 -s 30 "SET {key uniform 100000} {value 64}"
```

### Pipeline Operations

Use pipelining for bulk operations:

```bash
# Pipeline 10 requests per connection
resp-benchmark -P 10 -c 128 -s 30 "SET {key uniform 100000} {value 64}"
```

### Cluster Mode

Test Redis clusters with automatic slot distribution:

```bash
resp-benchmark --cluster -h cluster-endpoint -p 7000 -s 30 "SET {key uniform 100000} {value 64}"
```

## Performance Optimization

### Best Practices

1. **Pre-load Data**: Use `--load` to populate test data before benchmarking
2. **Appropriate Connections**: Start with `-c 128` and adjust based on results
3. **Key Distribution**: Use `uniform` for reads, `sequence` for writes
4. **Pipeline Wisely**: Use `-P 10` for bulk operations, `-P 1` for latency testing
5. **Clean State**: Clear data between tests to avoid interference

### Example Workflow

```bash
# 1. Clear existing data
redis-cli FLUSHALL

# 2. Load test data
resp-benchmark --load -c 256 -P 10 -n 1000000 "SET {key sequence 100000} {value 64}"

# 3. Benchmark with different patterns
resp-benchmark -c 128 -s 30 "GET {key uniform 100000}"    # Random access
resp-benchmark -c 128 -s 30 "GET {key zipfian 100000}"    # Realistic access
```

## Comprehensive Examples

### String Operations

```bash
# Basic SET/GET
resp-benchmark --load -n 1000000 "SET {key sequence 100000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 100000}"

# Large values
resp-benchmark -s 10 "SET {key uniform 10000} {value 1024}"

# Increment operations
resp-benchmark -s 10 "INCR {key uniform 10000}"
```

### List Operations

```bash
# Build lists
resp-benchmark --load -n 1000000 "LPUSH {key sequence 1000} {value 64}"

# Random access
resp-benchmark -s 10 "LINDEX {key uniform 1000} {rand 1000}"

# Range operations
resp-benchmark -s 10 "LRANGE {key uniform 1000} {range 1000 10}"
```

### Set Operations

```bash
# Populate sets
resp-benchmark --load -n 1000000 "SADD {key sequence 1000} {key sequence 1000}"

# Test membership
resp-benchmark -s 10 "SISMEMBER {key uniform 1000} {key uniform 1000}"
```

### Sorted Set Operations

```bash
# Add scored members
resp-benchmark --load -n 1000000 "ZADD {key sequence 1000} {rand 10000} {key sequence 1000}"

# Score queries
resp-benchmark -s 10 "ZSCORE {key uniform 1000} {key uniform 1000}"

# Range queries
resp-benchmark -s 10 "ZRANGEBYSCORE {key uniform 1000} {range 10000 100}"
```

### Hash Operations

```bash
# Populate hashes
resp-benchmark --load -n 1000000 "HSET {key sequence 1000} {key sequence 100} {value 64}"

# Field access
resp-benchmark -s 10 "HGET {key uniform 1000} {key uniform 100}"
```

### Lua Scripts

```bash
# Load script
redis-cli SCRIPT LOAD "return redis.call('SET', KEYS[1], ARGV[1])"

# Benchmark script execution
resp-benchmark -s 10 "EVALSHA d8f2fad9f8e86a53d2a6ebd960b33c4972cacc37 1 {key uniform 100000} {value 64}"
```

## Python Library API

### Initialization

```python
from resp_benchmark import Benchmark

# Basic connection
bm = Benchmark(host="127.0.0.1", port=6379)

# With authentication
bm = Benchmark(
    host="redis.example.com",
    port=6379,
    username="user",
    password="pass"
)

# Cluster mode
bm = Benchmark(
    host="cluster-endpoint",
    port=7000,
    cluster=True
)

# Custom configuration
bm = Benchmark(
    host="127.0.0.1",
    port=6379,
    cores="0,1,2,3",  # Use specific cores
    timeout=30        # Connection timeout
)
```

### Loading Data

```python
# Sequential loading (recommended)
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128,
    pipeline=10
)

# With rate limiting
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128,
    target=50000  # 50k QPS
)
```

### Benchmarking

```python
# Time-based benchmark
result = bm.bench(
    command="GET {key uniform 1000000}",
    seconds=30,
    connections=64
)

# Count-based benchmark
result = bm.bench(
    command="GET {key uniform 1000000}",
    count=1000000,
    connections=64
)

# With pipeline
result = bm.bench(
    command="SET {key uniform 1000000} {value 64}",
    seconds=30,
    connections=64,
    pipeline=10
)

# Using Lua script
lua_script = """
local user_id = bench.key(10000, "uniform", "user_id")
local data = bench.value(64)
function generate()
    key = user_id()
    value = data()
    return { "SET", key, value }
end
"""
result = bm.bench(command=lua_script, seconds=30, connections=64, use_lua=True)
```

### Result Analysis

```python
# Access benchmark results
print(f"QPS: {result.qps:.2f}")
print(f"Average Latency: {result.avg_latency_ms:.2f}ms")
print(f"P99 Latency: {result.p99_latency_ms:.2f}ms")
print(f"Connections Used: {result.connections}")
```

## Differences from redis-benchmark

resp-benchmark provides more realistic testing compared to redis-benchmark:

1. **Varied Data**: Generates different values for each request, triggering realistic persistence and replication
2. **Distributed Keys**: Uses varied keys instead of single keys for collections
3. **Cluster Awareness**: Properly distributes requests across all slots in cluster mode
4. **Advanced Placeholders**: Supports realistic data distributions (uniform, zipfian, sequence)
5. **Better Metrics**: Detailed latency histograms and connection statistics

## Contributing

Contributions are welcome! Please feel free to submit pull requests or open issues.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.