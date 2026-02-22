import { describe, it, expect } from 'vitest';
import { getCommandId, getTaskTypeLabel } from './index';

describe('getCommandId', () => {
  it('extracts ID from command: prefix', () => {
    expect(getCommandId('command:fix-lint')).toBe('fix-lint');
  });

  it('handles multi-segment IDs', () => {
    expect(getCommandId('command:sync-with-main')).toBe('sync-with-main');
  });

  it('returns null for custom type', () => {
    expect(getCommandId('custom')).toBeNull();
  });

  it('returns null for arbitrary string without prefix', () => {
    expect(getCommandId('something-else')).toBeNull();
  });

  it('returns empty string for bare command: prefix', () => {
    expect(getCommandId('command:')).toBe('');
  });
});

describe('getTaskTypeLabel', () => {
  it('returns Custom for custom type', () => {
    expect(getTaskTypeLabel('custom')).toBe('Custom');
  });

  it('title-cases a bare ID (serde/IPC format)', () => {
    expect(getTaskTypeLabel('fix-lint')).toBe('Fix Lint');
    expect(getTaskTypeLabel('sync-with-main')).toBe('Sync With Main');
    expect(getTaskTypeLabel('code-review')).toBe('Code Review');
    expect(getTaskTypeLabel('cleanup')).toBe('Cleanup');
  });

  it('title-cases a prefixed ID (DB format)', () => {
    expect(getTaskTypeLabel('command:fix-lint')).toBe('Fix Lint');
    expect(getTaskTypeLabel('command:sync-with-main')).toBe('Sync With Main');
  });

  it('title-cases underscore-separated IDs', () => {
    expect(getTaskTypeLabel('unknown_thing')).toBe('Unknown_thing');
  });
});
