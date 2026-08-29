from __future__ import annotations

import ast
import base64
import hashlib
import importlib.metadata
import inspect
import json
import os
import shutil
import stat
import tempfile
from pathlib import Path
from typing import Callable, get_type_hints

import pytest

import eqiora


ROOT = Path(__file__).resolve().parents[3]
PRIMARY = ROOT / "verify/packages/offline-model-package/models"
PRIMARY_STORE = PRIMARY / "store"
PRIMARY_RESOLUTION_FILE = PRIMARY / "resolution.json"
EXPECTED = (
    ROOT
    / "verify/packages/offline-model-package/expected"
)
EXPECTED_TYPED = (
    ROOT
    / "verify/packages/typed-execution-lineage/expected"
)
SECONDARY = (
    ROOT
    / "verify/interfaces/python-offline-model-package/models/typed-execution-lineage"
)
HOME_SCRATCH = Path.home() / ".cache/eqiora/oracle-tests"

OFFLINE_MODEL_ID = "3JNCJVGEYX9N2QSYVEXRXWXWF4"
OFFLINE_MODEL_DIGEST = (
    "92837f0f85ff4a1310af0ca6e412d3ace81393df837d017caf5bfabeb8f6c1a1"
)
OFFLINE_RESOLUTION_DIGEST = (
    "081cca92b2a8d6ee8bba78741db2becd5d5edfa896a114a193c8e1486997b6fe"
)
OFFLINE_COMPILATION_DIGEST = (
    "c1218f8672d1a9009b91f0ea26a3c989fd6e4cc03bf645ead6ab82ff4c24276d"
)
FROZEN_A3_OFFLINE_COMPILATION_DIGEST = (
    "3a8352b7a4843266749e9b213c3f7dedf33c280afdc0d92fc42985b5a0e0a3fa"
)
TYPED_MODEL_ID = "7Q7ZYW89BV0RH2HSB3S5ZMTY0K"
TYPED_SOURCE_DIGEST = "4f3aa811b814ac7fb959f777ff5d758804e2e68593a568ee8935b122c9565462"
TYPED_RESOLUTION_DIGEST = (
    "38b5bb0c7e1f8aa7baa5e690157014a974c446f8f38fcd19d6b73b981e9ca810"
)
TYPED_MODEL_DIGEST = "c2c35e6b58f6ee0d40b8aa2bd0c252e519eec6f6779e39366ae2e28cdbd5300a"
TYPED_COMPILATION_DIGEST = (
    "ecb76b187d8d7bff8d436e3c2f0b039d42a4edc8e2f8a3bdf04863b291cf6775"
)
CURRENT_COMPILER_VERSION = "0.1.0-alpha.4"
CURRENT_COMPILATION_DIGESTS = {
    "false_claim": "c803c7d34d7239ed2d8c743783c5efbabbcba5b72f887cd9e7e4b7e5e0ca87be",
    "accepted_poisson": TYPED_COMPILATION_DIGEST,
}
LIBRARY_SOURCE = "ce343238d92f202646d2dd2947d68c311eac90aa711aa9d0e3905fa170f6f3f1"
ROOT_SOURCE = "cd7afe063d06007b97c108d3957e1bdc92e64fe47adfc7ac92975fee4f2c0d28"
CONFORMANCE = ROOT / "verify/interfaces/python-package-conformance"
FALSE_CLAIM = CONFORMANCE / "models/false-scientific-claim"
FALSE_CLAIM_STORE = FALSE_CLAIM / "store"
FALSE_CLAIM_RESOLUTION_FILE = FALSE_CLAIM / "resolution.json"
CONFORMANCE_IDENTITIES = json.loads(
    (CONFORMANCE / "expected/identities.json").read_text(encoding="utf-8")
)
CONFORMANCE_PROFILE = "eqiora.package.structural-conformance-v1"
CONFORMANCE_PACKAGE_FIELDS = (
    "name",
    "version",
    "semantic_digest",
    "source_digest",
)
CONFORMANCE_REPORT_FIELDS = (
    "profile",
    "eqiora_version",
    "compiler",
    "compiler_version",
    "semantic_canonicalization_version",
    "source_bundle_version",
    "resolution_version",
    "root_package",
    "packages",
    "entry_model",
    "resolution_digest",
    "package_compilation_digest",
    "model_id",
    "model_revision",
    "model_digest",
    "deterministic_replay_agreement",
)
EXPECTED_EQIORA_ALL = [
    "__version__",
    "Array",
    "BoundarySide",
    "CancellationError",
    "CapabilityError",
    "CompatibilityError",
    "Connection",
    "ConservingPort",
    "ConvergenceReason",
    "DerivativeImplementation",
    "Diagnostic",
    "DifferentiableEvaluation",
    "DifferentiableJvp",
    "DifferentiablePrimal",
    "DifferentiableProgram",
    "DifferentiableVjp",
    "DifferentiationEvidence",
    "DifferentiationMode",
    "Dimension",
    "DomainRef",
    "Domain",
    "EqioraError",
    "ExecutionError",
    "Expression",
    "Field",
    "FieldOutput",
    "FieldRef",
    "FormulationView",
    "FormulationKind",
    "FormulationSelectionMode",
    "InitialField",
    "InternalError",
    "LinearSolveSummary",
    "LinearizationState",
    "Model",
    "PackageConformancePackage",
    "PackageConformanceReport",
    "Parameter",
    "ParameterRef",
    "PhysicalDomain",
    "Plan",
    "Representation",
    "Relation",
    "Result",
    "Revision",
    "ResolvedExecution",
    "ScalarPlanView",
    "Run",
    "RunStatus",
    "Series",
    "State",
    "TransientRunCancellation",
    "TransientRunProgress",
    "StructuralSemanticFingerprint",
    "ValidationError",
    "ValueEdit",
    "across",
    "check_package_conformance",
    "compile",
    "compile_package",
    "connect",
    "derivative",
    "div",
    "grad",
    "lang",
    "resolve",
    "run",
    "submit",
    "through",
    "trace",
    "diff",
    "fem",
    "fluid",
    "formulation",
    "fsi",
    "fvm",
    "geometry",
    "meshing",
    "solid",
    "solve",
    "time",
    "trajectory",
]


