export type IpcError = {
  kind: string;
  message: string;
  command: string | null;
};

const UNKNOWN_COMMAND_PATTERN =
  /unknown command|command ['`]?[\w-]+['`]? not found|not allowed by (the )?acl|command .+ not allowed|command is not available in this build/;

export function isCommandMissing(error: IpcError): boolean {
  if (error.kind === "commandMissing") {
    return true;
  }

  if (error.kind !== "internal") {
    return false;
  }

  return UNKNOWN_COMMAND_PATTERN.test(error.message.toLowerCase());
}

function readStringProperty(value: object, key: string): string | null {
  if (!(key in value)) {
    return null;
  }

  const property = Reflect.get(value, key);
  return typeof property === "string" ? property : null;
}

export function toIpcError(error: unknown, command: string): IpcError {
  if (typeof error === "object" && error !== null) {
    const kind = readStringProperty(error, "kind");
    const message = readStringProperty(error, "message");
    if (kind !== null && message !== null) {
      return { kind, message, command };
    }

    if (message !== null) {
      const parsed = tryParseNestedError(message, command);
      if (parsed !== null) {
        return parsed;
      }

      return {
        kind: inferKindFromMessage(message),
        message,
        command,
      };
    }
  }

  if (typeof error === "string") {
    return {
      kind: inferKindFromMessage(error),
      message: error,
      command,
    };
  }

  return {
    kind: "internal",
    message: "An unexpected backend error occurred.",
    command,
  };
}

function tryParseNestedError(message: string, command: string): IpcError | null {
  const trimmed = message.trim();
  if (!trimmed.startsWith("{")) {
    return null;
  }

  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (typeof parsed === "object" && parsed !== null) {
      const kind = readStringProperty(parsed, "kind");
      const innerMessage = readStringProperty(parsed, "message");
      if (kind !== null && innerMessage !== null) {
        return { kind, message: innerMessage, command };
      }
    }
  } catch {
    return null;
  }

  return null;
}

function inferKindFromMessage(message: string): string {
  const text = message.toLowerCase();
  if (UNKNOWN_COMMAND_PATTERN.test(text)) {
    return "commandMissing";
  }

  if (text.includes("not configured")) {
    return "notConfigured";
  }

  return "internal";
}

export function formatIpcError(error: IpcError): string {
  if (isCommandMissing(error) && error.command !== null) {
    return `The ${error.command} command is not available in this build yet.`;
  }

  return error.message;
}
