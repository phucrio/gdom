import { selectAccountPair } from "./accountPair.ts";
import { confirmCanaryEmail } from "./canary.ts";

export const WIZARD_STEP_ORDER = [
  "select-accounts",
  "add-roots",
  "scan-preflight",
  "canary-review",
  "live-migration",
] as const;

export type WizardStepId = (typeof WIZARD_STEP_ORDER)[number];

export const WIZARD_STEP_TITLES: Record<WizardStepId, string> = {
  "select-accounts": "Select accounts",
  "add-roots": "Add root folders",
  "scan-preflight": "Scan and dry-run",
  "canary-review": "Canary review",
  "live-migration": "Live migration",
};

export type WizardAdvanceError =
  | "accounts-not-selected"
  | "same-source-and-target"
  | "no-valid-roots"
  | "scan-incomplete"
  | "canary-not-confirmed"
  | "cannot-skip-step"
  | "already-at-step";

export type WizardGate = {
  sourceAccountId: string | null;
  targetAccountId: string | null;
  validRootCount: number;
  preflightReady: boolean;
  canaryConfirmed: boolean;
};

export type StepMoveResult =
  | { ok: true; step: WizardStepId }
  | { ok: false; reason: WizardAdvanceError };

export function wizardStepIndex(step: WizardStepId): number {
  return WIZARD_STEP_ORDER.indexOf(step);
}

export function canLeaveStep(step: WizardStepId, gate: WizardGate): StepMoveResult {
  switch (step) {
    case "select-accounts": {
      const pair = selectAccountPair(gate.sourceAccountId, gate.targetAccountId);
      if (!pair.ok) {
        if (pair.reason === "same-source-and-target") {
          return { ok: false, reason: "same-source-and-target" };
        }
        return { ok: false, reason: "accounts-not-selected" };
      }
      return { ok: true, step: "add-roots" };
    }
    case "add-roots":
      if (gate.validRootCount < 1) {
        return { ok: false, reason: "no-valid-roots" };
      }
      return { ok: true, step: "scan-preflight" };
    case "scan-preflight":
      if (!gate.preflightReady) {
        return { ok: false, reason: "scan-incomplete" };
      }
      return { ok: true, step: "canary-review" };
    case "canary-review":
      if (!gate.canaryConfirmed) {
        return { ok: false, reason: "canary-not-confirmed" };
      }
      return { ok: true, step: "live-migration" };
    case "live-migration":
      return { ok: false, reason: "already-at-step" };
  }
}

export function moveToStep(
  from: WizardStepId,
  to: WizardStepId,
  gate: WizardGate,
): StepMoveResult {
  if (from === to) {
    return { ok: true, step: to };
  }

  const fromIndex = wizardStepIndex(from);
  const toIndex = wizardStepIndex(to);

  if (toIndex < fromIndex) {
    return { ok: true, step: to };
  }

  if (toIndex > fromIndex + 1) {
    return { ok: false, reason: "cannot-skip-step" };
  }

  return canLeaveStep(from, gate);
}

export function advanceWizard(from: WizardStepId, gate: WizardGate): StepMoveResult {
  return canLeaveStep(from, gate);
}

export function canaryGateAllowsContinue(
  enteredEmail: string,
  targetEmail: string,
): boolean {
  return confirmCanaryEmail(enteredEmail, targetEmail);
}

export function wizardAdvanceErrorMessage(reason: WizardAdvanceError): string {
  switch (reason) {
    case "accounts-not-selected":
      return "Select both a source and a target account.";
    case "same-source-and-target":
      return "Source and target must be different accounts.";
    case "no-valid-roots":
      return "Add at least one valid root folder before scanning.";
    case "scan-incomplete":
      return "Finish the scan and review the dry-run before canary.";
    case "canary-not-confirmed":
      return "Re-enter the target email and confirm it matches before migrating.";
    case "cannot-skip-step":
      return "Complete each step in order. The canary gate cannot be skipped.";
    case "already-at-step":
      return "The wizard is already on the live migration step.";
  }
}