def canonical_fixture(path: Path) -> bytes:
    stored = path.read_bytes()
    assert stored.endswith(b"\n")
    canonical = stored.removesuffix(b"\n")
    assert not canonical.endswith(b"\n")
    return canonical


def canonical_json(value: object) -> bytes:
    return json.dumps(value, separators=(",", ":")).encode("utf-8")


def source_bundle_digest(source: object) -> str:
    digest = hashlib.sha256()
    digest.update(b"eqiora.source-bundle.sha256.v1\0")
    digest.update(canonical_json(source))
    return digest.hexdigest()


PRIMARY_RESOLUTION = canonical_fixture(PRIMARY_RESOLUTION_FILE)
SECONDARY_RESOLUTION = canonical_fixture(SECONDARY / "resolution.json")
EXPECTED_MODEL = canonical_fixture(EXPECTED / "model.json")
EXPECTED_TYPED_MODEL = canonical_fixture(EXPECTED_TYPED / "model.json")
FALSE_CLAIM_RESOLUTION = canonical_fixture(FALSE_CLAIM_RESOLUTION_FILE)


def assert_compatibility(error: eqiora.CompatibilityError) -> None:
    assert error.category == "compatibility"
    assert error.diagnostics
    assert error.diagnostics[0].code == "EQ0901"


def check_conformance(
    store: object,
    resolution: bytes,
    *,
    entry_model: str = "Main",
    profile: object = CONFORMANCE_PROFILE,
) -> eqiora.PackageConformanceReport:
    return eqiora.check_package_conformance(
        store,
        resolution,
        entry_model=entry_model,
        profile=profile,
    )


def assert_conformance_rejection(
    store: Path,
    *,
    resolution: bytes = FALSE_CLAIM_RESOLUTION,
    entry_model: str = "Main",
    profile: object = CONFORMANCE_PROFILE,
    expected: type[eqiora.EqioraError] = eqiora.CompatibilityError,
) -> eqiora.EqioraError:
    before = tree_snapshot(store)
    result: object = object()
    sentinel = result
    with pytest.raises(expected) as caught:
        result = check_conformance(
            store,
            resolution,
            entry_model=entry_model,
            profile=profile,
        )
    assert result is sentinel, "a rejected check exposed a partial report"
    assert tree_snapshot(store) == before, "the structural check mutated its store"
    return caught.value


