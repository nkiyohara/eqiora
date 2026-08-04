//! Structural oracle for the certificate-derived local instruction tape.
//!
//! This module deliberately contains no elasticity arithmetic. It freezes the
//! private tape seam, mutates only generated structure, and delegates numeric
//! observation to the already accepted exact matrix oracle in the parent.

use std::borrow::Cow;

use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id, RawId};
use eqiora_schema::kernel::ExprId;
use eqiora_schema::kernel::typing::TypedResidual;

use super::super::execution::{
    LocalFormExecution, LocalFormInput2d, LocalFormInstruction, LocalFormInstructionProvenance,
    LocalFormProgram2d, NormalizedLocalFormInstructionProvenance, NormalizedLocalFormProgram2d,
    compile_derived_local_form_program_2d, compile_witness_local_form_program_2d,
};
use super::super::{
    AdmittedCartesianQ1ElasticityForm2d, DerivationCertificate, DerivedCartesianQ1ElasticityForm2d,
};
use crate::form_compiler::{MatrixSlot, WeakSign, WeakTermSlot};

const ELASTICITY_SOURCE: &str = include_str!("../../elasticity.rs");
const EXECUTION_SOURCE: &str = include_str!("../execution.rs");

const FROZEN_SOURCE_PREDICATE: &str = "Across the production writable paths, there are zero \
source expressions, coefficient tables, generated-source literals, or closures whose data flow \
directly combines a material coefficient or material role with component-indexed test/trial \
basis gradients. The only such relationships are data dependencies in an instruction stream \
mechanically emitted from the typed DAG, schema-owned pure calculus, and certificate rules. The \
generic executor operates on opaque dense values and does not inspect those roles.";

pub(super) fn structural_tape_falsifiers() {
    private_tape_seam_matches_the_frozen_contract();
    derived_and_witness_programs_normalize_identically();
    mutated_structure_and_provenance_fail_closed();
    off_diagonal_instruction_mutant_is_rejected_by_the_exact_matrix_oracle();
    evaluator_result_depends_on_the_mutated_instruction_tape();
    primal_and_all_actions_consume_one_admitted_program_instance();
    witness_program_equality_does_not_admit_actions();
}

fn private_tape_seam_matches_the_frozen_contract() {
    let _: fn(LocalFormInput2d) = closed_local_form_input_vocabulary;
    let _: fn(LocalFormInstructionProvenance) = exact_instruction_provenance_fields;
    let _: fn(NormalizedLocalFormInstructionProvenance) = exact_normalized_provenance_fields;
    let _: fn(LocalFormProgram2d) = exact_program_fields;
    let _: fn(NormalizedLocalFormProgram2d) = exact_normalized_program_fields;

    let _: fn(
        &TypedResidual<RawId>,
        ExprId,
        RawId,
        [Id<kinds::Parameter>; 2],
        &DerivationCertificate,
    ) -> Result<LocalFormProgram2d, Diagnostic> = compile_derived_local_form_program_2d;

    let _: fn() -> Result<LocalFormProgram2d, Diagnostic> = compile_witness_local_form_program_2d;

    let _: for<'a> fn(&'a LocalFormProgram2d) -> Result<(), Diagnostic> =
        LocalFormProgram2d::validate;

    let _: for<'a> fn(&'a LocalFormProgram2d) -> NormalizedLocalFormProgram2d =
        LocalFormProgram2d::normalized;

    let _: for<'a> fn(
        &'a LocalFormProgram2d,
        &'a [f64],
        Option<&'a [f64]>,
        Option<&'a [f64]>,
    ) -> Result<LocalFormExecution, Diagnostic> = LocalFormProgram2d::execute;

    let _: for<'a> fn(&'a LocalFormExecution) -> &'a [f64] = LocalFormExecution::roots;

    let _: for<'a> fn(&'a LocalFormExecution) -> Option<&'a [f64]> =
        LocalFormExecution::root_tangents;

    let _: for<'a> fn(&'a LocalFormExecution) -> Option<&'a [f64]> =
        LocalFormExecution::input_cotangents;

    bind_executable_program_for_arbitrary_form_lifetime();
}

