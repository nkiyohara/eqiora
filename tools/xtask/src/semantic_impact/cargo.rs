use std::collections::{BTreeMap, BTreeSet};

use toml::{Table, Value};

use super::model::{
    AnalysisCaps, AnalysisFailure, CargoAuthorityRecord, CargoDependencyKind, CargoTargetKind,
    CfgProfile, Declaration, DependencyJoin, DependencyTemplate, ExactAtom, ExactRepoPath,
    FeatureRef, NormalizedCargoTarget, RevisionPoint, SortedSet, external_dependency_join,
    normalize_repo_path, package_id, parse_feature_ref, target_id, validate_edition,
    validate_feature_atom, validate_library_types, validate_package_name, validate_target_name,
};
use super::repository::{
    TreeImage, WorkspaceDocuments, auto_enabled, expect_bool, expect_string, explicit_target_root,
    inherited_string, insert_authority_record, optional_bool, optional_string_set, required_string,
    required_table, string_set, workspace_dependency,
};

type CargoResult<T> = Result<T, AnalysisFailure>;

#[derive(Clone)]
struct Package {
    name: String,
    id: String,
    manifest: String,
    directory: String,
    table: Table,
    features: BTreeMap<String, Vec<FeatureRef>>,
}

pub(super) fn semantic_records(
    documents: &WorkspaceDocuments,
    tree: &TreeImage,
    revision: &RevisionPoint,
    profile: &CfgProfile,
    caps: &AnalysisCaps,
) -> CargoResult<SortedSet<CargoAuthorityRecord>> {
    let root_package = required_table(&documents.root, "workspace")?
        .get("package")
        .and_then(Value::as_table)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let packages = derive_packages(documents, root_package, caps)?;
    let mut records = BTreeSet::new();
    let mut target_count = 0_u64;
    let mut derived_bytes = 0_u64;
    for package in &packages {
        insert_authority_record(
            &mut records,
            CargoAuthorityRecord::Package {
                revision: revision.clone(),
                package_id: ExactAtom(package.id.clone()),
                package_name: ExactAtom(package.name.clone()),
                manifest_path: ExactRepoPath(package.manifest.clone()),
            },
            caps,
            &mut derived_bytes,
        )?;
        for target in derive_targets(package, tree, caps)? {
            target_count = target_count
                .checked_add(1)
                .ok_or(AnalysisFailure::CapBeforeSafeFallback)?;
            if target_count > caps.max_cargo_targets_per_side {
                return Err(AnalysisFailure::CapBeforeSafeFallback);
            }
            let id = target_id(&package.id, &target.kind, &target.name, &target.root);
            insert_authority_record(
                &mut records,
                CargoAuthorityRecord::Target {
                    revision: revision.clone(),
                    package_id: ExactAtom(package.id.clone()),
                    target_id: id,
                    cfg_profile: profile.clone(),
                    target_kind: target.kind,
                    crate_root: ExactRepoPath(target.root),
                },
                caps,
                &mut derived_bytes,
            )?;
        }
    }
    let declarations = derive_declarations(documents, &packages, caps)?;
    records.extend(resolve_dependencies(
        &packages,
        &declarations,
        revision,
        profile,
        caps,
        &mut derived_bytes,
    )?);
    Ok(records)
}

fn derive_packages(
    documents: &WorkspaceDocuments,
    workspace_package: &Table,
    caps: &AnalysisCaps,
) -> CargoResult<Vec<Package>> {
    let workspace_version = required_string(workspace_package, "version")?;
    let workspace_edition = required_string(workspace_package, "edition")?;
    validate_edition(workspace_edition)?;
    let mut packages = Vec::with_capacity(documents.members.len());
    let mut ids = BTreeSet::new();
    for document in &documents.members {
        if packages.len() as u64 >= caps.max_workspace_packages_per_side {
            return Err(AnalysisFailure::CapBeforeSafeFallback);
        }
        let package_table = required_table(&document.table, "package")?;
        let name = required_string(package_table, "name")?.to_owned();
        validate_package_name(&name, caps.max_atom_bytes)?;
        let version = inherited_string(package_table.get("version"), workspace_version)?;
        let edition = inherited_string(package_table.get("edition"), workspace_edition)?;
        validate_edition(edition)?;
        let id = package_id(&name, version, &document.path.0).0;
        if !ids.insert(id.clone()) {
            return Err(AnalysisFailure::AuthorityConflict);
        }
        let directory = document
            .path
            .0
            .strip_suffix("/Cargo.toml")
            .unwrap_or("")
            .to_owned();
        let mut package = Package {
            name,
            id,
            manifest: document.path.0.clone(),
            directory,
            table: document.table.clone(),
            features: parse_features(&document.table, caps)?,
        };
        add_implicit_optional_features(&mut package, caps)?;
        packages.push(package);
    }
    packages.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(packages)
}