def expected_conformance_report(label: str) -> eqiora.PackageConformanceReport:
    facts = CONFORMANCE_IDENTITIES[label]
    package = eqiora.PackageConformancePackage(
        facts["name"],
        facts["version"],
        facts["semantic_identity"],
        facts["source_identity"],
    )
    return eqiora.PackageConformanceReport(
        CONFORMANCE_IDENTITIES["profile"],
        eqiora.__version__,
        CONFORMANCE_IDENTITIES["compiler"],
        CURRENT_COMPILER_VERSION,
        CONFORMANCE_IDENTITIES["semantic_canonicalization_version"],
        CONFORMANCE_IDENTITIES["source_bundle_version"],
        CONFORMANCE_IDENTITIES["resolution_version"],
        package,
        (package,),
        "Main",
        facts["resolution_identity"],
        CURRENT_COMPILATION_DIGESTS[label],
        facts["object_id"],
        facts["revision"],
        facts["canonical_identity"],
        True,
    )


def tree_snapshot(root: Path) -> tuple[tuple[object, ...], ...]:
    snapshot: list[tuple[object, ...]] = []
    for path in sorted(
        root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()
    ):
        relative = path.relative_to(root).as_posix()
        metadata = path.lstat()
        mode = stat.S_IMODE(metadata.st_mode)
        if path.is_symlink():
            snapshot.append((relative, "symlink", mode, os.readlink(path)))
        elif path.is_dir():
            snapshot.append((relative, "directory", mode))
        elif path.is_file():
            content = path.read_bytes()
            snapshot.append(
                (
                    relative,
                    "file",
                    mode,
                    len(content),
                    hashlib.sha256(content).hexdigest(),
                )
            )
        else:
            snapshot.append((relative, "nonregular", mode))
    return tuple(snapshot)


def copied_store(parent: Path, name: str = "保管庫") -> Path:
    store = parent / name
    shutil.copytree(PRIMARY_STORE, store)
    return store


def with_scratch(callback: Callable[[Path], None]) -> None:
    HOME_SCRATCH.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix="python-offline-model-package-",
        dir=HOME_SCRATCH,
    ) as directory:
        callback(Path(directory))