fn bind_executable_program_for_arbitrary_form_lifetime<'form>() {
    let _: for<'borrow> fn(
        &'borrow AdmittedCartesianQ1ElasticityForm2d<'form>,
    ) -> &'borrow LocalFormProgram2d = AdmittedCartesianQ1ElasticityForm2d::executable_program;
}

fn derived_and_witness_programs_normalize_identically() {
    let derived = super::derived_form();
    let witness = compile_witness_local_form_program_2d().unwrap();
    derived.program.validate().unwrap();
    witness.validate().unwrap();
    assert_closed_input_inventory(&derived.program);
    assert_closed_input_inventory(&witness);
    assert_normalization_erases_only_relation_and_source_node(&derived.program);
    assert_normalization_erases_only_relation_and_source_node(&witness);
    assert!(
        derived
            .program
            .provenance
            .iter()
            .any(|provenance| provenance.relation.is_some() && provenance.source_node.is_some()),
        "derived instructions erased their exact Relation/source-node provenance",
    );
    assert!(
        witness
            .provenance
            .iter()
            .all(|provenance| provenance.relation.is_none() && provenance.source_node.is_none()),
        "witness provenance fabricated Semantic or source-node identity",
    );
    assert_eq!(
        derived.program.normalized(),
        witness.normalized(),
        "derived and witness streams differ after exactly Relation/source-node provenance erasure",
    );
}