fn derive_targets(
    package: &Package,
    tree: &TreeImage,
    caps: &AnalysisCaps,
) -> CargoResult<BTreeSet<NormalizedCargoTarget>> {
    let package_table = required_table(&package.table, "package")?;
    let mut targets = BTreeSet::new();
    if auto_enabled(package_table, "autolib")? {
        let root = package_path(package, "src/lib.rs", caps)?;
        if tree.is_regular(&root) {
            insert_target(
                &mut targets,
                NormalizedCargoTarget {
                    kind: CargoTargetKind::Library,
                    name: package.name.replace('-', "_"),
                    root,
                },
            )?;
        }
    }
    for (kind, area, enabled) in [
        (
            CargoTargetKind::Binary,
            "src/bin",
            auto_enabled(package_table, "autobins")?,
        ),
        (
            CargoTargetKind::IntegrationTest,
            "tests",
            auto_enabled(package_table, "autotests")?,
        ),
        (
            CargoTargetKind::Example,
            "examples",
            auto_enabled(package_table, "autoexamples")?,
        ),
        (
            CargoTargetKind::Benchmark,
            "benches",
            auto_enabled(package_table, "autobenches")?,
        ),
    ] {
        if enabled {
            for (name, root) in implicit_named_roots(package, tree, area, caps)? {
                insert_target(
                    &mut targets,
                    NormalizedCargoTarget {
                        kind: kind.clone(),
                        name,
                        root,
                    },
                )?;
            }
        }
    }
    if auto_enabled(package_table, "autobins")? {
        let root = package_path(package, "src/main.rs", caps)?;
        if tree.is_regular(&root) {
            insert_target(
                &mut targets,
                NormalizedCargoTarget {
                    kind: CargoTargetKind::Binary,
                    name: package.name.clone(),
                    root,
                },
            )?;
        }
    }
    add_build_script(package, package_table, tree, caps, &mut targets)?;
    if let Some(value) = package.table.get("lib") {
        let table = value
            .as_table()
            .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
        let name = table
            .get("name")
            .map(expect_string)
            .transpose()?
            .map(str::to_owned)
            .unwrap_or_else(|| package.name.replace('-', "_"));
        let root = explicit_root(package, table, "src/lib.rs", tree, caps)?;
        let kind = normalize_lib_kind(table)?;
        validate_target_table(table, &kind, &package.features)?;
        targets.retain(|target| target.kind != CargoTargetKind::Library);
        insert_target(&mut targets, NormalizedCargoTarget { kind, name, root })?;
    }
    for (key, kind, area) in [
        ("bin", CargoTargetKind::Binary, "src/bin"),
        ("test", CargoTargetKind::IntegrationTest, "tests"),
        ("example", CargoTargetKind::Example, "examples"),
        ("bench", CargoTargetKind::Benchmark, "benches"),
    ] {
        apply_explicit_targets(package, tree, caps, key, kind, area, &mut targets)?;
    }
    let mut roots = BTreeMap::new();
    for target in &targets {
        validate_target_name(&target.name, caps.max_atom_bytes)?;
        if roots
            .insert(target.root.clone(), target.name.clone())
            .is_some()
        {
            return Err(AnalysisFailure::AuthorityConflict);
        }
    }
    Ok(targets)
}

