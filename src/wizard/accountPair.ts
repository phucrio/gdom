export type AccountPairSuccess = {
  ok: true;
  sourceAccountId: string;
  targetAccountId: string;
};

export type AccountPairFailure = {
  ok: false;
  reason: "missing-source" | "missing-target" | "same-source-and-target";
};

export type AccountPairResult = AccountPairSuccess | AccountPairFailure;

export function selectAccountPair(
  sourceAccountId: string | null,
  targetAccountId: string | null,
): AccountPairResult {
  if (sourceAccountId === null || sourceAccountId.length === 0) {
    return { ok: false, reason: "missing-source" };
  }

  if (targetAccountId === null || targetAccountId.length === 0) {
    return { ok: false, reason: "missing-target" };
  }

  if (sourceAccountId === targetAccountId) {
    return { ok: false, reason: "same-source-and-target" };
  }

  return { ok: true, sourceAccountId, targetAccountId };
}

export function accountPairErrorMessage(reason: AccountPairFailure["reason"]): string {
  switch (reason) {
    case "missing-source":
      return "Select a source account.";
    case "missing-target":
      return "Select a target account.";
    case "same-source-and-target":
      return "Source and target must be different accounts.";
  }
}