fn mutated_structure_and_provenance_fail_closed() {
    let mut missing_instruction_provenance = compile_witness_local_form_program_2d().unwrap();
    missing_instruction_provenance
        .provenance
        .pop()
        .expect("the generated program has instruction-aligned provenance");
    assert_validation_rejects(
        &missing_instruction_provenance,
        "missing instruction provenance",
    );

    let mut dense_role = compile_witness_local_form_program_2d().unwrap();
    let material_zero = dense_role
        .inputs
        .iter()
        .position(|input| *input == LocalFormInput2d::Material(0))
        .expect("the frozen dense-role inventory contains material coordinate zero");
    let material_one = dense_role
        .inputs
        .iter()
        .position(|input| *input == LocalFormInput2d::Material(1))
        .expect("the frozen dense-role inventory contains material coordinate one");
    dense_role.inputs.swap(material_zero, material_one);
    assert_validation_rejects(&dense_role, "reordered dense input roles");

    let mut operand = super::derived_form();
    let instruction = operand
        .program
        .instructions
        .iter_mut()
        .find(|instruction| {
            matches!(instruction, LocalFormInstruction::Add(left, right) | LocalFormInstruction::Mul(left, right) if left != right)
        })
        .expect("the generated program contains distinct ordered operands");
    match instruction {
        LocalFormInstruction::Add(left, right) | LocalFormInstruction::Mul(left, right) => {
            std::mem::swap(left, right);
        }
        LocalFormInstruction::Read(_)
        | LocalFormInstruction::ConstantBits(_)
        | LocalFormInstruction::Neg(_) => unreachable!(),
    }
    assert_admission_rejects(operand, "reordered instruction operands");

    let mut root = compile_witness_local_form_program_2d().unwrap();
    assert_eq!(root.roots.len(), 2);
    root.roots.swap(0, 1);
    assert_validation_rejects(&root, "reordered result roots");

    let mut instruction = super::derived_form();
    let read = instruction
        .program
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, LocalFormInstruction::Read(_)))
        .expect("the generated program reads its closed input inventory");
    instruction.program.instructions[read] = LocalFormInstruction::ConstantBits(1.0_f64.to_bits());
    assert_admission_rejects(instruction, "stale instruction kind");

    let mut relation = super::derived_form();
    let provenance = relation
        .program
        .provenance
        .iter()
        .position(|provenance| provenance.relation.is_some())
        .expect("the derived program retains Relation provenance");
    let exact_relation = relation.program.provenance[provenance].relation.unwrap();
    let foreign_relation = relation
        .certificate
        .entries
        .iter()
        .map(|entry| entry.relation)
        .find(|candidate| *candidate != exact_relation)
        .expect("the certificate contains an independently foreign boundary Relation");
    relation.program.provenance[provenance].relation = Some(foreign_relation);
    assert_admission_rejects(relation, "foreign instruction Relation provenance");

    let mut source_node = super::derived_form();
    let provenance = source_node
        .program
        .provenance
        .iter()
        .position(|provenance| provenance.source_node.is_some())
        .expect("the derived program retains source-node provenance");
    let exact_node = source_node.program.provenance[provenance]
        .source_node
        .unwrap();
    let foreign_node = source_node
        .certificate
        .entries
        .iter()
        .map(|entry| entry.source_node)
        .find(|candidate| *candidate != exact_node)
        .expect("the certificate contains an independently foreign source node");
    source_node.program.provenance[provenance].source_node = Some(foreign_node);
    assert_admission_rejects(source_node, "foreign source-node provenance");

    let mut operator = super::derived_form();
    let provenance = operator
        .program
        .provenance
        .iter()
        .position(|provenance| provenance.operator_definition.is_some())
        .expect("the generated program retains pure-operator provenance");
    let exact_operator = operator.program.provenance[provenance]
        .operator_definition
        .unwrap();
    let foreign_operator = operator
        .program
        .provenance
        .iter()
        .filter_map(|provenance| provenance.operator_definition)
        .find(|candidate| *candidate != exact_operator)
        .expect("the two admitted pure operators have distinct definition digests");
    operator.program.provenance[provenance].operator_definition = Some(foreign_operator);
    assert_admission_rejects(operator, "foreign operator-definition provenance");

    let mut rule = super::derived_form();
    let provenance = rule
        .program
        .provenance
        .iter()
        .position(|provenance| provenance.rule_id.is_some())
        .expect("the generated program retains weak-rule provenance");
    rule.program.provenance[provenance].rule_id = Some("fem.derive.v1.foreign-rule");
    assert_admission_rejects(rule, "foreign weak-rule provenance");

    let mut slot = super::derived_form();
    let bilinear = slot
        .certificate
        .entries
        .iter()
        .position(|entry| matches!(entry.slot, WeakTermSlot::Bilinear { .. }))
        .expect("the certificate retains a bilinear test/trial slot");
    slot.certificate.entries[bilinear].slot = WeakTermSlot::Bilinear {
        test: MatrixSlot::Trial,
        trial: MatrixSlot::Test,
    };
    assert_admission_rejects(slot, "swapped certificate test/trial slot");

    let mut sign = super::derived_form();
    let signed = sign
        .certificate
        .entries
        .iter()
        .position(|entry| entry.sign == WeakSign::Positive)
        .expect("the certificate retains a signed weak rule");
    sign.certificate.entries[signed].sign = WeakSign::Negative;
    assert_admission_rejects(sign, "mutated certificate sign");
}

fn off_diagonal_instruction_mutant_is_rejected_by_the_exact_matrix_oracle() {
    let (accepted, mutant) = accepted_and_mutated_witness_matrices();
    super::assert_absolute_slice(&accepted, &super::MATRIX);
    super::assert_rejected_numeric_mutant(
        &mutant,
        &super::MATRIX,
        "off-diagonal symmetric-part instruction mutation",
    );
}

fn evaluator_result_depends_on_the_mutated_instruction_tape() {
    let (accepted, mutant) = accepted_and_mutated_witness_matrices();
    assert_ne!(
        accepted, mutant,
        "the cumulative evaluator ignored the mutated instruction tape and returned the accepted matrix",
    );
}