def test_package_conformance_public_signature_named_tuples_and_stub_are_exact() -> None:
    assert eqiora.__all__ == EXPECTED_EQIORA_ALL
    assert len(eqiora.__all__) == len(set(eqiora.__all__))
    assert "_check_package_conformance" not in eqiora.__all__

    signature = inspect.signature(eqiora.check_package_conformance)
    assert tuple(signature.parameters) == (
        "store_root",
        "resolution_bytes",
        "entry_model",
        "profile",
    )
    assert (
        signature.parameters["store_root"].kind
        is inspect.Parameter.POSITIONAL_OR_KEYWORD
    )
    assert (
        signature.parameters["resolution_bytes"].kind
        is inspect.Parameter.POSITIONAL_OR_KEYWORD
    )
    for name in ("entry_model", "profile"):
        parameter = signature.parameters[name]
        assert parameter.kind is inspect.Parameter.KEYWORD_ONLY
        assert parameter.default is inspect.Parameter.empty

    hints = get_type_hints(eqiora.check_package_conformance)
    assert hints == {
        "store_root": str | os.PathLike[str],
        "resolution_bytes": bytes,
        "entry_model": str,
        "profile": str,
        "return": eqiora.PackageConformanceReport,
    }

    assert eqiora.PackageConformancePackage.__bases__ == (tuple,)
    assert eqiora.PackageConformancePackage._fields == CONFORMANCE_PACKAGE_FIELDS
    assert eqiora.PackageConformancePackage.__match_args__ == CONFORMANCE_PACKAGE_FIELDS
    assert get_type_hints(eqiora.PackageConformancePackage) == {
        "name": str,
        "version": str,
        "semantic_digest": str,
        "source_digest": str,
    }
    assert eqiora.PackageConformanceReport.__bases__ == (tuple,)
    assert eqiora.PackageConformanceReport._fields == CONFORMANCE_REPORT_FIELDS
    assert eqiora.PackageConformanceReport.__match_args__ == CONFORMANCE_REPORT_FIELDS
    assert get_type_hints(eqiora.PackageConformanceReport) == {
        "profile": str,
        "eqiora_version": str,
        "compiler": str,
        "compiler_version": str,
        "semantic_canonicalization_version": int,
        "source_bundle_version": int,
        "resolution_version": int,
        "root_package": eqiora.PackageConformancePackage,
        "packages": tuple[eqiora.PackageConformancePackage, ...],
        "entry_model": str,
        "resolution_digest": str,
        "package_compilation_digest": str,
        "model_id": str,
        "model_revision": int,
        "model_digest": str,
        "deterministic_replay_agreement": bool,
    }

    caller_value = expected_conformance_report("false_claim")
    with pytest.raises(AttributeError):
        caller_value.profile = "changed"
    with pytest.raises(AttributeError):
        del caller_value.root_package
    with pytest.raises(TypeError):
        caller_value[0] = "changed"
    assert not hasattr(caller_value, "trust_status")
    assert not hasattr(caller_value, "attestation")
    assert not hasattr(caller_value, "to_json")

    stub_path = Path(inspect.getfile(eqiora)).with_name("__init__.pyi")
    stub = ast.parse(stub_path.read_text(encoding="utf-8"))
    classes = {node.name: node for node in stub.body if isinstance(node, ast.ClassDef)}
    for name, fields in [
        ("PackageConformancePackage", CONFORMANCE_PACKAGE_FIELDS),
        ("PackageConformanceReport", CONFORMANCE_REPORT_FIELDS),
    ]:
        assert [ast.unparse(base) for base in classes[name].bases] == ["NamedTuple"]
        declarations = {
            node.target.id: ast.unparse(node.annotation)
            for node in classes[name].body
            if isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name)
        }
        assert tuple(declarations) == fields
        assert (
            declarations
            == {
                "PackageConformancePackage": {
                    "name": "str",
                    "version": "str",
                    "semantic_digest": "str",
                    "source_digest": "str",
                },
                "PackageConformanceReport": {
                    "profile": "str",
                    "eqiora_version": "str",
                    "compiler": "str",
                    "compiler_version": "str",
                    "semantic_canonicalization_version": "int",
                    "source_bundle_version": "int",
                    "resolution_version": "int",
                    "root_package": "PackageConformancePackage",
                    "packages": "tuple[PackageConformancePackage, ...]",
                    "entry_model": "str",
                    "resolution_digest": "str",
                    "package_compilation_digest": "str",
                    "model_id": "str",
                    "model_revision": "int",
                    "model_digest": "str",
                    "deterministic_replay_agreement": "bool",
                },
            }[name]
        )
    function = next(
        node
        for node in stub.body
        if isinstance(node, ast.FunctionDef)
        and node.name == "check_package_conformance"
    )
    assert [argument.arg for argument in function.args.args] == [
        "store_root",
        "resolution_bytes",
    ]
    assert [argument.arg for argument in function.args.kwonlyargs] == [
        "entry_model",
        "profile",
    ]
    assert {
        argument.arg: ast.unparse(argument.annotation)
        for argument in [*function.args.args, *function.args.kwonlyargs]
    } == {
        "store_root": "str | os.PathLike[str]",
        "resolution_bytes": "bytes",
        "entry_model": "str",
        "profile": "str",
    }
    assert function.args.defaults == []
    assert function.args.kw_defaults == [None, None]
    assert ast.unparse(function.returns) == "PackageConformanceReport"
    stub_exports = next(
        ast.literal_eval(node.value)
        for node in stub.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name) and target.id == "__all__"
            for target in node.targets
        )
    )
    assert stub_exports == EXPECTED_EQIORA_ALL


