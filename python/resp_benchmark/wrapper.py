import multiprocessing
from dataclasses import dataclass
from typing import List

import pydantic
import redis

from .cores import parse_cores_string

from ._resp_benchmark_rust_lib import BenchmarkWorkerPool

@dataclass
class Result:
    """
    Represents the overall result of a benchmark.

    Attributes:
        qps (float): Average queries per second.
        avg_latency_ms (float): Average latency in milliseconds.
        p99_latency_ms (float): 99th percentile latency in milliseconds.
        connections (int): The number of parallel connections.
    """
    qps: float
    avg_latency_ms: float
    p99_latency_ms: float
    max_latency_ms: float
    connections: int


class Benchmark:
    """
    A class to perform and manage benchmark tests on a Redis server.

    Attributes:
        host (str): The host address of the Redis server.
        port (int): The port number of the Redis server.
        username (str): The username for authentication.
        password (str): The password for authentication.
        cluster (bool): Whether to connect to a Redis cluster.
        tls (bool): Whether to use TLS for the connection.
        timeout (int): Timeout for the connection in seconds.
        cores (str): Comma-separated list of CPU cores to use.
    """

    @pydantic.validate_call
    def __init__(
            self,
            host: str = "127.0.0.1",
            port: int = 6379,
            username: str = "",
            password: str = "",
            cluster: bool = False,
            # tls: bool = False,
            timeout: int = 30,
            cores: str = "",
    ):
        self.host = host
        self.port = port
        self.username = username
        self.password = password
        self.cluster = cluster
        # self.tls = tls
        self.timeout = timeout
        if cores == "":
            cores = f"0-{multiprocessing.cpu_count() - 1}"
        self.cores = parse_cores_string(cores)

    def bench(
            self,
            command: str,
            connections: int = 0,
            pipeline: int = 1,
            count: int = 0,
            target: int = 0,
            seconds: int = 0,
            quiet: bool = False,
    ) -> Result:
        """
        Runs a benchmark test with the specified parameters.

        Args:
            command (str): The Redis command to benchmark.
            connections (int): The number of parallel connections.
            pipeline (int): The number of commands to pipeline.
            count (int): The total number of requests to make.
            target(int): The number of requests to send per second.
            seconds (int): The duration of the test in seconds.
            quiet: (bool): Whether to suppress output.
        Returns:
            Result: The results of the benchmark test.
        """
        from . import _resp_benchmark_rust_lib
        ret = _resp_benchmark_rust_lib.benchmark(
            host=self.host,
            port=self.port,
            username=self.username,
            password=self.password,
            cluster=self.cluster,
            tls=False,  # TODO: Implement TLS support
            timeout=self.timeout,
            cores=self.cores,

            command=command,
            connections=connections,
            pipeline=pipeline,
            count=count,
            target=target,
            seconds=seconds,
            load=False,
            quiet=quiet,
        )
        result = Result(
            qps=ret.qps,
            avg_latency_ms=ret.avg_latency_ms,
            p99_latency_ms=ret.p99_latency_ms,
            max_latency_ms=ret.max_latency_ms,
            connections=ret.connections
        )

        return result

    def load_data(self, command: str, count: int, target: int = 0, connections: int = 128, pipeline: int = 10, quiet: bool = False):
        """
        Load data into the Redis server using the specified command.

        Args:
            command (str): The Redis command to use for loading data.
            count (int): The total number of requests to make.
            target(int): The number of requests to send per second.
            connections (int): The number of parallel connections.
            pipeline (int): The number of commands to pipeline
            quiet: (bool): Whether to suppress output.
        """

        from . import _resp_benchmark_rust_lib
        _resp_benchmark_rust_lib.benchmark(
            host=self.host,
            port=self.port,
            username=self.username,
            password=self.password,
            cluster=self.cluster,
            tls=False,
            timeout=self.timeout,
            cores=self.cores,

            command=command,
            connections=connections,
            pipeline=pipeline,
            count=count,
            target=target,
            seconds=0,
            load=True,
            quiet=quiet,
        )

    def flushall(self):
        """
        Clears all data from all Redis databases.
        """
        r = redis.Redis(host=self.host, port=self.port, username=self.username, password=self.password)
        r.flushall()

    class AsyncBenchmarkContext:
        def __init__(self, raw_benchmark_context):
            self.raw_benchmark_context = raw_benchmark_context

        def stop(self):
            self.raw_benchmark_context.stop()
        
        def join(self):
            self.raw_benchmark_context.join()
        
        def try_join(self):
            return self.raw_benchmark_context.try_join()
        
        def current_result(self) -> Result:
            ret = self.raw_benchmark_context.current_result()
            return Result(
                qps=ret.qps,
                avg_latency_ms=ret.avg_latency_ms,
                p99_latency_ms=ret.p99_latency_ms,
                max_latency_ms=ret.max_latency_ms,
                connections=ret.connections
            )

    def async_bench(self, benchmark_pool: BenchmarkWorkerPool, command: str, connections: int = 0, pipeline: int = 1, count: int = 0, target: int = 0, seconds: int = 0, quiet: bool = False) -> AsyncBenchmarkContext:
        from . import _resp_benchmark_rust_lib
        raw_ctx = _resp_benchmark_rust_lib.async_benchmark(
            worker_pool=benchmark_pool,

            host=self.host,
            port=self.port,
            username=self.username,
            password=self.password,
            cluster=self.cluster,
            tls=False,  # TODO: Implement TLS support
            timeout=self.timeout,
            cores=self.cores,

            command=command,
            connections=connections,
            pipeline=pipeline,
            count=count,
            target=target,
            seconds=seconds,
            load=False,
            quiet=quiet,
        )
        return self.AsyncBenchmarkContext(raw_ctx)