fn primal_and_all_actions_consume_one_admitted_program_instance() {
    assert_frozen_source_predicate_and_single_cumulative_path();

    let program = super::compile_program(super::SOURCE);
    let model =
        crate::canonical_elasticity::lower_isotropic_elasticity_cartesian_2d(&program).unwrap();
    let quadrature = super::quadrature();
    let form = super::super::derive_cartesian_q1_elasticity_form_2d(&program).unwrap();
    let admitted = form.admit_quadrature(&quadrature).unwrap();
    let executable = admitted.executable_program();

    admitted
        .evaluate(
            &super::geometry(),
            &quadrature,
            super::PARAMETERS[0],
            super::PARAMETERS[1],
            Some(model.load_potential_expression()),
        )
        .unwrap();
    assert!(std::ptr::eq(executable, admitted.executable_program()));

    let actions = admitted
        .evaluate_with_actions(
            &super::geometry(),
            &quadrature,
            super::PARAMETERS,
            &super::STATE,
            &super::STATE_DIRECTION,
            super::PARAMETER_DIRECTION,
            &super::COTANGENT,
        )
        .unwrap();
    assert!(std::ptr::eq(executable, admitted.executable_program()));
    assert_eq!(actions.state_direction_action().len(), 8);
    assert_eq!(actions.parameter_direction_action().len(), 8);
    assert_eq!(actions.state_transpose_action().len(), 8);
    assert_eq!(actions.parameter_transpose_action().len(), 3);
}

fn witness_program_equality_does_not_admit_actions() {
    let derived = super::derived_form();
    let quadrature = super::quadrature();
    let witness_program = compile_witness_local_form_program_2d().unwrap();
    let witness = super::super::compile_cartesian_q1_elasticity_form_2d(&quadrature).unwrap();
    assert_eq!(derived.program.normalized(), witness_program.normalized());
    assert_eq!(
        witness.executable_program().normalized(),
        witness_program.normalized(),
    );

    let error = match witness.evaluate_with_actions(
        &super::geometry(),
        &quadrature,
        super::PARAMETERS,
        &super::STATE,
        &super::STATE_DIRECTION,
        super::PARAMETER_DIRECTION,
        &super::COTANGENT,
    ) {
        Ok(_) => panic!("normalized program equality granted witness differential actions"),
        Err(error) => error,
    };
    assert!(
        error.message().contains("certificate"),
        "witness actions did not fail at the pre-program certificate gate: {error:?}",
    );
}

fn accepted_and_mutated_witness_matrices() -> (Vec<f64>, Vec<f64>) {
    let quadrature = super::quadrature();
    let mut witness = super::super::compile_cartesian_q1_elasticity_form_2d(&quadrature).unwrap();
    let accepted = witness
        .evaluate(
            &super::geometry(),
            &quadrature,
            super::PARAMETERS[0],
            super::PARAMETERS[1],
            None,
        )
        .unwrap()
        .matrix()
        .to_vec();

    let mutant = full_gradient_instruction_mutant(witness.executable_program());
    mutant
        .validate()
        .expect("the off-diagonal mutant remains a well-formed executable tape");
    witness.program = Cow::Owned(mutant);
    let mutated = witness
        .evaluate(
            &super::geometry(),
            &quadrature,
            super::PARAMETERS[0],
            super::PARAMETERS[1],
            None,
        )
        .unwrap()
        .matrix()
        .to_vec();
    (accepted, mutated)
}

fn full_gradient_instruction_mutant(program: &LocalFormProgram2d) -> LocalFormProgram2d {
    let mut mutant = program.clone();
    let (instruction_index, input_index) = program
        .instructions
        .iter()
        .enumerate()
        .find_map(|(instruction_index, instruction)| {
            let LocalFormInstruction::Read(input_index) = instruction else {
                return None;
            };
            let input = program.inputs.get(usize::from(*input_index))?;
            let is_off_diagonal = matches!(
                input,
                LocalFormInput2d::StateJet { component, axis } if component != axis
            );
            (is_off_diagonal
                && program.provenance[instruction_index]
                    .operator_definition
                    .is_some())
            .then_some((instruction_index, usize::from(*input_index)))
        })
        .expect("a symmetric-part expansion reads an off-diagonal state jet");
    let replacement = program
        .inputs
        .iter()
        .enumerate()
        .find_map(|(candidate_index, input)| {
            let is_other_off_diagonal = matches!(
                input,
                LocalFormInput2d::StateJet { component, axis }
                    if component != axis && candidate_index != input_index
            );
            is_other_off_diagonal.then_some(candidate_index)
        })
        .expect("the closed input inventory contains the other off-diagonal state jet");
    mutant.instructions[instruction_index] =
        LocalFormInstruction::Read(u16::try_from(replacement).unwrap());
    mutant
}