def test_structural_reports_match_frozen_facts_without_scientific_inference() -> None:
    def run(parent: Path) -> None:
        false_store = parent / "copied-false-claim"
        shutil.copytree(FALSE_CLAIM_STORE, false_store)
        false_resolution_path = parent / "copied-false-resolution.json"
        shutil.copy2(FALSE_CLAIM_RESOLUTION_FILE, false_resolution_path)
        false_resolution = canonical_fixture(false_resolution_path)
        before = tree_snapshot(false_store)
        false_report = check_conformance(false_store, false_resolution)
        assert false_report == expected_conformance_report("false_claim")
        assert false_report.deterministic_replay_agreement is True
        assert false_report.eqiora_version == eqiora.__version__
        assert false_report.eqiora_version == importlib.metadata.version("eqiora")
        assert tree_snapshot(false_store) == before
        assert check_conformance(false_store, false_resolution) == false_report

        release_path = false_store / f"{false_report.root_package.source_digest}.json"
        release = json.loads(release_path.read_bytes())
        documentation = next(
            file
            for file in release["source"]["files"]
            if file["role"] == "documentation"
        )
        decoded_documentation = base64.b64decode(documentation["bytes"]).decode("utf-8")
        assert "every physical prediction is exact" in decoded_documentation
        assert set(false_report._asdict()) == set(CONFORMANCE_REPORT_FIELDS)
        assert all("evidence" not in field for field in false_report._fields)
        assert all("physics" not in field for field in false_report._fields)

        reordered_store = parent / "reordered-creation"
        reordered_store.mkdir()
        (reordered_store / "unlisted-before.json").write_text(
            "not part of the exact closure\n", encoding="utf-8"
        )
        for source in sorted(FALSE_CLAIM_STORE.iterdir(), reverse=True):
            shutil.copy2(source, reordered_store / source.name)
        reordered_before = tree_snapshot(reordered_store)
        assert check_conformance(reordered_store, false_resolution) == false_report
        assert tree_snapshot(reordered_store) == reordered_before

        poisson_store = parent / "copied-poisson"
        shutil.copytree(SECONDARY / "store", poisson_store)
        poisson_before = tree_snapshot(poisson_store)
        poisson_report = check_conformance(poisson_store, SECONDARY_RESOLUTION)
        assert poisson_report == expected_conformance_report("accepted_poisson")
        assert tree_snapshot(poisson_store) == poisson_before
        assert poisson_report.root_package != false_report.root_package
        assert poisson_report.resolution_digest != false_report.resolution_digest
        assert (
            poisson_report.package_compilation_digest
            != false_report.package_compilation_digest
        )
        assert poisson_report.model_digest != false_report.model_digest

        parallel_store = parent / "copied-parallel"
        shutil.copytree(PRIMARY_STORE, parallel_store)
        parallel_before = tree_snapshot(parallel_store)
        parallel_report = check_conformance(parallel_store, PRIMARY_RESOLUTION)
        parallel_compilation = json.loads(
            (EXPECTED / "historical-alpha1-compilation.json").read_bytes()
        )
        expected_packages = tuple(
            eqiora.PackageConformancePackage(
                item["package"]["name"],
                item["package"]["version"],
                item["package"]["semantic_digest"],
                item["source_digest"],
            )
            for item in parallel_compilation["packages"]
        )
        assert parallel_report.packages == expected_packages
        assert list(parallel_report.packages) == sorted(
            parallel_report.packages,
            key=lambda package: (
                package.name,
                package.version,
                package.semantic_digest,
                package.source_digest,
            ),
        )
        assert parallel_report.root_package == next(
            package
            for package in parallel_report.packages
            if package.name == parallel_compilation["root"]["name"]
            and package.version == parallel_compilation["root"]["version"]
            and package.semantic_digest
            == parallel_compilation["root"]["semantic_digest"]
        )
        assert parallel_report.resolution_digest == OFFLINE_RESOLUTION_DIGEST
        assert parallel_report.package_compilation_digest == OFFLINE_COMPILATION_DIGEST
        assert parallel_report.model_id == OFFLINE_MODEL_ID
        assert parallel_report.model_revision == 1
        assert parallel_report.model_digest == OFFLINE_MODEL_DIGEST
        assert parallel_report.deterministic_replay_agreement is True
        assert tree_snapshot(parallel_store) == parallel_before

        caller_copy = eqiora.PackageConformanceReport(*false_report)
        assert caller_copy == false_report
        assert check_conformance(false_store, false_resolution) == false_report

    with_scratch(run)


def test_profile_and_argument_shapes_fail_before_filesystem_authority() -> None:
    class UntouchedPath:
        calls = 0

        def __fspath__(self) -> str:
            self.calls += 1
            raise AssertionError("the profile rejection touched os.fspath")

    for rejected in [
        "EQIORA.PACKAGE.STRUCTURAL-CONFORMANCE-V1",
        "eqiora.package.structural-conformance-v1 ",
        "eqiora.package.structural-conformance-v2",
        "eqiora.package.structural-conformance",
        "structural-conformance-v1",
        "",
    ]:
        path = UntouchedPath()
        with pytest.raises(eqiora.CompatibilityError) as caught:
            check_conformance(path, b"not a resolution", profile=rejected)
        assert_compatibility(caught.value)
        assert path.calls == 0

    path = UntouchedPath()
    with pytest.raises(TypeError):
        check_conformance(path, b"not a resolution", profile=1)
    assert path.calls == 0

    with pytest.raises(TypeError):
        check_conformance(os.fsencode(FALSE_CLAIM_STORE), FALSE_CLAIM_RESOLUTION)

    class BytesPath:
        def __fspath__(self) -> bytes:
            return os.fsencode(FALSE_CLAIM_STORE)

    with pytest.raises(TypeError):
        check_conformance(BytesPath(), FALSE_CLAIM_RESOLUTION)

    class ResolutionBytes(bytes):
        pass

    assert check_conformance(
        FALSE_CLAIM_STORE,
        ResolutionBytes(FALSE_CLAIM_RESOLUTION),
    ) == expected_conformance_report("false_claim")

    with pytest.raises(TypeError):
        eqiora.check_package_conformance(
            FALSE_CLAIM_STORE,
            FALSE_CLAIM_RESOLUTION,
            entry_model="Main",
        )
    with pytest.raises(TypeError):
        eqiora.check_package_conformance(
            FALSE_CLAIM_STORE,
            FALSE_CLAIM_RESOLUTION,
            "Main",
            CONFORMANCE_PROFILE,
        )
    with pytest.raises(TypeError):
        eqiora.check_package_conformance(
            FALSE_CLAIM_STORE,
            FALSE_CLAIM_RESOLUTION,
            entry_model=1,
            profile=CONFORMANCE_PROFILE,
        )
    for invalid in [
        bytearray(FALSE_CLAIM_RESOLUTION),
        memoryview(FALSE_CLAIM_RESOLUTION),
        FALSE_CLAIM_RESOLUTION.decode("utf-8"),
    ]:
        with pytest.raises(TypeError):
            check_conformance(FALSE_CLAIM_STORE, invalid)


