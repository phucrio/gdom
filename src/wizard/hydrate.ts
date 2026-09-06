import type { JobDto } from "../ipc/types.ts";
import { wizardStepForJob, type WizardStepId } from "./steps.ts";

export type HydratedWizard = {
  sourceAccountId: string;
  targetAccountId: string;
  roots: Array<{ folderId: string; input: string }>;
  step: WizardStepId;
};

/** Reopen uses persisted source/target IDs from the job, not registry selection. */
export function hydrateWizardFromJob(job: JobDto): HydratedWizard {
  return {
    sourceAccountId: job.sourceAccountId,
    targetAccountId: job.targetAccountId,
    roots: job.roots.map((root) => ({
      folderId: root.rootFileId,
      input: root.rootName.length > 0 ? root.rootName : root.rootFileId,
    })),
    step: wizardStepForJob(job),
  };
}
