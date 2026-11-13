// The two ways to take a finished run's work: try it in its worktree, or open the review.
export interface IntegrationInvite {
  open: () => void;
  tryIt: () => void;
}