def test_resolution_wire_is_exact_before_store_and_rejects_stale_or_foreign_inputs() -> (
    None
):
    false_decoded = json.loads(FALSE_CLAIM_RESOLUTION)
    key_reordered = json.dumps(
        {
            "nodes": false_decoded["nodes"],
            "schema": false_decoded["schema"],
            "root": false_decoded["root"],
            "edges": false_decoded["edges"],
        },
        separators=(",", ":"),
    ).encode("utf-8")

    primary_decoded = json.loads(PRIMARY_RESOLUTION)
    node_reordered = dict(primary_decoded)
    node_reordered["nodes"] = list(reversed(node_reordered["nodes"]))
    node_reordered_bytes = canonical_json(node_reordered)

    root_release = json.loads((PRIMARY_STORE / f"{ROOT_SOURCE}.json").read_bytes())
    assert source_bundle_digest(root_release["source"]) == ROOT_SOURCE
    dependencies = root_release["source"]["manifest"]["dependencies"]
    assert len(dependencies) == 1
    spare_dependency = {
        "alias": "spare",
        "target": dependencies[0]["target"],
    }
    dependencies.append(spare_dependency)
    two_edge = json.loads(PRIMARY_RESOLUTION)
    root_node = next(
        node for node in two_edge["nodes"] if node["identity"] == two_edge["root"]
    )
    root_node["source_digest"] = source_bundle_digest(root_release["source"])
    spare_edge = {
        "declaring": two_edge["edges"][0]["declaring"],
        "alias": "spare",
        "target": two_edge["edges"][0]["target"],
    }
    two_edge["edges"].append(spare_edge)
    two_edge_bytes = canonical_json(two_edge)
    assert len(two_edge["edges"]) == 2

    edge_reordered = json.loads(two_edge_bytes)
    edge_reordered["edges"].reverse()
    edge_reordered_bytes = canonical_json(edge_reordered)
    assert edge_reordered_bytes != two_edge_bytes
    edge_reordered["edges"].reverse()
    assert canonical_json(edge_reordered) == two_edge_bytes

    class UntouchedPath:
        calls = 0

        def __fspath__(self) -> str:
            self.calls += 1
            raise AssertionError("noncanonical resolution touched os.fspath")

    for invalid in [
        b" " + FALSE_CLAIM_RESOLUTION,
        FALSE_CLAIM_RESOLUTION_FILE.read_bytes(),
        key_reordered,
        node_reordered_bytes,
        edge_reordered_bytes,
        b"{not-json",
        b"{}",
    ]:
        path = UntouchedPath()
        with pytest.raises(eqiora.CompatibilityError) as caught:
            check_conformance(path, invalid)
        assert_compatibility(caught.value)
        assert path.calls == 0

    def run(parent: Path) -> None:
        two_edge_store = parent / "two-edge"
        two_edge_store.mkdir()
        library_path = PRIMARY_STORE / f"{LIBRARY_SOURCE}.json"
        shutil.copy2(library_path, two_edge_store / library_path.name)
        two_edge_root = two_edge_store / f"{root_node['source_digest']}.json"
        two_edge_root.write_bytes(canonical_json(root_release))
        two_edge_before = tree_snapshot(two_edge_store)
        two_edge_report = check_conformance(two_edge_store, two_edge_bytes)
        assert two_edge_report.root_package.source_digest == root_node["source_digest"]
        assert tuple(package.name for package in two_edge_report.packages) == (
            "Eqiora.Electrical.Basic",
            "org.example.parallel",
        )
        assert tree_snapshot(two_edge_store) == two_edge_before

        store = parent / "stale-inputs"
        shutil.copytree(FALSE_CLAIM_STORE, store)

        digest_mismatch = json.loads(FALSE_CLAIM_RESOLUTION)
        digest_mismatch["nodes"][0]["source_digest"] = "0" * 64
        mismatch_bytes = json.dumps(digest_mismatch, separators=(",", ":")).encode(
            "utf-8"
        )
        assert_compatibility(
            assert_conformance_rejection(store, resolution=mismatch_bytes)
        )

        stale_identity = json.loads(FALSE_CLAIM_RESOLUTION)
        stale_identity["root"]["semantic_digest"] = "1" * 64
        stale_identity["nodes"][0]["identity"]["semantic_digest"] = "1" * 64
        stale_bytes = json.dumps(stale_identity, separators=(",", ":")).encode("utf-8")
        assert_compatibility(
            assert_conformance_rejection(store, resolution=stale_bytes)
        )

        assert_compatibility(
            assert_conformance_rejection(store, resolution=SECONDARY_RESOLUTION)
        )

    with_scratch(run)


