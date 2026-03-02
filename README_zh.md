# resp-benchmark

<p align="center">
  <strong>高性能 RESP 协议数据库基准测试工具</strong>
</p>

<p align="center">
  Rust 构建，极致性能 &bull; Python 绑定，开箱即用
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
  <a href="README.md">English</a> | 中文
</p>

---

## 为什么不用 redis-benchmark？

`redis-benchmark` 是 Redis 自带的基准测试工具，适合做快速冒烟测试，但其设计在很多场景下会产生**不真实的结果**。`resp-benchmark` 就是为了解决这些问题而构建的：

| | redis-benchmark | resp-benchmark |
|---|---|---|
| **请求数据** | 每次发送**完全相同的字节**。例如 `redis-benchmark -t SET` 会对所有 key 反复写入相同的 value，这与真实业务流量的特征完全不同——生产环境中每个 key 对应的 value 往往各不相同、大小各异。使用相同数据测出的结果无法反映服务端在面对多样化数据时的真实表现。 | 每个请求通过占位符（如 `{key uniform 100000} {value 64}`）生成**不同的键和值**，更贴近真实业务流量，测试结果更具参考价值。 |
| **键访问模式** | 仅支持 `-r` 标志的随机序号。无法模拟热点键负载或真实的读取模式。 | 三种内置分布：**uniform**（等概率随机）、**zipfian**（热点键，指数 1.03，模拟生产环境的缓存访问规律）、**sequence**（顺序递增，适合批量导入数据）。 |
| **命令灵活性** | 只能从内置的 `-t` 列表中选择少量预定义命令（如 `SET`、`GET`、`LPUSH`），无法自由组合参数，也无法测试自定义命令或模块命令。 | 命令完全由用户拼写，任意 RESP 命令均可直接测试——包括模块命令、`EVALSHA`、多参数命令等。配合占位符和 Lua 脚本，还能实现条件分支、JSON 编码等复杂场景。 |
| **集群模式** | 由于使用固定键，所有请求落入同一个哈希槽，只有集群中的一个节点在干活。测试结果反映的是单节点性能，而非集群吞吐。 | 生成的键均匀分布到**所有哈希槽**，集群中的每个节点都按比例承担流量，测试结果是真正的集群整体吞吐。 |
| **Python API** | 仅有命令行。难以集成到自动化测试流水线，也不方便用代码分析结果。 | 完整的 Python 库（`from resp_benchmark import Benchmark`），方便编写多阶段测试脚本、接入 CI/CD、用代码对比分析结果。 |
| **Lua 脚本** | 不支持。 | 支持 Lua 脚本，通过 `bench.key()`、`bench.value()`、`bench.rand()` 生成动态命令，可以包含条件分支、JSON 编码等复杂逻辑。 |
| **连接数调优** | 需要手动猜测 `-c` 的值。连接太少则服务端没有被充分利用，太多则浪费在上下文切换上。 | **自动扩展**（`-c 0`）：从 1 个连接开始，逐步翻倍，当吞吐不再增长（增幅 < 30%）时锁定最优连接数，无需手动调参。 |
| **短连接测试** | 不支持。 | `--short-connection` 模式为**每条命令**创建并销毁一条 TCP 连接，用于测量连接建立开销——这对测试代理层（如 Twemproxy、Codis）和连接池性能至关重要。 |

## 目录

