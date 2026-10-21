"""Run with `python -m unittest discover -s tests -v` after `maturin develop`.

These tests start their own temporary Redis server with 256 databases.
"""
import contextlib
import io
import multiprocessing
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import redis

from resp_benchmark import Benchmark
from resp_benchmark.cli import parse_args


class DbidValidationTests(unittest.TestCase):
    def test_non_positive_pipeline_is_rejected(self):
        bm = Benchmark(cores="0", dbid="0-2")
        for pipeline in [0, -1]:
            with self.subTest(pipeline=pipeline):
                for method in [bm.bench, bm.load_data]:
                    with self.assertRaisesRegex(ValueError, "pipeline must be greater than 0"):
                        method(command="PING", connections=1, pipeline=pipeline, count=1)
                result = subprocess.run([
                    sys.executable, "-m", "resp_benchmark.cli", "--dbid", "0-2",
                    "-c", "1", "-P", str(pipeline), "-n", "1", "PING",
                ], capture_output=True, text=True, timeout=5)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn("pipeline must be greater than 0", result.stderr)

    def test_native_arguments_are_validated(self):
        from resp_benchmark import _resp_benchmark_rust_lib

        args = dict(host="127.0.0.1", port=6379, username="", password="", cluster=False,
                    tls=False, timeout=1, cores=[0], command="PING", connections=1, target=0,
                    pipeline=1, count=1, seconds=0, load=False, quiet=True,
                    short_connection=False, use_lua=False)
        for invalid, message in [
            ({"pipeline": 0}, "pipeline must be greater than 0"),
            ({"dbids": []}, "at least one non-negative database ID"),
            ({"dbids": [-1]}, "at least one non-negative database ID"),
            ({"cluster": True, "dbids": [0, 1]}, "cluster mode only supports dbid 0"),
        ]:
            with self.subTest(invalid=invalid):
                with self.assertRaisesRegex(ValueError, message):
                    _resp_benchmark_rust_lib.benchmark(**dict(args, **invalid))

    def test_cli_and_python_reject_invalid_databases(self):
        for dbid in ["", "-1", "2-0", "0,,1"]:
            with self.subTest(dbid=dbid):
                with self.assertRaises(ValueError):
                    Benchmark(dbid=dbid)
                with patch.object(sys, "argv", ["resp-benchmark", f"--dbid={dbid}"]), contextlib.redirect_stderr(io.StringIO()):
                    with self.assertRaises(SystemExit) as error:
                        parse_args()
                    self.assertEqual(error.exception.code, 2)

    def test_cluster_only_accepts_database_zero(self):
        self.assertEqual(Benchmark(cluster=True, dbid="0,0").dbids, [0])
        with self.assertRaisesRegex(ValueError, "cluster mode only supports dbid 0"):
            Benchmark(cluster=True, dbid="0-2")
        with patch.object(sys, "argv", ["resp-benchmark", "--cluster", "--dbid", "255"]), contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                parse_args()
            self.assertEqual(error.exception.code, 2)