def test_release_normalization_accepts_representation_but_rejects_semantic_changes_and_roles() -> (
    None
):
    expected = expected_conformance_report("false_claim")

    def run(parent: Path) -> None:
        normalized = parent / "normalized-release"
        shutil.copytree(FALSE_CLAIM_STORE, normalized)
        release_path = normalized / f"{expected.root_package.source_digest}.json"
        release = json.loads(release_path.read_bytes())
        source = release["source"]
        manifest = source["manifest"]
        represented = {
            "source": {
                "files": list(reversed(source["files"])),
                "manifest": {
                    "bundle": list(reversed(manifest["bundle"])),
                    "dependencies": list(reversed(manifest["dependencies"])),
                    "version": manifest["version"],
                    "name": manifest["name"],
                    "schema": manifest["schema"],
                },
                "package": source["package"],
                "schema": source["schema"],
            },
            "semantic": {
                "declarations": list(reversed(release["semantic"]["declarations"])),
                "schema": release["semantic"]["schema"],
            },
            "schema": release["schema"],
        }
        release_path.write_text(
            json.dumps(represented, indent=2) + "\n", encoding="utf-8"
        )
        represented_before = tree_snapshot(normalized)
        assert check_conformance(normalized, FALSE_CLAIM_RESOLUTION) == expected
        assert tree_snapshot(normalized) == represented_before

        source_changed = parent / "source-changed"
        shutil.copytree(FALSE_CLAIM_STORE, source_changed)
        source_path = source_changed / release_path.name
        source_release = json.loads(source_path.read_bytes())
        model_source = next(
            file
            for file in source_release["source"]["files"]
            if file["role"] == "model_source"
        )
        decoded = base64.b64decode(model_source["bytes"])
        model_source["bytes"] = base64.b64encode(decoded + b"\n").decode("ascii")
        source_path.write_bytes(
            json.dumps(source_release, separators=(",", ":")).encode("utf-8")
        )
        assert_compatibility(assert_conformance_rejection(source_changed))

        foreign = parent / "foreign-release"
        shutil.copytree(FALSE_CLAIM_STORE, foreign)
        foreign_path = foreign / release_path.name
        typed_release = next((SECONDARY / "store").glob("*.json"))
        foreign_path.write_bytes(typed_release.read_bytes())
        assert_compatibility(assert_conformance_rejection(foreign))

        for role in ["executable", "plugin"]:
            hostile = parent / f"role-{role}"
            shutil.copytree(FALSE_CLAIM_STORE, hostile)
            hostile_path = hostile / release_path.name
            hostile_release = json.loads(hostile_path.read_bytes())
            assert source_bundle_digest(hostile_release["source"]) == hostile_path.stem
            hostile_file = hostile_release["source"]["files"][0]
            hostile_file["role"] = role
            hostile_bundle = next(
                entry
                for entry in hostile_release["source"]["manifest"]["bundle"]
                if entry["path"] == hostile_file["path"]
            )
            hostile_bundle["role"] = role
            hostile_source = hostile_release["source"]
            hostile_digest = source_bundle_digest(hostile_source)
            hostile_path.unlink()
            hostile_path = hostile / f"{hostile_digest}.json"
            hostile_path.write_bytes(canonical_json(hostile_release))
            hostile_resolution = json.loads(FALSE_CLAIM_RESOLUTION)
            hostile_resolution["nodes"][0]["source_digest"] = hostile_digest
            assert hostile_bundle["role"] == hostile_file["role"] == role
            assert hostile_path.stem == hostile_resolution["nodes"][0]["source_digest"]
            assert_compatibility(
                assert_conformance_rejection(
                    hostile,
                    resolution=canonical_json(hostile_resolution),
                )
            )

        alias_changed = parent / "alias-changed"
        shutil.copytree(PRIMARY_STORE, alias_changed)
        alias_path = alias_changed / f"{ROOT_SOURCE}.json"
        alias_release = json.loads(alias_path.read_bytes())
        alias_release["source"]["manifest"]["dependencies"][0]["alias"] = "changed"
        alias_path.write_bytes(
            json.dumps(alias_release, separators=(",", ":")).encode("utf-8")
        )
        alias_error = assert_conformance_rejection(
            alias_changed,
            resolution=PRIMARY_RESOLUTION,
        )
        assert_compatibility(alias_error)

    with_scratch(run)


