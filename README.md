# resp-benchmark

[![Python - Version](https://img.shields.io/badge/python-%3E%3D3.8-brightgreen)](https://www.python.org/doc/versions/)
[![PyPI - Version](https://img.shields.io/pypi/v/resp-benchmark?color=%231772b4)](https://pypi.org/project/resp-benchmark/)
[![PyPI - Downloads](https://img.shields.io/pypi/dw/resp-benchmark?color=%231ba784)](https://pypi.org/project/resp-benchmark/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/tair-opensource/resp-benchmark/blob/main/LICENSE)

resp-benchmark is a benchmark tool for testing databases that support the RESP protocol, 
such as [Redis](https://github.com/redis/redis), [Valkey](https://github.com/valkey-io/valkey), 
and [Tair](https://www.alibabacloud.com/en/product/tair). It offers both a command-line interface and a Python library.

## Installation

Requires Python 3.8 or higher.
```bash
pip install resp-benchmark
```

## Usage

### Command-Line Tool

```bash
resp-benchmark --help
```

### Python Library

```python
from resp_benchmark import Benchmark

bm = Benchmark(host="127.0.0.1", port=6379)
bm.flushall()
bm.load_data(command="SET {key sequence 10000000} {value 64}", count=1000000, connections=128)
result = bm.bench("GET {key uniform 10000000}", seconds=3, connections=16)
print(result.qps, result.avg_latency_ms, result.p99_latency_ms)
```

## Custom Commands

resp-benchmark supports custom test commands using placeholder syntax like `SET {key uniform 10000000} {value 64}` which means the SET command will have a key uniformly distributed in the range
0-10000000 and a value of 64 bytes.

Supported placeholders include:

- **`{key uniform N}`**: Generates a random number between `0` and `N-1`. For example, `{key uniform 100}` might generate `key_0000000099`.
- **`{key sequence N}`**: Sequentially generates from `0` to `N-1`, ensuring coverage during data loading. For example, `{key sequence 100}` generates `key_0000000000`, `key_0000000001`, etc.
- **`{key zipfian N}`**: Generates according to a Zipfian distribution (exponent 1.03), simulating real-world key distribution.
- **`{value N}`**: Generates a random string of length `N` bytes. For example, `{value 8}` might generate `92xsqdNg`.
- **`{rand N}`**: Generates a random number between `0` and `N-1`. For example, `{rand 100}` might generate `99`.
- **`{range N W}`**: Generates a pair of random numbers within the range `0` to `N-1`, with a difference of `W`, used for testing `*range*` commands. For example, `{range 100 10}` might generate
  `89 99`.

## Best Practices

### Benchmarking zset

```shell
# Load data
resp-benchmark --load -P 10 -c 256 -n 10007000 "ZADD {key sequence 1000} {rand 70000} {key sequence 10007}"
# Benchmark ZSCORE
resp-benchmark -s 10 "ZSCORE {key uniform 1000} {key uniform 10007}"
# Benchmark ZRANGEBYSCORE
resp-benchmark -s 10 "ZRANGEBYSCORE {key uniform 1000} {range 70000 10}"
```

### Benchmarking Lua Scripts

```shell
redis-cli 'SCRIPT LOAD "return redis.call('\''SET'\'', KEYS[1], ARGV[1])"'
resp-benchmark -s 10 "EVALSHA d8f2fad9f8e86a53d2a6ebd960b33c4972cacc37 1 {key uniform 100000} {value 64}"
```

## Differences with redis-benchmark

When testing Redis with resp-benchmark and redis-benchmark, you might get different results due to:

1. redis-benchmark always uses the same value when testing the set command, which does not trigger DB persistence and replication. In contrast, resp-benchmark uses `{value 64}` to generate different data for each command.
2. redis-benchmark always uses the same primary key when testing list/set/zset/hash commands, while resp-benchmark generates different keys using placeholders like `{key uniform 10000000}`.
3. In cluster mode, redis-benchmark sends requests to each node, but all requests target the same slot on every node.