@unittest.skipUnless(shutil.which("redis-server"), "redis-server is required")
class DbidIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix="resp-benchmark-dbid-")
        cls.addClassCleanup(cls.temp.cleanup)
        with socket.socket() as sock:
            sock.bind(("127.0.0.1", 0))
            cls.port = sock.getsockname()[1]
        cls.password = "dbid-test-p@ss:/"
        log = Path(cls.temp.name) / "redis.log"
        cls.server = subprocess.Popen([
            shutil.which("redis-server"), "--bind", "127.0.0.1", "--port", str(cls.port),
            "--databases", "256", "--save", "", "--appendonly", "no",
            "--dir", cls.temp.name, "--logfile", str(log), "--requirepass", cls.password,
        ], stdout=subprocess.DEVNULL, stderr=subprocess.STDOUT)
        cls.addClassCleanup(cls.stop_server)
        cls.admin = cls.connection(0)
        cls.addClassCleanup(cls.admin.close)
        for _ in range(100):
            try:
                if cls.admin.ping():
                    break
            except redis.ConnectionError:
                pass
            if cls.server.poll() is not None:
                raise RuntimeError(log.read_text())
            time.sleep(0.05)
        else:
            raise RuntimeError("Temporary Redis server did not become ready")

    @classmethod
    def stop_server(cls):
        cls.server.terminate()
        try:
            cls.server.wait(timeout=5)
        except subprocess.TimeoutExpired:
            cls.server.kill()
            cls.server.wait(timeout=5)

    @classmethod
    def connection(cls, dbid):
        return redis.Redis(host="127.0.0.1", port=cls.port, db=dbid, password=cls.password)

    def assert_database_counts(self, expected):
        for dbid, count in expected.items():
            with self.connection(dbid) as connection:
                self.assertEqual(int(connection.get("request_count") or 0), count, f"DB {dbid}")
        actual_databases = set(self.admin.info("keyspace"))
        self.assertEqual(actual_databases, {f"db{dbid}" for dbid, count in expected.items() if count})

    def test_request_quotas_across_modes(self):
        for mode, dbid, count, connections, pipeline in [
            ("bench", "0-2,255", 1003, 7, 17),
            ("load", "0,1,2,255", 101, 1, 10),
            ("short", "0-2,255", 19, 3, 1),
            ("auto", "0-2,255", 17, 0, 5),
            ("bench", "0-2,255", 2, 1, 10),
            ("bench", "255", 31, 2, 10),
            ("lua", "0-2,255", 23, 2, 4),
            ("rate_limited", "0-2,255", 101, 3, 7),
            ("bench", "255,0-2,2", 19, 1, 7),
        ]:
            with self.subTest(mode=mode, dbid=dbid, count=count, connections=connections):
                self.admin.flushall()
                cores = "0,1" if multiprocessing.cpu_count() > 1 else "0"
                bm = Benchmark(port=self.port, password=self.password, cores=cores, dbid=dbid)
                before = self.admin.info("stats")["total_connections_received"]
                args = dict(command="INCR request_count", count=count, connections=connections, pipeline=pipeline, quiet=True)
                if mode == "rate_limited":
                    args["target"] = 100
                if mode == "load":
                    bm.load_data(**args)
                else:
                    if mode == "lua":
                        args.update(command="function generate() return {'INCR', 'request_count'} end", use_lua=True)
                    bm.bench(**args, short_connection=mode == "short")
                after = self.admin.info("stats")["total_connections_received"]
                if connections == 1:
                    self.assertEqual(after - before, 1, "Database switches should reuse the connection")
                expected = {db: count // len(bm.dbids) + (i < count % len(bm.dbids)) for i, db in enumerate(bm.dbids)}
                self.assert_database_counts(expected)

    def test_default_database(self):
        self.admin.flushall()
        bm = Benchmark(port=self.port, password=self.password, cores="0")
        bm.bench(command="INCR request_count", count=17, connections=1, pipeline=4, quiet=True)
        self.assert_database_counts({0: 17})

    def test_cli_list_and_range(self):
        for dbid in ["0,1,2,255", "0-2,255"]:
            with self.subTest(dbid=dbid):
                self.admin.flushall()
                result = subprocess.run([
                    sys.executable, "-m", "resp_benchmark.cli", "-p", str(self.port),
                    "-a", self.password, "--dbid", dbid, "--cores", "0", "-c", "1",
                    "-P", "10", "-n", "101", "INCR request_count",
                ], capture_output=True, text=True, timeout=20)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assert_database_counts({0: 26, 1: 25, 2: 25, 255: 25})

    def test_time_only_run_stops_and_visits_all_databases(self):
        self.admin.flushall()
        result = subprocess.run([
            sys.executable, "-m", "resp_benchmark.cli", "-p", str(self.port),
            "-a", self.password, "--dbid", "0-2,255", "--cores", "0", "-c", "4",
            "-P", "5", "-s", "1", "INCR request_count",
        ], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        for dbid in [0, 1, 2, 255]:
            with self.connection(dbid) as connection:
                self.assertGreater(int(connection.get("request_count") or 0), 0)
        self.assertEqual(set(self.admin.info("keyspace")), {"db0", "db1", "db2", "db255"})

    def test_unavailable_database_fails_before_writing_to_it(self):
        for dbid, count, expected in [("256", 1, {}), ("0,256", 2, {0: 1})]:
            with self.subTest(dbid=dbid):
                self.admin.flushall()
                result = subprocess.run([
                    sys.executable, "-m", "resp_benchmark.cli", "-p", str(self.port),
                    "-a", self.password, "--dbid", dbid, "--cores", "0", "-c", "1",
                    "-n", str(count), "INCR request_count",
                ], capture_output=True, text=True, timeout=20)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("dbid 256", result.stderr)
                self.assert_database_counts(expected)


if __name__ == "__main__":
    unittest.main()