@pytest.mark.parametrize(
    "selector",
    ["", "Missing", "dep.Main", "org.example.poisson.Main", "wave_number"],
)
def test_conformance_selector_is_bare_root_local_without_a_visibility_policy(
    selector: str,
) -> None:
    error = assert_conformance_rejection(
        FALSE_CLAIM_STORE,
        entry_model=selector,
        expected=eqiora.ValidationError,
    )
    assert error.category == "validation"
    assert error.diagnostics
    assert (
        check_conformance(FALSE_CLAIM_STORE, FALSE_CLAIM_RESOLUTION).entry_model
        == "Main"
    )


def test_conformance_filesystem_authority_and_atomicity_are_exact() -> None:
    def run(parent: Path) -> None:
        accepted = parent / "accepted"
        shutil.copytree(FALSE_CLAIM_STORE, accepted)
        (accepted / "unlisted.json").write_text("ignored\n", encoding="utf-8")
        before = tree_snapshot(accepted)
        assert check_conformance(
            accepted, FALSE_CLAIM_RESOLUTION
        ) == expected_conformance_report("false_claim")
        assert tree_snapshot(accepted) == before

        missing = parent / "missing"
        shutil.copytree(FALSE_CLAIM_STORE, missing)
        next(missing.glob("*.json")).unlink()
        assert_compatibility(assert_conformance_rejection(missing))

        nonregular = parent / "nonregular"
        shutil.copytree(FALSE_CLAIM_STORE, nonregular)
        nonregular_entry = next(nonregular.glob("*.json"))
        nonregular_entry.unlink()
        nonregular_entry.mkdir()
        assert_compatibility(assert_conformance_rejection(nonregular))

        absent = parent / "absent"
        with pytest.raises(eqiora.CompatibilityError) as missing_root:
            check_conformance(absent, FALSE_CLAIM_RESOLUTION)
        assert_compatibility(missing_root.value)

        regular = parent / "regular-root"
        regular.write_text("not a directory\n", encoding="utf-8")
        with pytest.raises(eqiora.CompatibilityError) as regular_root:
            check_conformance(regular, FALSE_CLAIM_RESOLUTION)
        assert_compatibility(regular_root.value)

        if os.name == "posix":
            exact_link = parent / "exact-link"
            shutil.copytree(FALSE_CLAIM_STORE, exact_link)
            linked_entry = next(exact_link.glob("*.json"))
            outside = parent / "outside-release.json"
            linked_entry.replace(outside)
            linked_entry.symlink_to(outside)
            assert_compatibility(assert_conformance_rejection(exact_link))

            real = parent / "real-store"
            shutil.copytree(FALSE_CLAIM_STORE, real)
            root_link = parent / "root-link"
            root_link.symlink_to(real, target_is_directory=True)
            real_before = tree_snapshot(real)
            with pytest.raises(eqiora.CompatibilityError) as linked_root:
                check_conformance(root_link, FALSE_CLAIM_RESOLUTION)
            assert_compatibility(linked_root.value)
            assert tree_snapshot(real) == real_before

    with_scratch(run)
