#!/usr/bin/env python3
"""Project the exact-head CLI, control-v2, and MCP interfaces into MDX."""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


MCP_PROTOCOL = "2026-07-28"
SOURCE_SHA_PATTERN = re.compile(r"[0-9a-f]{40}", flags=re.ASCII)
GIT_IDENTITY_ENVIRONMENT = {
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
}
OUTPUTS = {
    "cli": Path("docs/site/src/content/docs/reference/cli/index.mdx"),
    "control": Path("docs/site/src/content/docs/reference/control-v2/index.mdx"),
    "mcp": Path("docs/site/src/content/docs/reference/mcp/index.mdx"),
}


class ProjectionError(RuntimeError):
    """A live interface or committed projection violates the documentation contract."""


def _regular_file(path: Path, label: str, *, executable: bool = False) -> Path:
    try:
        details = path.stat()
    except FileNotFoundError as error:
        raise ProjectionError(f"missing {label}: {path}") from error
    if not stat.S_ISREG(details.st_mode):
        raise ProjectionError(f"{label} must be a regular file: {path}")
    if executable and details.st_mode & 0o111 == 0:
        raise ProjectionError(f"{label} is not executable: {path}")
    return path.resolve(strict=True)


def _text(path: Path, label: str) -> str:
    source = _regular_file(path, label)
    try:
        payload = source.read_bytes()
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProjectionError(f"{label} is not UTF-8: {path}") from error
    if b"\r" in payload or not text.endswith("\n"):
        raise ProjectionError(f"{label} must be LF-only and end in one LF: {path}")
    return text


def _command_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment.update(
        {
            "COLUMNS": "100",
            "LANG": "C",
            "LC_ALL": "C",
            "NO_COLOR": "1",
            "TERM": "dumb",
            "TZ": "UTC",
        }
    )
    return environment


