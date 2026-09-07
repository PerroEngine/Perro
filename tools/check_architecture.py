"""Check engine dependency direction without resolving external dependencies."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
IMPLEMENTATIONS = {"perro_runtime", "perro_app", "perro_headless", "perro_dev_runner", "perro_cli", "perro_graphics", "perro_compiler", "perro_static_pipeline"}

def main():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, text=True
    ))
    packages = {p["name"]: p for p in metadata["packages"]}
    graph = {name: {d["name"] for d in p["dependencies"] if d["name"] in packages and d["kind"] != "dev"} for name, p in packages.items()}
    errors = []
    for name, package in packages.items():
        path = package["manifest_path"].replace("\\", "/")
        foundation = any(part in path for part in ("/core/", "/api_modules/", "/script_stack/", "/io_stack/")) or name in {"perro_render_bridge", "perro_runtime_render", "perro_physics"}
        if foundation:
            for dependency in sorted(graph[name] & IMPLEMENTATIONS):
                errors.append(f"{name} -> {dependency}: contract/data crate depends on implementation")
        if name == "perro_graphics":
            for dependency in sorted(graph[name] & (IMPLEMENTATIONS - {"perro_graphics"})):
                errors.append(f"{name} -> {dependency}: graphics must communicate through the render bridge")
    visited, active = set(), []
    def visit(name):
        if name in active:
            errors.append("dependency cycle: " + " -> ".join(active[active.index(name):] + [name]))
            return
        if name in visited:
            return
        active.append(name)
        for child in sorted(graph[name]):
            visit(child)
        active.pop()
        visited.add(name)
    for name in sorted(graph):
        visit(name)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Architecture check: {len(packages)} crates; dependency direction + cycles pass")
    return 0

if __name__ == "__main__":
    sys.exit(main())
