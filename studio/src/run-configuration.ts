import { MAX_REQUESTED_STEPS } from "./reference-run-protocol";

export interface RunConfiguration {
  readonly endTime: string;
  readonly maxStep: string;
}

export interface ValidatedRunConfiguration {
  readonly endTime: number;
  readonly maxStep: number;
}

export interface RunConfigurationErrors {
  readonly endTime: string | null;
  readonly maxStep: string | null;
}

export interface RunConfigurationValidation {
  readonly value: ValidatedRunConfiguration | null;
  readonly errors: RunConfigurationErrors;
}

const DECIMAL_NUMBER = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

function positiveFiniteNumber(input: string): number | null {
  const normalized = input.trim();
  if (!DECIMAL_NUMBER.test(normalized)) {
    return null;
  }
  const value = Number(normalized);
  return Number.isFinite(value) && value > 0 ? value : null;
}

export function validateRunConfiguration(
  configuration: RunConfiguration,
): RunConfigurationValidation {
  const endTime = positiveFiniteNumber(configuration.endTime);
  const maxStep = positiveFiniteNumber(configuration.maxStep);
  const endTimeError = endTime === null ? "Enter a positive, finite end time." : null;
  const maxStepError = maxStep === null ? "Enter a positive, finite maximum step." : null;

  if (endTime === null || maxStep === null) {
    return {
      value: null,
      errors: { endTime: endTimeError, maxStep: maxStepError },
    };
  }

  if (Math.ceil(endTime / maxStep) > MAX_REQUESTED_STEPS) {
    return {
      value: null,
      errors: {
        endTime: null,
        maxStep: `Choose a step that requests at most ${MAX_REQUESTED_STEPS.toLocaleString("en-US")} integration steps.`,
      },
    };
  }

  return {
    value: { endTime, maxStep },
    errors: { endTime: null, maxStep: null },
  };
}
