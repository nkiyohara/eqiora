export const MAX_VALUE_EDIT_INPUT_LENGTH = 128;

export type ValueEditValidation = Readonly<{
  value: number | null;
  error: string | null;
}>;

export function validateValueEditInput(input: string, currentValue: number): ValueEditValidation {
  if (input.length > MAX_VALUE_EDIT_INPUT_LENGTH) {
    return { value: null, error: "Value exceeds the 128-character editor limit." };
  }
  if (input.trim().length === 0) {
    return { value: null, error: "Enter one coherent-SI scalar." };
  }
  const value = Number(input);
  if (!Number.isFinite(value)) {
    return { value: null, error: "Value must be a finite scalar." };
  }
  if (value === currentValue) {
    return { value: null, error: null };
  }
  return { value, error: null };
}
