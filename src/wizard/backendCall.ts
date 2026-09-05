export type BackendCall<T> =
  | { status: "ok"; value: T }
  | { status: "missing" }
  | { status: "error" };

export function unlocksNextGate(result: BackendCall<unknown>): boolean {
  return result.status === "ok" || result.status === "missing";
}
