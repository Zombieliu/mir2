"""Run the exact Rust release/refresh Lua against an isolated loopback Redis.

Creates only random playtest-cache-test-* keys and deletes only those keys.
No SCAN, FLUSH, credentials, production data, or translated Lua implementation.
"""
import argparse
import concurrent.futures
import hashlib
import json
import pathlib
import re
import secrets
import socket
import threading


def command(port, *args):
    values = [str(value).encode() for value in args]
    request = b"*%d\r\n" % len(values) + b"".join(
        b"$%d\r\n" % len(value) + value + b"\r\n" for value in values
    )
    with socket.create_connection(("127.0.0.1", port), timeout=5) as connection:
        connection.sendall(request)
        with connection.makefile("rb") as stream:
            line = stream.readline()
            kind, value = line[:1], line[1:-2]
            if kind == b"+":
                return value.decode()
            if kind == b"-":
                raise RuntimeError(value.decode())
            if kind == b":":
                return int(value)
            if kind == b"$":
                size = int(value)
                if size == -1:
                    return None
                result = stream.read(size)
                assert stream.read(2) == b"\r\n"
                return result.decode()
            raise RuntimeError("Unexpected Redis response type")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cache-source", required=True)
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--output")
    options = parser.parse_args()
    if not 1024 <= options.port <= 65535 or options.port == 6379:
        raise ValueError("Use an explicit isolated non-default loopback Redis port")
    source = pathlib.Path(options.cache_source).read_text(encoding="utf-8")
    release = re.search(r'const RELEASE_OWNED_SESSION_ROUTE_LUA: &str = r#"(.*?)"#;', source, re.S).group(1)
    refresh_candidates = [value for value in re.findall(r'let script = r#"(.*?)"#;', source, re.S)
                          if '"SETEX", KEYS[2]' in value and '"EXPIRE", KEYS[1]' in value]
    assert len(refresh_candidates) == 1, "Expected the exact backend refresh Lua"
    refresh = refresh_candidates[0]
    prefix = "playtest-cache-test-" + secrets.token_hex(12)
    record_key, lease_key, index_key, other_key = [prefix + suffix for suffix in (":record", ":lease", ":character", ":other")]
    keys = [record_key, lease_key, index_key, other_key]
    receipts = []

    def redis(*args):
        return command(options.port, *args)

    def record(owner):
        return json.dumps({"routeLeaseOwner": owner, "gatewaySessionId": owner, "characterName": "LuaFixture"})

    def reset(owner="old-owner", lease=True):
        redis("DEL", *keys)
        redis("SET", record_key, record(owner), "EX", 120)
        redis("SET", index_key, record_key, "EX", 120)
        if lease:
            redis("SET", lease_key, owner, "EX", 120)

    def remove(owner="old-owner"):
        return redis("EVAL", release, 3, record_key, lease_key, index_key, owner)

    def refresh_old():
        return redis("EVAL", refresh, 3, lease_key, record_key, index_key, "old-owner", 120, 120, record("old-owner"))

    try:
        assert redis("PING") == "PONG"
        reset()
        assert remove() == 1
        assert all(redis("GET", key) is None for key in keys)
        assert remove() == 0
        assert refresh_old() == 0
        receipts.append("normal-release-idempotent-and-late-refresh-cannot-resurrect")

        reset()
        redis("DEL", record_key)
        assert remove() == 1 and redis("GET", lease_key) is None
        receipts.append("missing-record-still-releases-owned-lease")

        reset()
        redis("SET", lease_key, "new-owner", "EX", 120)
        assert remove() == 0
        assert redis("GET", lease_key) == "new-owner"
        assert redis("GET", record_key) == record("old-owner")
        assert refresh_old() == -1
        receipts.append("new-owner-lease-protected-before-record-publication")

        reset("new-owner")
        assert remove() == 0 and redis("GET", record_key) == record("new-owner")
        assert refresh_old() == -1
        receipts.append("new-owner-record-and-lease-protected-from-old-cleanup-and-refresh")

        reset()
        redis("SET", index_key, other_key, "EX", 120)
        assert remove() == 1 and redis("GET", index_key) == other_key
        receipts.append("unrelated-character-index-not-deleted")

        reset()
        redis("SET", record_key, "invalid-json", "EX", 120)
        try:
            remove()
            raise AssertionError("Malformed record must fail closed")
        except RuntimeError:
            assert redis("GET", lease_key) == "old-owner"
        receipts.append("malformed-record-fails-before-any-deletion")

        reset()
        redis("DEL", index_key)
        redis("LPUSH", index_key, "invalid-index-type")
        try:
            remove()
            raise AssertionError("Malformed index must fail closed")
        except RuntimeError:
            assert redis("GET", lease_key) == "old-owner"
            assert redis("GET", record_key) == record("old-owner")
        receipts.append("malformed-index-fails-before-any-deletion")

        for _ in range(16):
            reset()
            barrier = threading.Barrier(2)

            def simultaneous(action):
                barrier.wait(timeout=5)
                return action()

            with concurrent.futures.ThreadPoolExecutor(2) as pool:
                removing = pool.submit(simultaneous, remove)
                refreshing = pool.submit(simultaneous, refresh_old)
                assert removing.result() == 1
                assert refreshing.result() in (0, 1)
            assert all(redis("GET", key) is None for key in (record_key, lease_key, index_key))
        receipts.append("16-real-concurrent-old-refresh-versus-release-races")

        for _ in range(16):
            reset(lease=False)
            barrier = threading.Barrier(2)

            def new_owner():
                assert redis("SET", lease_key, "new-owner", "NX", "EX", 120) == "OK"
                redis("SET", record_key, record("new-owner"), "EX", 120)
                redis("SET", index_key, record_key, "EX", 120)

            with concurrent.futures.ThreadPoolExecutor(2) as pool:
                removing = pool.submit(simultaneous, remove)
                acquiring = pool.submit(simultaneous, new_owner)
                assert removing.result() in (0, 1)
                acquiring.result()
            assert redis("GET", lease_key) == "new-owner"
            assert redis("GET", record_key) == record("new-owner")
            assert remove() == 0 and refresh_old() == -1
        receipts.append("16-real-concurrent-new-owner-versus-old-release-races")
    finally:
        redis("DEL", *keys)

    result = {"ok": True, "backend": "real-redis-loopback", "port": options.port,
              "namespace": prefix, "cleanup": "only-four-owned-keys",
              "releaseLuaSha256": hashlib.sha256(release.encode()).hexdigest(),
              "refreshLuaSha256": hashlib.sha256(refresh.encode()).hexdigest(), "checks": receipts}
    encoded = json.dumps(result, indent=2)
    if options.output:
        pathlib.Path(options.output).write_text(encoded + "\n", encoding="utf-8")
    print(encoded)


if __name__ == "__main__":
    main()