fn apply_explicit_targets(
    package: &Package,
    tree: &TreeImage,
    caps: &AnalysisCaps,
    key: &str,
    kind: CargoTargetKind,
    area: &str,
    targets: &mut BTreeSet<NormalizedCargoTarget>,
) -> CargoResult<()> {
    let Some(value) = package.table.get(key) else {
        return Ok(());
    };
    let entries = value
        .as_array()
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let mut explicit = BTreeSet::new();
    for value in entries {
        let table = value
            .as_table()
            .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
        let name = required_string(table, "name")?.to_owned();
        validate_target_table(table, &kind, &package.features)?;
        let root = if let Some(path) = table.get("path") {
            tree.require_regular(package_path(package, expect_string(path)?, caps)?)?
        } else {
            infer_explicit_root(package, tree, caps, &kind, area, &name)?
        };
        let target = NormalizedCargoTarget {
            kind: kind.clone(),
            name,
            root,
        };
        if !explicit.insert((target.kind.clone(), target.name.clone())) {
            return Err(AnalysisFailure::AuthorityConflict);
        }
        targets.retain(|existing| existing.kind != target.kind || existing.name != target.name);
        insert_target(targets, target)?;
    }
    Ok(())
}

fn infer_explicit_root(
    package: &Package,
    tree: &TreeImage,
    caps: &AnalysisCaps,
    kind: &CargoTargetKind,
    area: &str,
    name: &str,
) -> CargoResult<String> {
    let mut candidates = Vec::new();
    if *kind == CargoTargetKind::Binary && name == package.name {
        let root = package_path(package, "src/main.rs", caps)?;
        if tree.is_regular(&root) {
            candidates.push(root)
        }
    }
    for relative in [
        format!("{area}/{name}.rs"),
        format!("{area}/{name}/main.rs"),
    ] {
        let root = package_path(package, &relative, caps)?;
        if tree.is_regular(&root) {
            candidates.push(root)
        }
    }
    match candidates.len() {
        1 => Ok(candidates.remove(0)),
        0 => Err(AnalysisFailure::RequiredCoverageMissing),
        _ => Err(AnalysisFailure::AuthorityConflict),
    }
}

fn implicit_named_roots(
    package: &Package,
    tree: &TreeImage,
    area: &str,
    caps: &AnalysisCaps,
) -> CargoResult<Vec<(String, String)>> {
    let prefix = package_path(package, area, caps)? + "/";
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in tree.regular_paths() {
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        let name = if let Some(name) = rest.strip_suffix(".rs").filter(|name| !name.contains('/')) {
            Some(name)
        } else {
            rest.strip_suffix("/main.rs")
                .filter(|name| !name.contains('/'))
        };
        if let Some(name) = name {
            by_name
                .entry(name.to_owned())
                .or_default()
                .push(path.to_owned());
        }
    }
    let mut roots = Vec::new();
    for (name, mut paths) in by_name {
        if paths.len() != 1 {
            return Err(AnalysisFailure::AuthorityConflict);
        }
        roots.push((name, paths.remove(0)));
    }
    Ok(roots)
}

fn add_build_script(
    package: &Package,
    table: &Table,
    tree: &TreeImage,
    caps: &AnalysisCaps,
    targets: &mut BTreeSet<NormalizedCargoTarget>,
) -> CargoResult<()> {
    let root = match table.get("build") {
        Some(Value::Boolean(false)) => None,
        Some(Value::String(path)) => {
            Some(tree.require_regular(package_path(package, path, caps)?)?)
        }
        Some(_) => return Err(AnalysisFailure::RequiredCoverageMissing),
        None => {
            let path = package_path(package, "build.rs", caps)?;
            tree.is_regular(&path).then_some(path)
        }
    };
    if let Some(root) = root {
        insert_target(
            targets,
            NormalizedCargoTarget {
                kind: CargoTargetKind::BuildScript,
                name: "build-script-build".to_owned(),
                root,
            },
        )?;
    }
    Ok(())
}

