import argparse
import subprocess
from pathlib import Path

FUZZ_DIR = Path(".")
CARGO_TOML = Path("../Cargo.toml")

LEVEL_RUNS = {
    "default": 10_000,
    "medium": 50_000,
    "high": 200_000,
    "extreme": 1_000_000,
}

SLOW_TARGETS = {"fuzz_media_duration", "fuzz_media_dimensions"}


def get_runs(target, level):
    base = LEVEL_RUNS.get(level, 10_000)
    if target in SLOW_TARGETS:
        return max(100, base // 10)
    return base


def main():
    parser = argparse.ArgumentParser(description="Run fuzz targets")
    parser.add_argument(
        "--level",
        "-l",
        default="default",
        choices=LEVEL_RUNS.keys(),
        help="Fuzzing intensity level",
    )
    parser.add_argument(
        "--continue-on-failure", action="store_true", help="Don't stop on first crash"
    )
    parser.add_argument("--target", "-t", help="Run only this target")
    args = parser.parse_args()

    # Auto‑register any new targets
    targets = [f.stem for f in FUZZ_DIR.glob("*.rs")]
    if args.target:
        if args.target not in targets:
            print(f"Target {args.target} not found.")
            return
        targets = [args.target]

    cargo_content = CARGO_TOML.read_text()
    new_entries = []
    for target in targets:
        bin_header = f'[[bin]]\nname = "{target}"'
        if bin_header not in cargo_content:
            new_entries.append(f"""
[[bin]]
name = "{target}"
path = "fuzz_targets/{target}.rs"
test = false
doc = false
""")
    if new_entries:
        print("Auto‑registering new fuzz targets in fuzz/Cargo.toml...")
        with open(CARGO_TOML, "a") as f:
            f.writelines(new_entries)

    targets.sort(key=lambda t: 1 if t in SLOW_TARGETS else 0)

    for target in targets:
        runs = get_runs(target, args.level)
        print(f"\n🔍 Running {target} (level={args.level}, runs={runs})")
        result = subprocess.run(
            ["cargo", "fuzz", "run", target, "--", f"-runs={runs}"], cwd=".."
        )
        if result.returncode != 0:
            print(f"❌ {target} crashed or failed.")
            if not args.continue_on_failure:
                print("Stopping (use --continue-on-failure to ignore).")
                break
            else:
                print("Continuing as requested...")


if __name__ == "__main__":
    main()
