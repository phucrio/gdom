import { accountDisplayLabel } from "../accounts/status.ts";
import type { AccountDto, AccountSnapshotDto, JobDto } from "../ipc/types.ts";
import { JOB_LABEL_SEPARATOR, JOB_PAIR_ARROW } from "./copy.ts";

function snapshotSide(snapshot: AccountSnapshotDto, accounts: readonly AccountDto[]): string {
  const live = accounts.find((account) => account.id === snapshot.accountId) ?? null;
  const email = snapshot.email.trim();
  if (live !== null) {
    const label = accountDisplayLabel(live).trim();
    if (label.length > 0 && label !== email) {
      return `${label}${JOB_LABEL_SEPARATOR}${email}`;
    }
  }

  const displayName = snapshot.displayName.trim();
  if (displayName.length > 0 && displayName !== email) {
    return `${displayName}${JOB_LABEL_SEPARATOR}${email}`;
  }
  return email.length > 0 ? email : snapshot.accountId;
}

/** Pair text uses job snapshots, never the account currently selected in the registry. */
export function jobPairLabel(job: JobDto, accounts: readonly AccountDto[] = []): string {
  return `${snapshotSide(job.sourceSnapshot, accounts)}${JOB_PAIR_ARROW}${snapshotSide(job.targetSnapshot, accounts)}`;
}
