import argparse
from importlib.metadata import version

from resp_benchmark.wrapper import Benchmark


def parse_args():
    parser = argparse.ArgumentParser(
        description="RESP Benchmark Tool",
        add_help=False,
    )

    parser.add_argument("-h", metavar="host", default="127.0.0.1", help="Server hostname (default 127.0.0.1)")
    parser.add_argument("-p", metavar="port", type=int, default=6379, help="Server port (default 6379)")
    parser.add_argument("-u", metavar="username", type=str, default="", help="Used to send ACL style \"AUTH username pass\". Needs -a.")
    parser.add_argument("-a", metavar="password", type=str, default="", help="Password for Redis Auth")
    parser.add_argument("-c", metavar="clients", type=int, default=0, help="Number of parallel connections (0 for auto, default: 0)")
    parser.add_argument("--cores", type=str, default=f"", help="Comma-separated list of CPU cores to use (default all)")
    parser.add_argument("--cluster", action="store_true", help="Use cluster mode (default false)")
    parser.add_argument("-n", metavar="requests", type=int, default=0, help="Total number of requests (default 0), 0 for unlimited.")
    parser.add_argument("-s", metavar="seconds", type=int, default=0, help="Total time in seconds (default 0), 0 for unlimited.")
    parser.add_argument("-P", metavar="pipeline", type=int, default=1, help="Pipeline <numreq> requests. Default 1 (no pipeline).")
    # parser.add_argument("--tls", action="store_true", help="Use TLS for connection (default false)")
    parser.add_argument("--load", action="store_true", help="Only load data to Redis, no benchmark.")
    parser.add_argument('-v', '--version', action='version', version=version('resp_benchmark'))
    parser.add_argument("--help", action="help", help="Output this help and exit.")
    parser.add_argument("command", type=str, default="SET {key uniform 100000} {value 64}", nargs="?", help="The Redis command to benchmark (default SET {key uniform 100000} {value 64})")

    args = parser.parse_args()
    return args


def main():
    args = parse_args()
    bm = Benchmark(host=args.h, port=args.p, username=args.u, password=args.a, cluster=args.cluster, cores=args.cores, timeout=30)
    if args.load:
        bm.load_data(command=args.command, connections=args.c, pipeline=args.P, count=args.n)
    else:
        bm.bench(command=args.command, connections=args.c, pipeline=args.P, count=args.n, seconds=args.s)


if __name__ == "__main__":
    main()