fn assert_normalization_erases_only_relation_and_source_node(program: &LocalFormProgram2d) {
    let normalized = program.normalized();
    assert_eq!(normalized.inputs, program.inputs);
    assert_eq!(normalized.instructions, program.instructions);
    assert_eq!(normalized.roots, program.roots);
    assert_eq!(normalized.provenance.len(), program.provenance.len());
    for (normalized, exact) in normalized.provenance.iter().zip(&program.provenance) {
        assert_eq!(normalized.rule_id, exact.rule_id);
        assert_eq!(normalized.operator_definition, exact.operator_definition);
    }
}

fn assert_closed_input_inventory(program: &LocalFormProgram2d) {
    assert_eq!(
        program.inputs.as_slice(),
        &[
            LocalFormInput2d::Material(0),
            LocalFormInput2d::Material(1),
            LocalFormInput2d::StateJet {
                component: 0,
                axis: 0,
            },
            LocalFormInput2d::StateJet {
                component: 0,
                axis: 1,
            },
            LocalFormInput2d::StateJet {
                component: 1,
                axis: 0,
            },
            LocalFormInput2d::StateJet {
                component: 1,
                axis: 1,
            },
            LocalFormInput2d::ShapeGradient(0),
            LocalFormInput2d::ShapeGradient(1),
            LocalFormInput2d::ShapeValue,
            LocalFormInput2d::BodyForce(0),
            LocalFormInput2d::BodyForce(1),
            LocalFormInput2d::QuadratureScale,
        ],
        "generated program changed the frozen dense input role order",
    );
    assert_eq!(program.roots.len(), 2);
}

fn assert_frozen_source_predicate_and_single_cumulative_path() {
    for identifier in [
        "gradient_dot",
        "delta_term",
        "shear_basis",
        "volumetric_basis",
        "mu_matrix",
        "lambda_matrix",
    ] {
        assert!(
            !ELASTICITY_SOURCE.contains(identifier) && !EXECUTION_SOURCE.contains(identifier),
            "forbidden direct-formula identifier `{identifier}` violates: {FROZEN_SOURCE_PREDICATE}",
        );
    }

    let elasticity = compact_whitespace(ELASTICITY_SOURCE);
    let execution = compact_whitespace(EXECUTION_SOURCE);
    let evaluate = compact_whitespace(method_declaration(
        ELASTICITY_SOURCE,
        "pub(crate) fn evaluate(",
    ));
    let evaluate_with_actions = compact_whitespace(method_declaration(
        ELASTICITY_SOURCE,
        "pub(crate) fn evaluate_with_actions(",
    ));
    assert_eq!(
        evaluate.matches("self.executable_program()").count(),
        1,
        "primal evaluation must pass the single admitted program accessor to the cumulative path",
    );
    assert_eq!(
        evaluate_with_actions
            .matches("self.executable_program()")
            .count(),
        1,
        "differential evaluation must pass the same admitted program accessor to the cumulative path",
    );
    assert_eq!(
        elasticity.matches("self.executable_program()").count(),
        2,
        "only the two frozen evaluation methods may select an executable program",
    );
    let accessor = evaluate_with_actions
        .find("self.executable_program()")
        .unwrap();
    let action_prefix = &evaluate_with_actions[..accessor];
    assert!(
        action_prefix.contains("self.form") && action_prefix.contains("ok_or_else"),
        "witness actions must reject the absent certificate before reading the executable program",
    );

    assert_eq!(
        elasticity.matches(".execute(").count(),
        1,
        "the cumulative production path must have exactly one tape execution site",
    );
    assert_eq!(
        execution.matches("pub(super)fnexecute<'a>(").count(),
        1,
        "the closed instruction vocabulary must have exactly one executor",
    );
    assert_eq!(
        execution
            .matches("pub(super)structLocalFormProgram2d")
            .count(),
        1,
        "production must define exactly one local-form program type",
    );

    let derived = compact_whitespace(struct_declaration(
        ELASTICITY_SOURCE,
        "pub(crate) struct DerivedCartesianQ1ElasticityForm2d",
    ));
    let admitted = compact_whitespace(struct_declaration(
        ELASTICITY_SOURCE,
        "pub(crate) struct AdmittedCartesianQ1ElasticityForm2d",
    ));
    assert_eq!(derived.matches("program:LocalFormProgram2d").count(), 1);
    assert_eq!(
        admitted
            .matches("program:Cow<'form,LocalFormProgram2d>")
            .count(),
        1,
    );
    for declaration in [&derived, &admitted] {
        for forbidden_field in ["matrix:", "executor:", "closure:"] {
            assert!(
                !declaration.contains(forbidden_field),
                "admitted or derived form contains a forbidden fallback field `{forbidden_field}`: {FROZEN_SOURCE_PREDICATE}",
            );
        }
    }
}

