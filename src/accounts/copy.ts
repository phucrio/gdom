export const ACCOUNTS_LOAD_FAILED = "Could not load the account registry from the local backend.";

export function referencingJobsLabel(count: number): string {
  return count === 1 ? "1 referencing job" : `${count} referencing jobs`;
}
