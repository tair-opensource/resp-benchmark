# resp-benchmark

[![Python - Version](https://img.shields.io/badge/python-%3E%3D3.8-brightgreen)](https://www.python.org/doc/versions/)
[![PyPI - Version](https://img.shields.io/pypi/v/resp-benchmark?color=%231772b4)](https://pypi.org/project/resp-benchmark/)
[![PyPI - Downloads](https://img.shields.io/pypi/dw/resp-benchmark?color=%231ba784)](https://pypi.org/project/resp-benchmark/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/tair-opensource/resp-benchmark/blob/main/LICENSE)

[English](README_en.md) | 中文

一个基于 Rust 构建的 RESP (Redis 序列化协议) 数据库基准测试工具，提供 Python 绑定。用于测试 Redis、Valkey、Tair 等 RESP 兼容数据库的性能。

## 主要功能

- 支持自定义命令模板和占位符
- 多线程并发测试
- 支持连接池和管道操作
- 支持 Redis 集群模式
- 提供 CLI 工具和 Python 库接口
- 内置 QPS 限流控制

## 安装

需要 Python 3.9 或更高版本。

```bash
pip install resp-benchmark
```

## 快速开始

### 命令行使用

```bash
# 基础基准测试
resp-benchmark -s 10 "SET {key uniform 100000} {value 64}"

# 加载数据然后测试
resp-benchmark --load -n 1000000 "SET {key sequence 100000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 100000}"

# 自定义连接数和管道
resp-benchmark -c 128 -P 10 -s 30 "SET {key uniform 1000000} {value 128}"
```

### Python 库使用

```python
from resp_benchmark import Benchmark

# 初始化基准测试
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

resp-benchmark 使用强大的占位符系统来生成多样化和真实的测试数据：

### 键占位符

- **`{key uniform N}`**: 从 0 到 N-1 的随机键
  - 示例: `{key uniform 100000}` → `key_0000099999`
  
- **`{key sequence N}`**: 从 0 到 N-1 的顺序键（适用于加载数据）
  - 示例: `{key sequence 100000}` → `key_0000000000`, `key_0000000001`, ...
  
- **`{key zipfian N}`**: Zipfian 分布键（模拟真实世界的访问模式）
  - 示例: `{key zipfian 100000}` → 遵循指数为 1.03 的 Zipfian 分布

### 值占位符

- **`{value N}`**: N 字节的随机字符串
  - 示例: `{value 64}` → `a8x9mK2p...` (64 字节)
  
- **`{rand N}`**: 0 到 N-1 的随机数
  - 示例: `{rand 1000}` → `742`
  
- **`{range N W}`**: 范围 N 内相差 W 的两个数字
  - 示例: `{range 100 10}` → `45 55`

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

## 命令行选项

| 选项 | 描述 | 默认值 |
|------|------|--------|
| `-h` | 服务器主机名 | 127.0.0.1 |
| `-p` | 服务器端口 | 6379 |
| `-u` | 认证用户名 | "" |
| `-a` | 认证密码 | "" |
| `-c` | 连接数（0 为自动） | 0 |
| `-n` | 总请求数（0 为无限） | 0 |
| `-s` | 持续时间（秒）（0 为无限） | 0 |
| `-t` | 目标 QPS（0 为无限） | 0 |
| `-P` | 管道深度 | 1 |
| `--cores` | 使用的 CPU 核心（逗号分隔） | 全部 |
| `--cluster` | 启用集群模式 | false |
| `--load` | 仅加载数据，不进行基准测试 | false |
| `--short-connection` | 短连接模式：每次命令都创建新连接 | false |

## 高级特性

### 连接自动扩展

当指定 `-c 0` 时，resp-benchmark 根据系统资源和目标 QPS 自动确定最优连接数。

### CPU 核心绑定

将基准测试线程绑定到特定 CPU 核心以获得一致的性能：

```bash
# 使用核心 0, 1, 2, 3
resp-benchmark --cores 0,1,2,3 -s 10 "SET {key uniform 100000} {value 64}"
```

### 速率限制

控制请求速率进行渐进式负载测试：

```bash
# 目标 10,000 QPS
resp-benchmark -t 10000 -s 30 "SET {key uniform 100000} {value 64}"
```

### 管道操作

使用管道进行批量操作：

```bash
# 每个连接管道 10 个请求
resp-benchmark -P 10 -c 128 -s 30 "SET {key uniform 100000} {value 64}"
```

### 集群模式

使用自动槽位分布测试 Redis 集群：

```bash
resp-benchmark --cluster -h cluster-endpoint -p 7000 -s 30 "SET {key uniform 100000} {value 64}"
```

## 性能优化

### 最佳实践

1. **预加载数据**: 使用 `--load` 在基准测试前填充测试数据
2. **适当的连接数**: 从 `-c 128` 开始，根据结果调整
3. **键分布**: 读取使用 `uniform`，写入使用 `sequence`
4. **明智使用管道**: 批量操作使用 `-P 10`，延迟测试使用 `-P 1`
5. **清洁状态**: 测试之间清除数据以避免干扰

### 示例工作流程

```bash
# 1. 清除现有数据
redis-cli FLUSHALL

# 2. 加载测试数据
resp-benchmark --load -c 256 -P 10 -n 1000000 "SET {key sequence 100000} {value 64}"

# 3. 使用不同模式进行基准测试
resp-benchmark -c 128 -s 30 "GET {key uniform 100000}"    # 随机访问
resp-benchmark -c 128 -s 30 "GET {key zipfian 100000}"    # 真实访问模式
```

## 完整示例

### 字符串操作

```bash
# 基本 SET/GET
resp-benchmark --load -n 1000000 "SET {key sequence 100000} {value 64}"
resp-benchmark -s 10 "GET {key uniform 100000}"

# 大值
resp-benchmark -s 10 "SET {key uniform 10000} {value 1024}"

# 递增操作
resp-benchmark -s 10 "INCR {key uniform 10000}"
```

### 列表操作

```bash
# 构建列表
resp-benchmark --load -n 1000000 "LPUSH {key sequence 1000} {value 64}"

# 随机访问
resp-benchmark -s 10 "LINDEX {key uniform 1000} {rand 1000}"

# 范围操作
resp-benchmark -s 10 "LRANGE {key uniform 1000} {range 1000 10}"
```

### 集合操作

```bash
# 填充集合
resp-benchmark --load -n 1000000 "SADD {key sequence 1000} {key sequence 1000}"

# 测试成员关系
resp-benchmark -s 10 "SISMEMBER {key uniform 1000} {key uniform 1000}"
```

### 有序集合操作

```bash
# 添加带分数的成员
resp-benchmark --load -n 1000000 "ZADD {key sequence 1000} {rand 10000} {key sequence 1000}"

# 分数查询
resp-benchmark -s 10 "ZSCORE {key uniform 1000} {key uniform 1000}"

# 范围查询
resp-benchmark -s 10 "ZRANGEBYSCORE {key uniform 1000} {range 10000 100}"
```

### 哈希操作

```bash
# 填充哈希
resp-benchmark --load -n 1000000 "HSET {key sequence 1000} {key sequence 100} {value 64}"

# 字段访问
resp-benchmark -s 10 "HGET {key uniform 1000} {key uniform 100}"
```

### Lua 脚本

```bash
# 加载脚本
redis-cli SCRIPT LOAD "return redis.call('SET', KEYS[1], ARGV[1])"

# 基准测试脚本执行
resp-benchmark -s 10 "EVALSHA d8f2fad9f8e86a53d2a6ebd960b33c4972cacc37 1 {key uniform 100000} {value 64}"
```

## 短连接性能测试

短连接模式：每次命令都创建新连接，执行完成后立即关闭。

**限制：**
- 不支持 `--load` 和 `-P`（pipeline 必须为 1）

**使用：**
```bash
resp-benchmark --short-connection "PING" -c 50 -s 10
```

**Python API：**
```python
result = bm.bench(command="PING", seconds=30, connections=50, short_connection=True)
```


## Python 库 API

### 初始化

```python
from resp_benchmark import Benchmark

# 基本连接
bm = Benchmark(host="127.0.0.1", port=6379)

# 带认证
bm = Benchmark(
    host="redis.example.com",
    port=6379,
    username="user",
    password="pass"
)

# 集群模式
bm = Benchmark(
    host="cluster-endpoint",
    port=7000,
    cluster=True
)

# 自定义配置
bm = Benchmark(
    host="127.0.0.1",
    port=6379,
    cores="0,1,2,3",  # 使用特定核心
    timeout=30        # 连接超时
)
```

### 加载数据

```python
# 顺序加载（推荐）
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128,
    pipeline=10
)

# 带速率限制
bm.load_data(
    command="SET {key sequence 1000000} {value 64}",
    count=1000000,
    connections=128,
    target=50000  # 50k QPS
)
```

### 基准测试

```python
# 基于时间的基准测试
result = bm.bench(
    command="GET {key uniform 1000000}",
    seconds=30,
    connections=64
)

# 基于计数的基准测试
result = bm.bench(
    command="GET {key uniform 1000000}",
    count=1000000,
    connections=64
)

# 带管道
result = bm.bench(
    command="SET {key uniform 1000000} {value 64}",
    seconds=30,
    connections=64,
    pipeline=10
)

# 短连接模式
result = bm.bench(
    command="PING",
    seconds=30,
    connections=50,
    short_connection=True
)
```

### 结果分析

```python
# 访问基准测试结果
print(f"QPS: {result.qps:.2f}")
print(f"平均延迟: {result.avg_latency_ms:.2f}ms")
print(f"P99 延迟: {result.p99_latency_ms:.2f}ms")
print(f"使用的连接数: {result.connections}")
```

## 贡献

欢迎贡献！请随时提交拉取请求或开启议题。

## 许可证

该项目基于 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件。