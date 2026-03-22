/** Maps backend execution stage IDs to human-readable group labels.
 *  Multiple backend stages collapse into a single display group
 *  (e.g. "branch-gen" + "branch" -> "Branch"). */

const STAGE_TO_GROUP: Record<string, string> = {
  'branch-gen': 'Branch',
  'branch': 'Branch',
  'plan': 'Plan',
  'plan-validation': 'Plan',
  'plan-decompose': 'Plan',
  'implement': 'Implement',
  'code-review': 'Code Review',
  'code-review-fix': 'Code Review',
  'add-and-commit': 'Commit',
};

/** Canonical display order for the stage groups shown in the stepper. */
export const STAGE_GROUP_ORDER = ['Branch', 'Plan', 'Implement', 'Code Review', 'Commit'] as const;

export type StageGroupLabel = (typeof STAGE_GROUP_ORDER)[number] | string;

export type StageGroupStatus = 'pending' | 'running' | 'finished' | 'error' | 'skipped';

export interface StageGroup {
  label: StageGroupLabel;
  status: StageGroupStatus;
}

export function getStageGroupLabel(backendStage: string): string {
  return STAGE_TO_GROUP[backendStage] ?? formatCommandId(backendStage);
}

function formatCommandId(id: string): string {
  return id
    .split('-')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}

/** Given a list of sub-run objects (with `stage` and `status`), derive
 *  the collapsed stage groups with their aggregate status. */
export function deriveStageGroups(
  subRuns: Array<{ stage?: string; status: string }>,
): StageGroup[] {
  const seen = new Map<string, StageGroupStatus>();

  for (const run of subRuns) {
    if (!run.stage) continue;
    const label = getStageGroupLabel(run.stage);
    const existing = seen.get(label);
    const incoming = run.status as StageGroupStatus;

    if (!existing) {
      seen.set(label, incoming);
    } else {
      seen.set(label, mergeStatus(existing, incoming));
    }
  }

  const result: StageGroup[] = [];
  const ordered = [...STAGE_GROUP_ORDER];

  for (const label of ordered) {
    const status = seen.get(label);
    if (status) {
      result.push({ label, status });
      seen.delete(label);
    }
  }

  // Append any custom stages that aren't in the canonical order
  for (const [label, status] of seen) {
    result.push({ label, status });
  }

  return result;
}

function mergeStatus(a: StageGroupStatus, b: StageGroupStatus): StageGroupStatus {
  if (a === 'error' || b === 'error') return 'error';
  if (a === 'running' || b === 'running') return 'running';
  if (a === 'finished' && b === 'finished') return 'finished';
  return b;
}