fn struct_declaration<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing frozen private declaration `{marker}`"));
    let tail = &source[start..];
    let end = tail
        .find("\n}")
        .unwrap_or_else(|| panic!("unterminated frozen private declaration `{marker}`"));
    &tail[..end + 2]
}

fn method_declaration<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source
        .find(marker)
        .unwrap_or_else(|| panic!("missing frozen private method `{marker}`"));
    let tail = &source[start..];
    let end = tail
        .find("\n    }\n")
        .unwrap_or_else(|| panic!("unterminated frozen private method `{marker}`"));
    &tail[..end + 7]
}

fn compact_whitespace(source: &str) -> String {
    source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn assert_validation_rejects(program: &LocalFormProgram2d, mutation: &str) {
    assert!(
        program.validate().is_err(),
        "{mutation} survived LocalFormProgram2d::validate",
    );
}

fn assert_admission_rejects(form: DerivedCartesianQ1ElasticityForm2d, mutation: &str) {
    let quadrature = super::quadrature();
    assert!(
        form.admit_quadrature(&quadrature).is_err(),
        "{mutation} survived derived program regeneration and admission",
    );
}

fn closed_local_form_input_vocabulary(input: LocalFormInput2d) {
    match input {
        LocalFormInput2d::Material(_)
        | LocalFormInput2d::StateJet { .. }
        | LocalFormInput2d::ShapeGradient(_)
        | LocalFormInput2d::ShapeValue
        | LocalFormInput2d::BodyForce(_)
        | LocalFormInput2d::QuadratureScale => {}
    }
}

fn exact_instruction_provenance_fields(provenance: LocalFormInstructionProvenance) {
    let LocalFormInstructionProvenance {
        rule_id: _,
        relation: _,
        source_node: _,
        operator_definition: _,
    } = provenance;
}

fn exact_normalized_provenance_fields(provenance: NormalizedLocalFormInstructionProvenance) {
    let NormalizedLocalFormInstructionProvenance {
        rule_id: _,
        operator_definition: _,
    } = provenance;
}

fn exact_program_fields(program: LocalFormProgram2d) {
    let LocalFormProgram2d {
        inputs: _,
        instructions: _,
        roots: _,
        provenance: _,
    } = program;
}

fn exact_normalized_program_fields(program: NormalizedLocalFormProgram2d) {
    let NormalizedLocalFormProgram2d {
        inputs: _,
        instructions: _,
        roots: _,
        provenance: _,
    } = program;
}
