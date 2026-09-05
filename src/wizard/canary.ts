export function normalizeEmail(value: string): string {
  return value.trim().toLowerCase();
}

export function confirmCanaryEmail(entered: string, targetEmail: string): boolean {
  const expected = normalizeEmail(targetEmail);
  const actual = normalizeEmail(entered);
  return expected.length > 0 && actual === expected;
}
