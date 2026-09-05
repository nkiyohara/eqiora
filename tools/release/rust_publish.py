"""Package and publish the Cargo-derived eqiora closure from one release commit."""
from __future__ import annotations

import argparse
import email.utils
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import time
import tomllib
import urllib.error
import urllib.request


class PublicationError(RuntimeError):
    pass


def publication_order(metadata: dict) -> list[dict]:
    packages = {package["name"]: package for package in metadata["packages"]}
    ordered: list[dict] = []
    visited: set[str] = set()
    active: set[str] = set()

    def visit(name: str) -> None:
        if name in active:
            raise PublicationError(f"local dependency cycle at {name}")
        if name in visited:
            return
        package = packages[name]
        allowed = package.get("publish")
        if allowed is not None and "crates-io" not in allowed:
            raise PublicationError(f"{name} does not permit crates.io publication")
        active.add(name)
        for dependency in package["dependencies"]:
            if dependency.get("path"):
                visit(dependency["name"])
        active.remove(name)
        visited.add(name)
        ordered.append(package)

    visit("eqiora")
    return ordered


def require_source(source: Path, commit: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise PublicationError("expected a full lowercase release commit")
    actual = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
    if actual != commit:
        raise PublicationError("checkout differs from the accepted release commit")
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=all"], cwd=source, text=True
    )
    if dirty:
        raise PublicationError("release checkout must be clean")


def locked_toolchain(source: Path) -> str:
    lock = tomllib.loads((source / "mise.lock").read_text())
    candidates = [entry["version"] for entry in lock["tools"]["rust"]
                  if re.fullmatch(r"\d+\.\d+\.\d+-x86_64-unknown-linux-gnu", entry["version"])]
    if len(candidates) != 1:
        raise PublicationError("mise.lock must name one exact Linux Rust toolchain")
    return candidates[0]


def archive_name(package: dict) -> str:
    return f'{package["name"]}-{package["version"]}.crate'


def checksum(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def compare_archives(packages: list[dict], expected: Path, actual: Path) -> None:
    names = {archive_name(package) for package in packages}
    if {path.name for path in expected.iterdir()} != names:
        raise PublicationError("accepted artifact inventory differs from the publication closure")
    for name in sorted(names):
        if not (expected / name).is_file() or checksum(expected / name) != checksum(actual / name):
            raise PublicationError(f"reconstructed archive differs from accepted bytes: {name}")


def require_archive_source(packages: list[dict], archives: Path, commit: str) -> None:
    for package in packages:
        name = archive_name(package)
        path = archives / name
        if path.is_symlink() or not path.is_file():
            raise PublicationError(f"expected a regular Cargo archive: {name}")
        with tarfile.open(path, "r:gz") as archive:
            member = archive.extractfile(f'{name.removesuffix(".crate")}/.cargo_vcs_info.json')
            if member is None:
                raise PublicationError(f"missing Cargo source identity: {name}")
            vcs = json.load(member)
        if vcs["git"]["sha1"] != commit or vcs["git"].get("dirty", False):
            raise PublicationError(f"archive differs from the clean release commit: {name}")


def registry_version(package: dict) -> dict | None:
    url = f'https://crates.io/api/v1/crates/{package["name"]}/{package["version"]}'
    request = urllib.request.Request(url, headers={"User-Agent": "eqiora-release-ci"})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)["version"]
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise


def require_matching_version(package: dict, record: dict, archives: Path) -> None:
    if record.get("yanked") or record["checksum"] != checksum(archives / archive_name(package)):
        raise PublicationError(f'existing registry version differs from accepted bytes: {package["name"]}')


def pending_publications(packages: list[dict], archives: Path) -> list[dict]:
    pending = []
    # Detect every conflicting immutable version before uploading anything.
    for package in packages:
        record = registry_version(package)
        if record is None:
            pending.append(package)
        else:
            require_matching_version(package, record, archives)
            print(f'Already published with matching checksum: {archive_name(package)}', flush=True)
    return pending


def retry_at(output: str) -> float | None:
    match = re.search(r"Please try again after (.*?) and see https://crates.io/docs/rate-limits", output)
    if "status 429" not in output or match is None:
        return None
    return email.utils.parsedate_to_datetime(match.group(1)).timestamp() + 2


def publish(packages: list[dict], archives: Path, source: Path, cargo: list[str], env: dict) -> None:
    for package in pending_publications(packages, archives):
        while True:
            print(f'Publishing {archive_name(package)}', flush=True)
            result = subprocess.run(
                [*cargo, "publish", "--registry", "crates-io", "--locked", "--no-verify",
                 "-p", package["name"]], cwd=source, env=env,
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
            )
            print(result.stdout, flush=True)
            record = registry_version(package)
            if record is not None:
                require_matching_version(package, record, archives)
                break
            if result.returncode == 0:
                raise PublicationError("Cargo reported publication but registry version is unavailable")
            resume = retry_at(result.stdout)
            if resume is None:
                raise PublicationError(f'cargo publish failed for {package["name"]}')
            while time.time() < resume:
                delay = min(60, resume - time.time())
                if delay > 0:
                    print(f'Waiting for crates.io rate limit: {resume - time.time():.0f}s', flush=True)
                    time.sleep(delay)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("prepare", "validate", "publish"))
    parser.add_argument("--commit", required=True)
    parser.add_argument("--artifacts", required=True, type=Path)
    args = parser.parse_args()
    source = Path.cwd().resolve()
    require_source(source, args.commit)
    scratch = Path.home().resolve() / ".cache" / "eqiora-rust-release"
    scratch.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["TMPDIR"] = str(scratch)
    env["CARGO_TARGET_DIR"] = str(scratch / "target")
    cargo = ["cargo", f"+{locked_toolchain(source)}"]
    metadata = json.loads(subprocess.check_output(
        [*cargo, "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=source, env=env
    ))
    packages = publication_order(metadata)
    actual = scratch / "target" / "package"
    if args.mode != "publish":
        command = [*cargo, "package", "--registry", "crates-io", "--locked"]
        if args.mode == "validate":
            command.append("--no-verify")
        for package in packages:
            command.extend(["-p", package["name"]])
        subprocess.run(command, cwd=source, env=env, check=True)
    require_source(source, args.commit)
    require_archive_source(packages, actual, args.commit)
    if args.mode == "prepare":
        args.artifacts.mkdir(parents=True, exist_ok=False)
        for package in packages:
            name = archive_name(package)
            shutil.copyfile(actual / name, args.artifacts / name)
    else:
        compare_archives(packages, args.artifacts, actual)
        if args.mode == "publish":
            publish(packages, args.artifacts, source, cargo, env)


if __name__ == "__main__":
    main()
