import { describe, it, expect } from 'vitest';
import { getColumnColors, getColumnBg, getColumnGlow } from './constants';

describe('getColumnColors', () => {
  it.each([
    ['Backlog', { bg: 'bg-board-text-muted', dot: 'bg-board-text-muted', glow: '' }],
    ['Ready', { bg: 'bg-status-info', dot: 'bg-status-info', glow: '' }],
    ['In Progress', { bg: 'bg-status-warning', dot: 'bg-status-warning', glow: 'glow-warning' }],
    ['Blocked', { bg: 'bg-status-error', dot: 'bg-status-error', glow: 'glow-error' }],
    ['Review', { bg: 'bg-purple-500', dot: 'bg-purple-500', glow: '' }],
    ['Done', { bg: 'bg-status-success', dot: 'bg-status-success', glow: 'glow-success' }],
  ] as const)('returns correct colors for "%s"', (name, expected) => {
    expect(getColumnColors(name)).toEqual(expected);
  });

  it('returns default colors for unknown column names', () => {
    const defaults = { bg: 'bg-board-text-muted', dot: 'bg-board-text-muted', glow: '' };
    expect(getColumnColors('Unknown')).toEqual(defaults);
    expect(getColumnColors('')).toEqual(defaults);
    expect(getColumnColors('Custom Column')).toEqual(defaults);
  });
});

describe('getColumnBg', () => {
  it('returns only the bg class for a known column', () => {
    expect(getColumnBg('Done')).toBe('bg-status-success');
    expect(getColumnBg('In Progress')).toBe('bg-status-warning');
  });

  it('returns default bg for unknown column', () => {
    expect(getColumnBg('Nonexistent')).toBe('bg-board-text-muted');
  });
});

describe('getColumnGlow', () => {
  it('returns glow class for columns that have one', () => {
    expect(getColumnGlow('Done')).toBe('glow-success');
    expect(getColumnGlow('In Progress')).toBe('glow-warning');
    expect(getColumnGlow('Blocked')).toBe('glow-error');
  });

  it('returns empty string for columns without glow', () => {
    expect(getColumnGlow('Backlog')).toBe('');
    expect(getColumnGlow('Ready')).toBe('');
    expect(getColumnGlow('Review')).toBe('');
  });

  it('returns empty string for unknown columns', () => {
    expect(getColumnGlow('Whatever')).toBe('');
  });
});