def _git_identity_observation(
    repository: Path, arguments: list[str], label: str
) -> str:
    environment = os.environ.copy()
    for name in GIT_IDENTITY_ENVIRONMENT:
        environment.pop(name, None)
    try:
        completed = subprocess.run(
            ["git", "-C", str(repository), *arguments],
            cwd=repository,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProjectionError(f"could not observe {label}: {error}") from error
    if completed.returncode != 0:
        raise ProjectionError(f"could not observe {label}")
    if completed.stderr:
        raise ProjectionError(f"Git wrote to stderr while observing {label}")
    try:
        output = completed.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProjectionError(f"Git emitted non-UTF-8 {label}") from error
    if (
        b"\r" in completed.stdout
        or not output.endswith("\n")
        or output.count("\n") != 1
    ):
        raise ProjectionError(f"Git emitted malformed {label}")
    return output.removesuffix("\n")


def _admit_source_identity(repository: Path, source_sha: str) -> None:
    if SOURCE_SHA_PATTERN.fullmatch(source_sha) is None:
        raise ProjectionError(
            "source SHA must be exactly 40 lowercase hexadecimal characters"
        )
    environment_sha = os.environ.get("EQIORA_SITE_SOURCE_SHA")
    if environment_sha is not None and environment_sha != source_sha:
        raise ProjectionError("source SHA disagrees with EQIORA_SITE_SOURCE_SHA")

    if not os.path.lexists(repository / ".git"):
        return

    top_level_text = _git_identity_observation(
        repository, ["rev-parse", "--show-toplevel"], "canonical Git top level"
    )
    top_level_path = Path(top_level_text)
    try:
        top_level = top_level_path.resolve(strict=True)
    except OSError as error:
        raise ProjectionError(
            "Git top level is not a canonical existing path"
        ) from error
    if (
        not top_level_path.is_absolute()
        or top_level_text != str(top_level)
        or top_level != repository
    ):
        raise ProjectionError("Git top level disagrees with the repository root")

    head = _git_identity_observation(
        repository, ["rev-parse", "--verify", "HEAD"], "Git HEAD"
    )
    if SOURCE_SHA_PATTERN.fullmatch(head) is None:
        raise ProjectionError("Git HEAD is not a canonical 40-character commit")
    if head != source_sha:
        raise ProjectionError("Git HEAD disagrees with the source SHA")


def _run(
    binary: Path,
    arguments: list[str],
    *,
    cwd: Path,
    label: str,
) -> str:
    try:
        completed = subprocess.run(
            [str(binary), *arguments],
            cwd=cwd,
            env=_command_environment(),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProjectionError(f"could not capture {label}: {error}") from error
    if completed.returncode != 0:
        raise ProjectionError(f"{label} exited with {completed.returncode}")
    if completed.stderr:
        raise ProjectionError(f"{label} wrote to stderr")
    if len(completed.stdout) > 2 * 1024 * 1024:
        raise ProjectionError(f"{label} exceeded the documentation capture limit")
    try:
        output = completed.stdout.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ProjectionError(f"{label} did not emit UTF-8") from error
    if b"\r" in completed.stdout or not output.endswith("\n"):
        raise ProjectionError(f"{label} must emit LF-terminated output")
    return output


def _workspace_version(repository: Path) -> str:
    cargo_path = repository / "Cargo.toml"
    cargo = tomllib.loads(_text(cargo_path, "workspace manifest"))
    try:
        version = cargo["workspace"]["package"]["version"]
    except (KeyError, TypeError) as error:
        raise ProjectionError("Cargo.toml has no workspace.package.version") from error
    if not isinstance(version, str) or not version:
        raise ProjectionError("workspace.package.version is not a non-empty string")
    return version


def _capture_cli(repository: Path, binary: Path, version: str) -> dict[str, str]:
    cwd = repository.parent
    root_help = _run(binary, ["--help"], cwd=cwd, label="eqiora --help")
    check_help = _run(binary, ["check", "--help"], cwd=cwd, label="eqiora check --help")
    observed_version = _run(binary, ["--version"], cwd=cwd, label="eqiora --version")
    expected_version = f"eqiora {version}\n"
    if observed_version != expected_version:
        raise ProjectionError(
            "eqiora --version disagrees with workspace.package.version: "
            f"expected {expected_version!r}, observed {observed_version!r}"
        )
    return {
        "root_help": root_help,
        "check_help": check_help,
        "version": observed_version,
    }


def _mcp_request(identifier: str, method: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": identifier,
        "method": method,
        "params": {
            "_meta": {
                "io.modelcontextprotocol/protocolVersion": MCP_PROTOCOL,
                "io.modelcontextprotocol/clientCapabilities": {},
            }
        },
    }


def _compact_json(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _capture_mcp(repository: Path, binary: Path, version: str) -> dict[str, Any]:
    requests = [
        _mcp_request("docs-discover", "server/discover"),
        _mcp_request("docs-tools", "tools/list"),
    ]
    request_bytes = b"".join(_compact_json(request) + b"\n" for request in requests)
    try:
        completed = subprocess.run(
            [str(binary)],
            cwd=repository.parent,
            env=_command_environment(),
            input=request_bytes,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ProjectionError(
            f"could not capture live MCP discovery/list: {error}"
        ) from error
    if completed.returncode != 0:
        raise ProjectionError(f"eqiora-mcp exited with {completed.returncode}")
    if completed.stderr:
        raise ProjectionError("eqiora-mcp wrote to stderr during discovery/list")
    if b"\r" in completed.stdout or not completed.stdout.endswith(b"\n"):
        raise ProjectionError("eqiora-mcp responses must be LF-terminated")
    response_lines = completed.stdout.splitlines()
    if len(response_lines) != 2 or any(
        len(line) > 2 * 1024 * 1024 for line in response_lines
    ):
        raise ProjectionError(
            "eqiora-mcp must return exactly two bounded response lines"
        )

    responses: list[dict[str, Any]] = []
    for request, line in zip(requests, response_lines, strict=True):
        try:
            response = json.loads(line)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise ProjectionError("eqiora-mcp returned a non-JSON response") from error
        if not isinstance(response, dict):
            raise ProjectionError("eqiora-mcp response is not a JSON object")
        if response.get("jsonrpc") != "2.0" or response.get("id") != request["id"]:
            raise ProjectionError(
                "eqiora-mcp response identity disagrees with the request"
            )
        if "error" in response or not isinstance(response.get("result"), dict):
            raise ProjectionError("eqiora-mcp discovery/list did not return a result")
        responses.append(response)

    discover = responses[0]["result"]
    listed = responses[1]["result"]
    server_info = discover.get("_meta", {}).get("io.modelcontextprotocol/serverInfo")
    if server_info != {"name": "eqiora-mcp", "version": version}:
        raise ProjectionError(
            "live MCP serverInfo disagrees with the exact build identity"
        )
    if MCP_PROTOCOL not in discover.get("supportedVersions", []):
        raise ProjectionError(f"live MCP discovery does not support {MCP_PROTOCOL}")
    tools = listed.get("tools")
    if not isinstance(tools, list) or len(tools) != 1:
        raise ProjectionError("live MCP tools/list must return exactly one tool")
    if tools[0].get("name") != "eqiora.model.compile_check":
        raise ProjectionError("live MCP tools/list returned an unexpected tool")
    return {
        "requests": requests,
        "responses": responses,
        "tool_name": tools[0]["name"],
    }


def _validate_mcp_prose_authority(repository: Path) -> None:
    manifest_path = repository / "verify/interfaces/mcp-stdio-compile-check/case.toml"
    readme_path = repository / "verify/interfaces/mcp-stdio-compile-check/README.md"
    manifest = tomllib.loads(_text(manifest_path, "MCP evidence manifest"))
    readme = _text(readme_path, "MCP evidence README")
    boundary = manifest.get("claim_boundary")
    if not isinstance(boundary, dict):
        raise ProjectionError("MCP evidence manifest has no claim_boundary")
    required = {
        "local_stdio": True,
        "mcp_response_cancellation": "best-effort",
        "remote_transport": False,
    }
    if any(boundary.get(name) != value for name, value in required.items()):
        raise ProjectionError(
            "MCP evidence manifest no longer admits the projected framing prose"
        )
    capabilities = manifest.get("capabilities")
    if not isinstance(capabilities, list) or "mcp-2026-07-28-stdio" not in capabilities:
        raise ProjectionError(
            "MCP evidence manifest no longer admits MCP 2026-07-28 stdio"
        )
    for phrase in (
        "one thin local subprocess projection",
        "newline-delimited stdio",
        "one-active-call resource policy",
        "best-effort response cancellation",
    ):
        if phrase not in readme:
            raise ProjectionError(
                f"MCP evidence README no longer contains authority phrase: {phrase}"
            )


def _json_block(value: Any) -> str:
    rendered = json.dumps(value, ensure_ascii=False, indent=2)
    if "```" in rendered:
        raise ProjectionError(
            "captured JSON cannot be represented in the fixed MDX fence"
        )
    return rendered


def _code_block(value: str) -> str:
    rendered = value.removesuffix("\n")
    if "```" in rendered:
        raise ProjectionError(
            "captured command output cannot be represented in the fixed MDX fence"
        )
    return rendered


def _cli_page(cli: dict[str, str]) -> str:
    return f"""---
title: Command-line interface
description: Exact-head help and version for the bounded local Eqiora compile/check command.
---

import {{ Aside }} from '@astrojs/starlight/components';
import ExactSourceLink from '@components/site/ExactSourceLink.astro';

{{/* Generated by tools/docs/generate_interface_reference.py; do not edit. */}}

<Aside type="note" title="Exact-head projection">
  These blocks come from the built `eqiora` binary for this documentation commit. They are not copied from a test fixture.
</Aside>

## Version

```console
$ eqiora --version
{_code_block(cli["version"])}
```

## Command help

```console
$ eqiora --help
{_code_block(cli["root_help"])}
```

## `check` help

```console
$ eqiora check --help
{_code_block(cli["check_help"])}
```

## Usage notes

`eqiora check` validates and compiles one local regular Model source file
supplied as a path.

- <ExactSourceLink kind="blob" path="verify/interfaces/cli-compile-check/case.toml">CLI verification case</ExactSourceLink>
- <ExactSourceLink kind="blob" path="verify/interfaces/cli-compile-check/README.md">CLI verification details</ExactSourceLink>
"""


def _control_page(schema_text: str, schema: dict[str, Any]) -> str:
    title = schema.get("title")
    identifier = schema.get("$id")
    protocol = (
        schema.get("$defs", {})
        .get("request", {})
        .get("properties", {})
        .get("protocol", {})
        .get("const")
    )
    command = (
        schema.get("$defs", {})
        .get("request", {})
        .get("properties", {})
        .get("command", {})
        .get("const")
    )
    if not all(
        isinstance(value, str) and value
        for value in (title, identifier, protocol, command)
    ):
        raise ProjectionError("control-v2 schema lacks its required public identities")
    if "```" in schema_text:
        raise ProjectionError(
            "control-v2 schema cannot be represented in the fixed MDX fence"
        )
    return f"""---
title: Control protocol v2
description: Exact JSON Schema for the bounded compile/check control request and response.
---

import {{ Aside }} from '@astrojs/starlight/components';
import ExactSourceLink from '@components/site/ExactSourceLink.astro';

{{/* Generated by tools/docs/generate_interface_reference.py; do not edit. */}}

<Aside type="note" title="Exact contract">
  This page embeds the tracked schema bytes. Final site assembly publishes the same file as a download.
</Aside>

## Identity

| Field | Value |
| --- | --- |
| Title | `{title}` |
| Schema ID | `{identifier}` |
| Protocol | `{protocol}` |
| Command | `{command}` |

[Download `compile-v2.schema.json`](/reference/control-v2/compile-v2.schema.json)

<ExactSourceLink kind="blob" path="schemas/control/compile-v2.schema.json">View the exact schema source</ExactSourceLink>

## JSON Schema

```json
{schema_text.removesuffix(chr(10))}
```

## Usage notes

Control-v2 carries compile/check requests and structured responses in a closed,
transport-neutral JSON schema.

- <ExactSourceLink kind="blob" path="verify/interfaces/control-plane-compile-check/case.toml">Control-v2 verification case</ExactSourceLink>
- <ExactSourceLink kind="blob" path="verify/interfaces/control-plane-compile-check/README.md">Control-v2 verification details</ExactSourceLink>
"""


def _mcp_page(mcp: dict[str, Any]) -> str:
    discover_request, list_request = mcp["requests"]
    discover_response, list_response = mcp["responses"]
    return f"""---
title: MCP stdio interface
description: Live discovery and tool-list projection for Eqiora's bounded local MCP adapter.
---

import {{ Aside }} from '@astrojs/starlight/components';
import ExactSourceLink from '@components/site/ExactSourceLink.astro';

{{/* Generated by tools/docs/generate_interface_reference.py; do not edit. */}}

<Aside type="note" title="Live local observation">
  The requests below were sent to one exact-head `eqiora-mcp` subprocess. The responses are live output, not expected fixtures.
</Aside>

This bounded adapter uses MCP {MCP_PROTOCOL} over newline-delimited stdio and lists exactly one tool, `{mcp["tool_name"]}`. The admitted framing boundary is one local subprocess, bounded framing and metadata, one active call, and best-effort response cancellation.

## Discover the server

Request:

```json
{_json_block(discover_request)}
```

Live response:

```json
{_json_block(discover_response)}
```

## List tools

Request:

```json
{_json_block(list_request)}
```

Live response:

```json
{_json_block(list_response)}
```

## Usage notes

Eqiora's compile/check adapter communicates over local stdio and exposes the
tools returned by the live tool list above.

- <ExactSourceLink kind="blob" path="verify/interfaces/mcp-stdio-compile-check/case.toml">MCP verification case</ExactSourceLink>
- <ExactSourceLink kind="blob" path="verify/interfaces/mcp-stdio-compile-check/README.md">MCP verification details</ExactSourceLink>
"""


def _render(repository: Path, eqiora_binary: Path, mcp_binary: Path) -> dict[str, str]:
    version = _workspace_version(repository)
    cli = _capture_cli(repository, eqiora_binary, version)
    schema_text = _text(
        repository / "schemas/control/compile-v2.schema.json", "control-v2 schema"
    )
    try:
        schema = json.loads(schema_text)
    except json.JSONDecodeError as error:
        raise ProjectionError(f"control-v2 schema is not JSON: {error}") from error
    if not isinstance(schema, dict):
        raise ProjectionError("control-v2 schema is not a JSON object")
    _validate_mcp_prose_authority(repository)
    mcp = _capture_mcp(repository, mcp_binary, version)
    return {
        "cli": _cli_page(cli),
        "control": _control_page(schema_text, schema),
        "mcp": _mcp_page(mcp),
    }


def _write_or_check(repository: Path, rendered: dict[str, str], *, check: bool) -> None:
    failures: list[str] = []
    for name, relative in OUTPUTS.items():
        target = repository / relative
        payload = rendered[name].encode("utf-8")
        if b"\r" in payload or not payload.endswith(b"\n"):
            raise ProjectionError(
                f"generated {name} projection is not canonical UTF-8/LF"
            )
        if check:
            try:
                observed = target.read_bytes()
            except FileNotFoundError:
                failures.append(f"missing generated projection: {relative}")
                continue
            if observed != payload:
                failures.append(f"generated projection is stale: {relative}")
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(payload)
    if failures:
        raise ProjectionError("\n".join(failures))


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repository",
        type=Path,
        default=Path(__file__).resolve().parents[2],
        help="repository root containing Cargo.toml and the output paths",
    )
    parser.add_argument("--eqiora-binary", type=Path, required=True)
    parser.add_argument("--mcp-binary", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument(
        "--check",
        action="store_true",
        help="compare generated bytes without changing the checkout",
    )
    return parser


def main() -> int:
    arguments = _parser().parse_args()
    repository = arguments.repository.resolve(strict=True)
    _admit_source_identity(repository, arguments.source_sha)
    eqiora_binary = _regular_file(
        arguments.eqiora_binary, "eqiora binary", executable=True
    )
    mcp_binary = _regular_file(
        arguments.mcp_binary, "eqiora-mcp binary", executable=True
    )
    rendered = _render(repository, eqiora_binary, mcp_binary)
    _write_or_check(repository, rendered, check=arguments.check)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ProjectionError as error:
        print(f"interface reference projection failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
