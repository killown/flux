import subprocess
from pathlib import Path

fuzz_dir = Path(".")
cargo_toml_path = Path("../Cargo.toml")

targets = [f.stem for f in fuzz_dir.glob("*.rs")]
cargo_content = cargo_toml_path.read_text()

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
    print("Auto-registering new fuzz targets in fuzz/Cargo.toml...")
    with open(cargo_toml_path, "a") as f:
        f.writelines(new_entries)

# Define lower priority / slow targets (like media probing using disk/subprocess I/O)
low_priority_targets = {"fuzz_media_duration", "fuzz_media_dimensions"}

# Separate targets so fast/high-priority ones run first
sorted_targets = sorted(targets, key=lambda t: 1 if t in low_priority_targets else 0)

for target in sorted_targets:
    # Assign fewer runs to slow media targets, and full runs to high priority parsers
    runs = 100 if target in low_priority_targets else 10000

    print(f"\nRunning cargo fuzz for: {target} (runs={runs})")
    result = subprocess.run(
        ["cargo", "fuzz", "run", target, "--", f"-runs={runs}"], cwd=".."
    )
    if result.returncode != 0:
        print(f"Fuzz target {target} failed or found a crash.")
        break