fn normalize_lib_kind(table: &Table) -> CargoResult<CargoTargetKind> {
    let proc_macro = optional_bool(table, "proc-macro")?;
    let crate_types = optional_string_set(table, "crate-type")?;
    if crate_types.as_ref().is_some_and(BTreeSet::is_empty) {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    let proc_types = crate_types
        .as_ref()
        .is_some_and(|types| types == &BTreeSet::from(["proc-macro".to_owned()]));
    if proc_macro == Some(false) && proc_types {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    if proc_macro == Some(true) || proc_types {
        if crate_types.as_ref().is_some_and(|_| !proc_types) {
            return Err(AnalysisFailure::RequiredCoverageMissing);
        }
        return Ok(CargoTargetKind::ProcMacro);
    }
    validate_library_types(crate_types.as_ref())?;
    Ok(CargoTargetKind::Library)
}

fn validate_target_table(
    table: &Table,
    kind: &CargoTargetKind,
    features: &BTreeMap<String, Vec<FeatureRef>>,
) -> CargoResult<()> {
    if table.keys().any(|key| {
        !matches!(
            key.as_str(),
            "name"
                | "path"
                | "crate-type"
                | "proc-macro"
                | "required-features"
                | "test"
                | "doctest"
                | "bench"
                | "doc"
                | "harness"
                | "edition"
        )
    }) {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    for key in ["test", "doctest", "bench", "doc", "harness"] {
        if table.contains_key(key) {
            optional_bool(table, key)?;
        }
    }
    if let Some(edition) = table.get("edition") {
        validate_edition(expect_string(edition)?)?
    }
    let required = optional_string_set(table, "required-features")?.unwrap_or_default();
    if matches!(kind, CargoTargetKind::Library | CargoTargetKind::ProcMacro)
        && table.contains_key("required-features")
    {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    if required
        .iter()
        .any(|feature| !features.contains_key(feature))
    {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    match kind {
        CargoTargetKind::Library | CargoTargetKind::ProcMacro => {}
        CargoTargetKind::Example => {
            if table.contains_key("proc-macro") {
                return Err(AnalysisFailure::RequiredCoverageMissing);
            }
            let types = optional_string_set(table, "crate-type")?;
            if types
                .as_ref()
                .is_some_and(|set| set != &BTreeSet::from(["bin".to_owned()]))
            {
                validate_library_types(types.as_ref())?;
            }
        }
        _ if table.contains_key("proc-macro") || table.contains_key("crate-type") => {
            return Err(AnalysisFailure::RequiredCoverageMissing);
        }
        _ => {}
    }
    Ok(())
}

fn derive_declarations(
    documents: &WorkspaceDocuments,
    packages: &[Package],
    caps: &AnalysisCaps,
) -> CargoResult<Vec<Declaration>> {
    let root_workspace = required_table(&documents.root, "workspace")?;
    let root_dependencies = root_workspace
        .get("dependencies")
        .and_then(Value::as_table)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let templates = dependency_templates(root_dependencies, packages, caps)?;
    let mut declarations = Vec::new();
    for package in packages {
        if package.table.contains_key("target") {
            return Err(AnalysisFailure::RequiredCoverageMissing);
        }
        for (key, kind) in [
            ("dependencies", CargoDependencyKind::Normal),
            ("build-dependencies", CargoDependencyKind::Build),
            ("dev-dependencies", CargoDependencyKind::Development),
        ] {
            let Some(table) = package.table.get(key).and_then(Value::as_table) else {
                continue;
            };
            for (alias, value) in table {
                if declarations.len() as u64 >= caps.max_dependency_edges_per_side {
                    return Err(AnalysisFailure::CapBeforeSafeFallback);
                }
                validate_feature_atom(alias, caps.max_atom_bytes)?;
                let declaration = normalize_declaration(
                    package, alias, value, &kind, &templates, packages, caps,
                )?;
                declarations.push(declaration);
            }
        }
    }
    declarations.sort();
    for group in
        declarations.chunk_by(|left, right| left.owner == right.owner && left.alias == right.alias)
    {
        if group.iter().any(|item| item.join != group[0].join) {
            return Err(AnalysisFailure::AuthorityConflict);
        }
    }
    validate_feature_refs(packages, &declarations)?;
    Ok(declarations)
}

fn dependency_templates(
    table: &Table,
    packages: &[Package],
    caps: &AnalysisCaps,
) -> CargoResult<BTreeMap<String, DependencyTemplate>> {
    let mut templates = BTreeMap::new();
    for (alias, value) in table {
        validate_feature_atom(alias, caps.max_atom_bytes)?;
        if value
            .as_table()
            .is_some_and(|table| table.contains_key("optional") || table.contains_key("workspace"))
        {
            return Err(AnalysisFailure::RequiredCoverageMissing);
        }
        let raw = dependency_value(alias, value, "", packages, caps)?;
        templates.insert(
            alias.clone(),
            DependencyTemplate {
                join: raw.0,
                features: raw.1,
                uses_default_features: raw.2,
            },
        );
    }
    Ok(templates)
}

fn normalize_declaration(
    package: &Package,
    alias: &str,
    value: &Value,
    kind: &CargoDependencyKind,
    templates: &BTreeMap<String, DependencyTemplate>,
    packages: &[Package],
    caps: &AnalysisCaps,
) -> CargoResult<Declaration> {
    let (join, mut features, mut defaults) = if workspace_dependency(value)? {
        let template = templates
            .get(alias)
            .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
        (
            template.join.clone(),
            template.features.clone(),
            template.uses_default_features,
        )
    } else {
        dependency_value(alias, value, &package.directory, packages, caps)?
    };
    let table = value.as_table();
    if let Some(local) = table.and_then(|table| table.get("features")) {
        features.extend(string_set(local)?);
    }
    if let Some(local) = table.and_then(|table| table.get("default-features")) {
        defaults = expect_bool(local)?;
    }
    let optional = table
        .and_then(|table| table.get("optional"))
        .map(expect_bool)
        .transpose()?
        .unwrap_or(false);
    let package_name = match &join {
        DependencyJoin::Workspace { package_id } => packages
            .iter()
            .find(|item| &item.id == package_id)
            .map(|item| item.name.as_str()),
        DependencyJoin::ExternalRegistry { package_name, .. } => Some(package_name.as_str()),
    }
    .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    let renamed = (alias != package_name).then(|| alias.to_owned());
    Ok(Declaration {
        owner: package.id.clone(),
        alias: alias.to_owned(),
        join,
        rename: renamed,
        kind: kind.clone(),
        cfg_expression: None,
        optional,
        uses_default_features: defaults,
        requested_features: features,
    })
}

fn dependency_value(
    alias: &str,
    value: &Value,
    directory: &str,
    packages: &[Package],
    caps: &AnalysisCaps,
) -> CargoResult<(DependencyJoin, BTreeSet<String>, bool)> {
    if let Value::String(version) = value {
        return Ok((
            external_dependency_join(version, alias, caps.max_atom_bytes)?,
            BTreeSet::new(),
            true,
        ));
    }
    let table = value
        .as_table()
        .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
    if table.keys().any(|key| {
        !matches!(
            key.as_str(),
            "workspace"
                | "package"
                | "path"
                | "version"
                | "features"
                | "default-features"
                | "optional"
        )
    }) {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    if table
        .get("workspace")
        .is_some_and(|value| value.as_bool() != Some(true))
    {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    let package_name = table
        .get("package")
        .map(expect_string)
        .transpose()?
        .unwrap_or(alias);
    let join = if let Some(path) = table.get("path") {
        let manifest =
            normalize_repo_path(directory, expect_string(path)?, caps.max_repo_path_bytes)?
                + "/Cargo.toml";
        let matches: Vec<_> = packages
            .iter()
            .filter(|item| item.manifest == manifest && item.name == package_name)
            .collect();
        if matches.len() != 1 {
            return Err(AnalysisFailure::RequiredCoverageMissing);
        }
        DependencyJoin::Workspace {
            package_id: matches[0].id.clone(),
        }
    } else {
        let version = table
            .get("version")
            .map(expect_string)
            .transpose()?
            .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
        external_dependency_join(version, package_name, caps.max_atom_bytes)?
    };
    let features = table
        .get("features")
        .map(string_set)
        .transpose()?
        .unwrap_or_default();
    let defaults = table
        .get("default-features")
        .map(expect_bool)
        .transpose()?
        .unwrap_or(true);
    Ok((join, features, defaults))
}

fn resolve_dependencies(
    packages: &[Package],
    declarations: &[Declaration],
    revision: &RevisionPoint,
    profile: &CfgProfile,
    caps: &AnalysisCaps,
    derived_bytes: &mut u64,
) -> CargoResult<BTreeSet<CargoAuthorityRecord>> {
    let eligible: Vec<bool> = declarations
        .iter()
        .map(|declaration| {
            declaration.kind != CargoDependencyKind::Development
                || profile.cfg_atoms.contains(&ExactAtom("test".to_owned()))
        })
        .collect();
    let mut active: Vec<bool> = declarations
        .iter()
        .zip(&eligible)
        .map(|(declaration, eligible)| *eligible && !declaration.optional)
        .collect();
    let mut features: BTreeMap<String, BTreeSet<String>> = packages
        .iter()
        .map(|package| {
            let mut set = BTreeSet::new();
            if package.features.contains_key("default") {
                set.insert("default".to_owned());
            }
            (package.id.clone(), set)
        })
        .collect();
    loop {
        let before = (active.clone(), features.clone());
        for package in packages {
            let names: Vec<_> = features[&package.id].iter().cloned().collect();
            for name in names {
                for feature_ref in package.features.get(&name).into_iter().flatten() {
                    apply_feature_ref(
                        feature_ref,
                        &package.id,
                        declarations,
                        &eligible,
                        &mut active,
                        &mut features,
                    )?;
                }
            }
        }
        for (index, declaration) in declarations
            .iter()
            .enumerate()
            .filter(|(index, _)| active[*index])
        {
            if let DependencyJoin::Workspace { package_id } = &declaration.join {
                for feature in &declaration.requested_features {
                    insert_existing_feature(packages, &mut features, package_id, feature)?;
                }
                if declaration.uses_default_features
                    && package_by_id(packages, package_id)?
                        .features
                        .contains_key("default")
                {
                    features
                        .get_mut(package_id)
                        .expect("known package")
                        .insert("default".to_owned());
                }
            }
            let _ = index;
        }
        if before == (active.clone(), features.clone()) {
            break;
        }
    }
    let mut records = BTreeSet::new();
    for (index, declaration) in declarations.iter().enumerate() {
        if !active[index] || !eligible[index] {
            continue;
        }
        let DependencyJoin::Workspace { package_id } = &declaration.join else {
            continue;
        };
        let active_features = features[package_id]
            .iter()
            .map(|value| ExactAtom(value.clone()))
            .collect();
        insert_authority_record(
            &mut records,
            CargoAuthorityRecord::Dependency {
                revision: revision.clone(),
                dependent_package_id: ExactAtom(declaration.owner.clone()),
                dependency_package_id: ExactAtom(package_id.clone()),
                dependency_kind: declaration.kind.clone(),
                rename: declaration.rename.clone().map(ExactAtom),
                optional: declaration.optional,
                active_features,
                cfg_expression: None,
                cfg_value: true,
            },
            caps,
            derived_bytes,
        )?;
    }
    Ok(records)
}

fn apply_feature_ref(
    feature_ref: &FeatureRef,
    owner: &str,
    declarations: &[Declaration],
    eligible: &[bool],
    active: &mut [bool],
    features: &mut BTreeMap<String, BTreeSet<String>>,
) -> CargoResult<()> {
    let (alias, feature, activates) = match feature_ref {
        FeatureRef::Local(local) => {
            features
                .get_mut(owner)
                .expect("known owner")
                .insert(local.clone());
            return Ok(());
        }
        FeatureRef::ActivateOptional(alias) => (alias, None, true),
        FeatureRef::ForwardStrong(alias, feature) => (alias, Some(feature), true),
        FeatureRef::ForwardWeak(alias, feature) => (alias, Some(feature), false),
    };
    for (index, declaration) in declarations
        .iter()
        .enumerate()
        .filter(|(_, declaration)| declaration.owner == owner && declaration.alias == *alias)
    {
        if activates && eligible[index] && declaration.optional {
            active[index] = true
        }
        if active[index]
            && let (Some(feature), DependencyJoin::Workspace { package_id }) =
                (feature, &declaration.join)
        {
            let target = features
                .get_mut(package_id)
                .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
            target.insert((*feature).clone());
        }
    }
    Ok(())
}

fn parse_features(
    table: &Table,
    caps: &AnalysisCaps,
) -> CargoResult<BTreeMap<String, Vec<FeatureRef>>> {
    let Some(features) = table.get("features").and_then(Value::as_table) else {
        return Ok(BTreeMap::new());
    };
    let mut result = BTreeMap::new();
    for (name, value) in features {
        validate_feature_atom(name, caps.max_atom_bytes)?;
        let refs = value
            .as_array()
            .ok_or(AnalysisFailure::RequiredCoverageMissing)?;
        let mut exact = BTreeSet::new();
        let mut parsed = Vec::new();
        for value in refs {
            let spelling = expect_string(value)?;
            if !exact.insert(spelling) {
                return Err(AnalysisFailure::RequiredCoverageMissing);
            }
            parsed.push(parse_feature_ref(spelling, caps.max_atom_bytes)?);
        }
        result.insert(name.clone(), parsed);
    }
    Ok(result)
}

fn add_implicit_optional_features(package: &mut Package, caps: &AnalysisCaps) -> CargoResult<()> {
    for key in ["dependencies", "build-dependencies", "dev-dependencies"] {
        let Some(table) = package.table.get(key).and_then(Value::as_table) else {
            continue;
        };
        for (alias, value) in table {
            let optional = value
                .as_table()
                .and_then(|table| table.get("optional"))
                .map(expect_bool)
                .transpose()?
                .unwrap_or(false);
            if !optional {
                continue;
            }
            validate_feature_atom(alias, caps.max_atom_bytes)?;
            let has_activation =
                package.features.values().flatten().any(
                    |item| matches!(item, FeatureRef::ActivateOptional(value) if value == alias),
                );
            if package.features.contains_key(alias) && !has_activation {
                return Err(AnalysisFailure::RequiredCoverageMissing);
            }
            if !package.features.contains_key(alias) && !has_activation {
                package.features.insert(
                    alias.clone(),
                    vec![FeatureRef::ActivateOptional(alias.clone())],
                );
            }
        }
    }
    Ok(())
}

fn validate_feature_refs(packages: &[Package], declarations: &[Declaration]) -> CargoResult<()> {
    for package in packages {
        for feature_ref in package.features.values().flatten() {
            validate_feature_ref(packages, declarations, package, feature_ref)?;
        }
    }
    Ok(())
}

fn validate_feature_ref(
    packages: &[Package],
    declarations: &[Declaration],
    package: &Package,
    feature_ref: &FeatureRef,
) -> CargoResult<()> {
    match feature_ref {
        FeatureRef::Local(name) if !package.features.contains_key(name) => {
            Err(AnalysisFailure::RequiredCoverageMissing)
        }
        FeatureRef::ActivateOptional(alias)
            if !declarations.iter().any(|declaration| {
                declaration.owner == package.id
                    && declaration.alias == *alias
                    && declaration.optional
            }) =>
        {
            Err(AnalysisFailure::RequiredCoverageMissing)
        }
        FeatureRef::ForwardStrong(alias, feature) | FeatureRef::ForwardWeak(alias, feature) => {
            let mut found = false;
            for declaration in declarations.iter().filter(|declaration| {
                declaration.owner == package.id && declaration.alias == *alias
            }) {
                found = true;
                if let DependencyJoin::Workspace { package_id } = &declaration.join
                    && !package_by_id(packages, package_id)?
                        .features
                        .contains_key(feature)
                {
                    return Err(AnalysisFailure::RequiredCoverageMissing);
                }
            }
            found
                .then_some(())
                .ok_or(AnalysisFailure::RequiredCoverageMissing)
        }
        _ => Ok(()),
    }
}

fn package_path(package: &Package, relative: &str, caps: &AnalysisCaps) -> CargoResult<String> {
    normalize_repo_path(&package.directory, relative, caps.max_repo_path_bytes)
}

fn explicit_root(
    package: &Package,
    table: &Table,
    default: &str,
    tree: &TreeImage,
    caps: &AnalysisCaps,
) -> CargoResult<String> {
    explicit_target_root(
        &package.directory,
        table,
        default,
        tree,
        caps.max_repo_path_bytes,
    )
}

fn insert_target(
    targets: &mut BTreeSet<NormalizedCargoTarget>,
    target: NormalizedCargoTarget,
) -> CargoResult<()> {
    if targets.iter().any(|item| {
        item.kind == target.kind && item.name == target.name && item.root != target.root
    }) {
        return Err(AnalysisFailure::AuthorityConflict);
    }
    targets.insert(target);
    Ok(())
}

fn package_by_id<'a>(packages: &'a [Package], id: &str) -> CargoResult<&'a Package> {
    packages
        .iter()
        .find(|package| package.id == id)
        .ok_or(AnalysisFailure::RequiredCoverageMissing)
}

fn insert_existing_feature(
    packages: &[Package],
    features: &mut BTreeMap<String, BTreeSet<String>>,
    package_id: &str,
    feature: &str,
) -> CargoResult<()> {
    if !package_by_id(packages, package_id)?
        .features
        .contains_key(feature)
    {
        return Err(AnalysisFailure::RequiredCoverageMissing);
    }
    features
        .get_mut(package_id)
        .expect("known package")
        .insert(feature.to_owned());
    Ok(())
}
