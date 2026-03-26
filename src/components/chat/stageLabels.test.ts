import { describe, it, expect } from 'vitest';
import {
  getStageGroupLabel,
  deriveStageGroups,
  mapRunStatus,
  STAGE_GROUP_ORDER,
} from './stageLabels';

describe('getStageGroupLabel', () => {
  it('maps branch-gen to Branch', () => {
    expect(getStageGroupLabel('branch-gen')).toBe('Branch');
  });

  it('maps branch to Branch', () => {
    expect(getStageGroupLabel('branch')).toBe('Branch');
  });

  it('maps plan to Plan', () => {
    expect(getStageGroupLabel('plan')).toBe('Plan');
  });

  it('maps plan-validation to Plan', () => {
    expect(getStageGroupLabel('plan-validation')).toBe('Plan');
  });

  it('maps plan-decompose to Plan', () => {
    expect(getStageGroupLabel('plan-decompose')).toBe('Plan');
  });

  it('maps implement to Implement', () => {
    expect(getStageGroupLabel('implement')).toBe('Implement');
  });

  it('maps code-review to Code Review', () => {
    expect(getStageGroupLabel('code-review')).toBe('Code Review');
  });

  it('maps code-review-fix to Code Review', () => {
    expect(getStageGroupLabel('code-review-fix')).toBe('Code Review');
  });

  it('maps add-and-commit to Commit', () => {
    expect(getStageGroupLabel('add-and-commit')).toBe('Commit');
  });

  it('maps detour-sync to Commit', () => {
    expect(getStageGroupLabel('detour-sync')).toBe('Commit');
  });

  it('formats unknown command IDs as title case', () => {
    expect(getStageGroupLabel('unit-tests')).toBe('Unit Tests');
    expect(getStageGroupLabel('cleanup')).toBe('Cleanup');
    expect(getStageGroupLabel('review-changes')).toBe('Review Changes');
    expect(getStageGroupLabel('deslop')).toBe('Deslop');
  });
});

describe('STAGE_GROUP_ORDER', () => {
  it('has the expected canonical order', () => {
    expect([...STAGE_GROUP_ORDER]).toEqual([
      'Branch',
      'Plan',
      'Implement',
      'Code Review',
      'Commit',
    ]);
  });
});

describe('deriveStageGroups', () => {
  it('returns empty array for empty sub-runs', () => {
    expect(deriveStageGroups([])).toEqual([]);
  });

  it('skips sub-runs without a stage', () => {
    const result = deriveStageGroups([
      { status: 'running' },
      { stage: undefined, status: 'finished' },
    ]);
    expect(result).toEqual([]);
  });

  it('collapses multiple backend stages into a single group', () => {
    const result = deriveStageGroups([
      { stage: 'branch-gen', status: 'finished' },
      { stage: 'branch', status: 'finished' },
    ]);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({ label: 'Branch', status: 'finished' });
  });

  it('derives stages in canonical order', () => {
    const result = deriveStageGroups([
      { stage: 'implement', status: 'running' },
      { stage: 'plan', status: 'finished' },
      { stage: 'branch-gen', status: 'finished' },
    ]);
    expect(result.map((s) => s.label)).toEqual(['Branch', 'Plan', 'Implement']);
  });

  it('appends custom stages after canonical stages', () => {
    const result = deriveStageGroups([
      { stage: 'plan', status: 'finished' },
      { stage: 'implement', status: 'finished' },
      { stage: 'unit-tests', status: 'running' },
    ]);
    expect(result.map((s) => s.label)).toEqual(['Plan', 'Implement', 'Unit Tests']);
    expect(result[2].status).toBe('running');
  });

  it('merges status: running beats finished', () => {
    const result = deriveStageGroups([
      { stage: 'plan', status: 'finished' },
      { stage: 'plan-validation', status: 'running' },
    ]);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({ label: 'Plan', status: 'running' });
  });

  it('merges status: error beats running', () => {
    const result = deriveStageGroups([
      { stage: 'code-review', status: 'running' },
      { stage: 'code-review-fix', status: 'error' },
    ]);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({ label: 'Code Review', status: 'error' });
  });

  it('merges status: both finished stays finished', () => {
    const result = deriveStageGroups([
      { stage: 'plan', status: 'finished' },
      { stage: 'plan-validation', status: 'finished' },
      { stage: 'plan-decompose', status: 'finished' },
    ]);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({ label: 'Plan', status: 'finished' });
  });

  it('handles a full workflow with mixed statuses', () => {
    const result = deriveStageGroups([
      { stage: 'branch-gen', status: 'finished' },
      { stage: 'branch', status: 'finished' },
      { stage: 'plan', status: 'finished' },
      { stage: 'plan-validation', status: 'finished' },
      { stage: 'plan-decompose', status: 'finished' },
      { stage: 'implement', status: 'running' },
    ]);
    expect(result).toEqual([
      { label: 'Branch', status: 'finished' },
      { label: 'Plan', status: 'finished' },
      { label: 'Implement', status: 'running' },
    ]);
  });

  it('collapses detour-sync into Commit group with add-and-commit', () => {
    const result = deriveStageGroups([
      { stage: 'implement', status: 'finished' },
      { stage: 'add-and-commit', status: 'finished' },
      { stage: 'detour-sync', status: 'finished' },
    ]);
    expect(result).toEqual([
      { label: 'Implement', status: 'finished' },
      { label: 'Commit', status: 'finished' },
    ]);
  });

  it('shows Commit as running when detour-sync is running', () => {
    const result = deriveStageGroups([
      { stage: 'add-and-commit', status: 'finished' },
      { stage: 'detour-sync', status: 'running' },
    ]);
    expect(result).toEqual([
      { label: 'Commit', status: 'running' },
    ]);
  });

  it('only includes stages present in sub-runs', () => {
    const result = deriveStageGroups([
      { stage: 'implement', status: 'finished' },
    ]);
    expect(result).toEqual([{ label: 'Implement', status: 'finished' }]);
  });

  it('maps aborted sub-run status to error', () => {
    const result = deriveStageGroups([
      { stage: 'plan', status: 'finished' },
      { stage: 'implement', status: 'aborted' },
    ]);
    expect(result[1]).toEqual({ label: 'Implement', status: 'error' });
  });

  it('maps paused sub-run status to pending', () => {
    const result = deriveStageGroups([
      { stage: 'implement', status: 'paused' },
    ]);
    expect(result[0]).toEqual({ label: 'Implement', status: 'pending' });
  });
});

describe('mapRunStatus', () => {
  it('maps running to running', () => {
    expect(mapRunStatus('running')).toBe('running');
  });

  it('maps finished to finished', () => {
    expect(mapRunStatus('finished')).toBe('finished');
  });

  it('maps error to error', () => {
    expect(mapRunStatus('error')).toBe('error');
  });

  it('maps aborted to error', () => {
    expect(mapRunStatus('aborted')).toBe('error');
  });

  it('maps queued to pending', () => {
    expect(mapRunStatus('queued')).toBe('pending');
  });

  it('maps paused to pending', () => {
    expect(mapRunStatus('paused')).toBe('pending');
  });

  it('maps unknown values to pending', () => {
    expect(mapRunStatus('something_else')).toBe('pending');
  });
});
