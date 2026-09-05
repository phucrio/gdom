const SECRET_NAME_FRAGMENTS = [
  "accesstoken",
  "refreshtoken",
  "pkce",
  "authorizationcode",
  "clientsecret",
  "verifier",
] as const;

function normalizeFieldName(name: string): string {
  return name.replace(/_/g, "").replace(/-/g, "").toLowerCase();
}

export function fieldNameLooksLikeSecret(name: string): boolean {
  const normalized = normalizeFieldName(name);
  return SECRET_NAME_FRAGMENTS.some((fragment) => normalized.includes(fragment));
}

export function secretFieldsInRecord(record: object, depth = 0): string[] {
  if (depth > 6) {
    return [];
  }

  const leaked: string[] = [];
  for (const [key, value] of Object.entries(record)) {
    if (fieldNameLooksLikeSecret(key)) {
      leaked.push(key);
    }
    if (Array.isArray(value)) {
      for (const item of value) {
        if (typeof item === "object" && item !== null) {
          leaked.push(...secretFieldsInRecord(item, depth + 1));
        }
      }
    } else if (typeof value === "object" && value !== null) {
      leaked.push(...secretFieldsInRecord(value, depth + 1));
    }
  }
  return leaked;
}

export function assertNoSecretFields(record: object): void {
  const leaked = secretFieldsInRecord(record);
  if (leaked.length > 0) {
    throw new Error(`secret fields are forbidden in IPC payloads: ${leaked.join(", ")}`);
  }
}
