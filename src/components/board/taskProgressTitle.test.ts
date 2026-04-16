import { describe, it, expect } from 'vitest';
import { taskProgressTitle } from './taskProgressTitle';

describe('taskProgressTitle', () => {
  it('uses child-ticket wording for epics', () => {
    expect(taskProgressTitle(true, 2, 5)).toBe('2 of 5 child tickets in Done');
  });

  it('uses task wording for non-epics', () => {
    expect(taskProgressTitle(false, 1, 4)).toBe('1 of 4 tasks completed');
  });
});
