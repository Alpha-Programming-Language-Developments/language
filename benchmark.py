#!/usr/bin/env python3
"""
Alpha Multi-Language Benchmark Suite
Instantly compares execution speeds of Alpha, C, C++, Rust, Zig, and Go.
"""

import os
import sys
import time
import shutil
import subprocess

# Ensure UTF-8 output on Windows consoles
if hasattr(sys.stdout, "reconfigure"):
    try:
        sys.stdout.reconfigure(encoding="utf-8")
    except Exception:
        pass

DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "target", "benchmark")
os.makedirs(DIR, exist_ok=True)

# -----------------------------------------------------------------------------
# Benchmark Source Codes (fib(32) computation)
# -----------------------------------------------------------------------------

SOURCES = {
    "C": (
        os.path.join(DIR, "bench.c"),
        os.path.join(DIR, "bench_c.exe"),
        ["gcc", "-O3", os.path.join(DIR, "bench.c"), "-o", os.path.join(DIR, "bench_c.exe")],
        r"""#include <stdio.h>
long long fib(long long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
int main() {
    printf("%lld\n", fib(32));
    return 0;
}
""",
    ),
    "C++": (
        os.path.join(DIR, "bench.cpp"),
        os.path.join(DIR, "bench_cpp.exe"),
        ["g++", "-O3", os.path.join(DIR, "bench.cpp"), "-o", os.path.join(DIR, "bench_cpp.exe")],
        r"""#include <iostream>
long long fib(long long n) {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
int main() {
    std::cout << fib(32) << "\n";
    return 0;
}
""",
    ),
    "Rust": (
        os.path.join(DIR, "bench.rs"),
        os.path.join(DIR, "bench_rs.exe"),
        ["rustc", "-O", os.path.join(DIR, "bench.rs"), "-o", os.path.join(DIR, "bench_rs.exe")],
        r"""fn fib(n: i64) -> i64 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() {
    println!("{}", fib(32));
}
""",
    ),
    "Zig": (
        os.path.join(DIR, "bench.zig"),
        os.path.join(DIR, "bench_zig.exe"),
        ["zig", "build-exe", "-O", "ReleaseFast", os.path.join(DIR, "bench.zig"), f"-femit-bin={os.path.join(DIR, 'bench_zig.exe')}"],
        r"""const std = @import("std");
fn fib(n: i64) i64 {
    if (n <= 1) return n;
    return fib(n - 1) + fib(n - 2);
}
pub fn main() !void {
    const stdout = std.io.getStdOut().writer();
    try stdout.print("{d}\n", .{fib(32)});
}
""",
    ),
    "Go": (
        os.path.join(DIR, "bench.go"),
        os.path.join(DIR, "bench_go.exe"),
        ["go", "build", "-o", os.path.join(DIR, "bench_go.exe"), os.path.join(DIR, "bench.go")],
        r"""package main
import "fmt"
func fib(n int64) int64 {
	if n <= 1 { return n }
	return fib(n-1) + fib(n-2)
}
func main() {
	fmt.Println(fib(32))
}
""",
    ),
    "Alpha": (
        os.path.join(DIR, "bench.alpha"),
        os.path.join(DIR, "build", "bench.exe"),
        ["alpha", "build", os.path.join(DIR, "bench.alpha")],
        r"""import Core/Basic.alpha

function fib(n: int) [here, int] {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

function main() [public, int] {
    var res: int = fib(32)
    print(res)
    return 0
}
""",
    ),
}

EXPECTED_OUTPUT = "2178309"

def ensure_sources_and_binaries(force_rebuild=False):
    for lang, (src_path, exe_path, compile_cmd, code) in SOURCES.items():
        # Always make sure source is up-to-date
        needs_write = not os.path.exists(src_path) or force_rebuild
        if not needs_write:
            with open(src_path, "r", encoding="utf-8") as f:
                if f.read().strip() != code.strip():
                    needs_write = True

        if needs_write:
            with open(src_path, "w", encoding="utf-8") as f:
                f.write(code)

        if exe_path and compile_cmd:
            if force_rebuild or not os.path.exists(exe_path) or needs_write:
                compiler = compile_cmd[0]
                if not shutil.which(compiler):
                    continue
                subprocess.run(compile_cmd, capture_output=True, text=True)

def run_single(cmd, runs=3):
    # Warmup run: ensures Windows Defender scan & OS page cache are warm
    subprocess.run(cmd, capture_output=True, text=True)

    durations = []
    output = ""
    for _ in range(runs):
        t0 = time.perf_counter()
        res = subprocess.run(cmd, capture_output=True, text=True)
        t1 = time.perf_counter()
        if res.returncode == 0:
            durations.append((t1 - t0) * 1000.0)
            combined = (res.stdout + " " + res.stderr).strip()
            output = combined
        else:
            return None, (res.stdout + " " + res.stderr).strip()
    return min(durations), output

def main():
    force_rebuild = "--rebuild" in sys.argv
    runs_count = 1 if "--quick" in sys.argv else 2

    print("\n" + "=" * 68)
    print("      MULTI-LANGUAGE HARDWARE BENCHMARK: fib(32)")
    print("=" * 68)

    t_start = time.perf_counter()
    ensure_sources_and_binaries(force_rebuild)

    results = []

    for lang, (src_path, exe_path, _, _) in SOURCES.items():
        if not exe_path or not os.path.exists(exe_path):
            continue
        cmd = [exe_path]

        best_ms, out = run_single(cmd, runs=runs_count)
        if best_ms is not None:
            valid = EXPECTED_OUTPUT in out
            results.append({
                "lang": lang,
                "time_ms": best_ms,
                "valid": valid
            })
        else:
            print(f"  [-] Failed to run {lang}: {out}")

    # Sort fastest to slowest
    results.sort(key=lambda x: x["time_ms"])

    fastest_time = results[0]["time_ms"] if results else 1.0

    print("-" * 68)
    print(f"{'RANK':<8} {'LANGUAGE':<14} {'TOTAL TIME':<18} {'RELATIVE':<12} {'RESULT'}")
    print("-" * 68)

    for i, r in enumerate(results):
        rank = f"#{i+1}"
        rel = f"{r['time_ms'] / fastest_time:.2f}x"
        status = "PASS (2178309)" if r["valid"] else "FAIL"
        print(f"{rank:<8} {r['lang']:<14} {r['time_ms']:>8.2f} ms        {rel:<12} {status}")

    t_total = (time.perf_counter() - t_start)
    print("-" * 68)
    print(f"  Total benchmark wall-clock time: {t_total:.2f} seconds.")
    print("  Run 'python benchmark.py --rebuild' to force recompilation.\n")

if __name__ == "__main__":
    main()