- [安装](#安装)
- [快速开始](#快速开始)
- [命令语法](#命令语法)
- [Lua 脚本支持](#lua-脚本支持)
- [命令行选项](#命令行选项)
- [高级功能](#高级功能)
- [性能调优建议](#性能调优建议)
- [完整示例](#完整示例)
- [Python 库 API](#python-库-api)
- [贡献](#贡献)
- [许可证](#许可证)

## 安装

```bash
pip install resp-benchmark
```

需要 Python 3.9+。macOS 和 Linux 提供预编译安装包。

## 快速开始

### 命令行使用

```bash
# 基础测试：随机键 + 64 字节值，持续 10 秒
resp-benchmark -s 10 "SET {key uniform 100000} {value 64}"

# 先加载 100 万条数据，再测试读取
resp-benchmark --load -n 1000000 "SET {key sequence 100000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 100000}"

# 128 连接，管道深度 10，持续 30 秒
resp-benchmark -c 128 -P 10 -s 30 "SET {key uniform 1000000} {value 128}"

# 使用内联 Lua 脚本
resp-benchmark --lua -s 10 "local key = bench.key(10000, 'uniform', 'test'); function generate() return {'SET', key(), bench.value(64)()} end"

# 使用 Lua 脚本文件
resp-benchmark --lua-file -s 10 workloads/hello.lua
```

### Python 库使用

```python
from resp_benchmark import Benchmark

bm = Benchmark(host="127.0.0.1", port=6379)

# 加载测试数据
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128
)

# 运行基准测试
result = bm.bench(
    command="GET {key uniform 1000000}",
    seconds=30,
    connections=64
)

print(f"QPS: {result.qps}")
print(f"平均延迟: {result.avg_latency_ms}ms")
print(f"P99 延迟: {result.p99_latency_ms}ms")
```

## 命令语法

resp-benchmark 使用占位符系统生成多样化的真实测试数据。占位符用 `{}` 包裹在命令字符串中。

### 键占位符

| 占位符 | 描述 | 示例 |
|---|---|---|
| `{key uniform N}` | 从 `key_0000000000` 到 `key_{N-1}` 的随机键，每个键被选中的概率相等 | `{key uniform 100000}` → `key_0000042371` |
| `{key sequence N}` | 从 0 到 N-1 的顺序键，循环递增。适合加载数据，因为每个键在重复之前只会被写入一次。 | `{key sequence 100000}` → `key_0000000000`, `key_0000000001`, ... |
| `{key zipfian N}` | Zipfian 分布（指数 1.03）：少量键承担大部分请求，模拟生产环境的缓存热点访问规律。 | `{key zipfian 100000}` → 高频 `key_0000000001`，低频 `key_0000099999` |

### 值占位符

| 占位符 | 描述 | 示例 |
|---|---|---|
| `{value N}` | N 字节的随机字母数字字符串，每次请求都不同 | `{value 64}` → `a8x9mK2pQ7...`（64 字节） |
| `{rand N}` | 0 到 N-1 的随机整数 | `{rand 1000}` → `742` |
| `{range N W}` | 两个整数：[0, N-1] 内的随机起点和起点+W（上限为 N-1）。适合范围查询。 | `{range 1000 10}` → `45 55` |

### 命令示例

```bash
# 字符串操作
SET {key uniform 1000000} {value 64}
GET {key uniform 1000000}
INCR {key uniform 100000}

# 列表操作
LPUSH {key uniform 1000} {value 64}
LINDEX {key uniform 1000} {rand 100}

# 集合操作
SADD {key uniform 1000} {value 64}
SISMEMBER {key uniform 1000} {value 64}

# 有序集合操作
ZADD {key uniform 1000} {rand 1000} {value 64}
ZRANGEBYSCORE {key uniform 1000} {range 1000 100}

# 哈希操作
HSET {key uniform 1000} {key uniform 100} {value 64}
HGET {key uniform 1000} {key uniform 100}
```

## Lua 脚本支持

当需要动态逻辑（条件分支、计算字段、JSON 负载）时，可以用 Lua 脚本代替简单占位符。

### Lua API

全局 `bench` 对象提供以下生成器函数：

**`bench.key(range, distribution, name)`** — 创建键生成器。`distribution` 可选 `"uniform"`、`"sequence"` 或 `"zipfian"`。`name` 标识生成器；同名生成器在线程间共享状态，确保 sequence 生成器不会产生重复。

**`bench.value(size)`** — 创建值生成器，每次调用返回 `size` 字节的随机字母数字字符串。

**`bench.rand(range)`** — 创建随机数生成器，每次调用返回 [0, range-1] 内的随机整数。

**`json.encode(data)`** — 将 Lua 表转换为 JSON 字符串。

### 脚本结构

每个 Lua 脚本必须定义全局 `generate` 函数，返回表示 Redis 命令的字符串数组：

```lua
function generate()
    return { "COMMAND", "arg1", "arg2", ... }
end
```

### 示例

#### 基础 SET

```lua
local key_gen = bench.key(10000, "uniform", "my_key")
local value_gen = bench.value(64)
function generate()
    return { "SET", key_gen(), value_gen() }
end
```

#### 混合读写（50/50）

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

#### 哈希操作

```lua
local key_gen = bench.key(1000, "uniform", "hash_key")
local field_gen = bench.key(100, "zipfian", "hash_field")
local value_gen = bench.value(32)
function generate()
    return { "HSET", key_gen(), field_gen(), value_gen() }
end
```

#### JSON 负载

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

## 命令行选项

| 选项 | 描述 | 默认值 |
|---|---|---|
| `-h` | 服务器主机名 | 127.0.0.1 |
| `-p` | 服务器端口 | 6379 |
| `-u` | ACL 认证用户名 | "" |
| `-a` | 认证密码 | "" |
| `-c` | 连接数（0 = 自动扩展） | 0 |
| `-n` | 总请求数（0 = 无限制） | 0 |
| `-s` | 持续时间/秒（0 = 无限制） | 0 |
| `-t` | 目标 QPS（0 = 不限速） | 0 |
| `-P` | 管道深度 | 1 |
| `--cores` | 使用的 CPU 核心（逗号分隔，如 `0,1,2,3`） | 全部 |
| `--cluster` | 启用 Redis 集群模式 | false |
| `--load` | 仅加载数据，不进行基准测试 | false |
| `--short-connection` | 每个命令创建新连接 | false |
| `--lua` | 将命令字符串作为 Lua 脚本执行 | false |
| `--lua-file` | 将命令字符串作为 Lua 脚本文件路径 | false |
| `-q` | 静默模式：不输出进度信息 | false |

## 高级功能

### 连接自动扩展

当 `-c 0`（默认值）时，resp-benchmark 从少量连接开始，逐步翻倍。当翻倍后吞吐增幅不足 30% 时，锁定当前连接数开始正式测试。自动找到最优连接数，无需手动调参。

```bash
resp-benchmark -c 0 -s 30 "GET {key uniform 100000}"
```

### CPU 核心绑定

将测试线程绑定到指定 CPU 核心，减少上下文切换抖动，获得更稳定的结果：

```bash
resp-benchmark --cores 0,1,2,3 -s 10 "SET {key uniform 100000} {value 64}"
```

### 速率限制

控制请求速率进行渐进式负载测试，适合找到延迟飙升的 QPS 阈值：

```bash
resp-benchmark -t 10000 -s 30 "SET {key uniform 100000} {value 64}"
```

### 管道操作

每次往返发送多条命令以最大化吞吐。管道深度越大，单命令延迟开销越低，但每批次延迟越高：

```bash
resp-benchmark -P 10 -c 128 -s 30 "SET {key uniform 100000} {value 64}"
```

### 集群模式

根据键的哈希槽自动将请求分发到所有集群节点：

```bash
resp-benchmark --cluster -h cluster-endpoint -p 7000 -s 30 "SET {key uniform 100000} {value 64}"
```

### 短连接模式

为每条命令创建并关闭一条 TCP 连接，用于测量连接建立开销。这对测试代理层（如 Twemproxy、Codis）或连接池性能至关重要。

限制：不兼容 `--load`；管道必须为 1。

```bash
resp-benchmark --short-connection -c 50 -s 10 "PING"
```

## 性能调优建议

1. **预加载数据**：用 `--load` 配合 `sequence` 键预先填充数据，避免读取测试时命中空键。
2. **先用自动连接数**：让 `-c 0` 找到最优值，然后固定该值以确保可复现。
3. **选择合适的键分布**：缓存场景用 `uniform`，模拟生产流量用 `zipfian`，批量导入用 `sequence`。
4. **管道与延迟的取舍**：测吞吐量用 `-P 10`，测单命令延迟用 `-P 1`。
5. **保持测试环境干净**：每次测试前执行 `FLUSHALL`，避免残留数据干扰。

### 典型流程

```bash
# 1. 清除现有数据
redis-cli FLUSHALL

# 2. 加载 100 万条键值对
resp-benchmark --load -c 256 -P 10 -n 1000000 "SET {key sequence 1000000} {value 64}"

# 3. 测试不同访问模式
resp-benchmark -c 128 -s 30 "GET {key uniform 1000000}"    # 随机访问
resp-benchmark -c 128 -s 30 "GET {key zipfian 1000000}"    # 热点访问
```

## 完整示例

### 字符串操作

```bash
resp-benchmark --load -n 1000000 "SET {key sequence 1000000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 1000000}"
resp-benchmark -s 10 "SET {key uniform 10000} {value 1024}"
resp-benchmark -s 10 "INCR {key uniform 10000}"
```

### 列表操作

```bash
resp-benchmark --load -n 1000000 "LPUSH {key sequence 1000} {value 64}"
resp-benchmark -s 10 "LINDEX {key uniform 1000} {rand 1000}"
resp-benchmark -s 10 "LRANGE {key uniform 1000} {range 1000 10}"
```

### 集合操作

```bash
resp-benchmark --load -n 1000000 "SADD {key sequence 1000} {key sequence 1000}"
resp-benchmark -s 10 "SISMEMBER {key uniform 1000} {key uniform 1000}"
```

### 有序集合操作

```bash
resp-benchmark --load -n 1000000 "ZADD {key sequence 1000} {rand 10000} {key sequence 1000}"
resp-benchmark -s 10 "ZSCORE {key uniform 1000} {key uniform 1000}"
resp-benchmark -s 10 "ZRANGEBYSCORE {key uniform 1000} {range 10000 100}"
```

### 哈希操作

```bash
resp-benchmark --load -n 1000000 "HSET {key sequence 1000} {key sequence 100} {value 64}"
resp-benchmark -s 10 "HGET {key uniform 1000} {key uniform 100}"
```

### EVALSHA

```bash
redis-cli SCRIPT LOAD "return redis.call('SET', KEYS[1], ARGV[1])"
resp-benchmark -s 10 "EVALSHA d8f2fad9f8e86a53d2a6ebd960b33c4972cacc37 1 {key uniform 100000} {value 64}"
```

## Python 库 API

### 初始化

```python
from resp_benchmark import Benchmark

# 基本连接
bm = Benchmark(host="127.0.0.1", port=6379)

# 带认证
bm = Benchmark(host="redis.example.com", port=6379, username="user", password="pass")

# 集群模式
bm = Benchmark(host="cluster-endpoint", port=7000, cluster=True)

# 绑定 CPU 核心
bm = Benchmark(host="127.0.0.1", port=6379, cores="0,1,2,3")
```

### 加载数据

```python
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128,
    pipeline=10
)
```

### 基准测试

```python
# 按时间
result = bm.bench(command="GET {key uniform 1000000}", seconds=30, connections=64)

# 按次数
result = bm.bench(command="GET {key uniform 1000000}", count=1000000, connections=64)

# 带管道
result = bm.bench(command="SET {key uniform 1000000} {value 64}", seconds=30, connections=64, pipeline=10)

# 短连接
result = bm.bench(command="PING", seconds=30, connections=50, short_connection=True)

# Lua 脚本
lua_script = """
local user_id = bench.key(10000, "uniform", "user_id")
local data = bench.value(64)
function generate()
    return { "SET", user_id(), data() }
end
"""
result = bm.bench(command=lua_script, seconds=30, connections=64, use_lua=True)
```

### 结果分析

```python
print(f"QPS: {result.qps:.2f}")
print(f"平均延迟: {result.avg_latency_ms:.2f}ms")
print(f"P99 延迟: {result.p99_latency_ms:.2f}ms")
print(f"连接数: {result.connections}")
```

## 贡献

欢迎贡献！请随时提交 Pull Request 或开启 Issue。

## 许可证

该项目基于 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。
