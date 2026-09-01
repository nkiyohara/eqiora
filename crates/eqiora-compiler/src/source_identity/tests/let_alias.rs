use super::identity;

#[test]
fn source_structure_has_exact_identity() {
    let base = "model m { parameter p: m = 2; let k: 1 / m = math.pi / p; }";
    let reformatted = "model m {\n parameter p: m = 2;\n let k: 1/m = math.pi/p;\n}";
    let renamed = "model m { parameter p: m = 2; let wave: 1 / m = math.pi / p; }";
    let changed = "model m { parameter p: m = 2; let k: 1 / m = 2 / p; }";

    assert_eq!(identity(base), identity(reformatted));
    assert_ne!(identity(base), identity(renamed));
    assert_ne!(identity(base), identity(changed));
}

#[test]
fn omitted_dimension_has_distinct_deterministic_identity() {
    let annotated = "model m { parameter p: m = 2; let k: 1 / m = math.pi / p; }";
    let inferred = "model m { parameter p: m = 2; let k = math.pi / p; }";
    let reformatted = "model m {\n parameter p: m = 2;\n let k=math.pi/p;\n}";

    assert_ne!(identity(annotated), identity(inferred));
    assert_eq!(identity(inferred), identity(reformatted));
}
