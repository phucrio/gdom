/** Drops stale async results when a newer load has started. */
export function createLatestLoad() {
  let generation = 0;

  return {
    begin(): number {
      generation += 1;
      return generation;
    },
    isCurrent(token: number): boolean {
      return token === generation;
    },
  };
}
