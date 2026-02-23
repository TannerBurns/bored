import { describe, it, expect } from 'vitest';
import { formatCost, formatNumber, formatDuration, formatDateLabel } from './DashboardView';

describe('formatCost', () => {
  it.each([
    [0.001, '$0.0010'],
    [0.009, '$0.0090'],
    [0.05, '$0.050'],
    [0.999, '$0.999'],
    [1.0, '$1.00'],
    [12.5, '$12.50'],
    [100.123, '$100.12'],
  ] as const)('formats %f as %s', (input, expected) => {
    expect(formatCost(input)).toBe(expected);
  });

  it('uses 4 decimal places for sub-cent values', () => {
    expect(formatCost(0.0001)).toBe('$0.0001');
  });

  it('uses 2 decimal places for values >= $1', () => {
    expect(formatCost(5.678)).toBe('$5.68');
  });
});

describe('formatNumber', () => {
  it.each([
    [0, '0'],
    [999, '999'],
    [1000, '1.0K'],
    [1500, '1.5K'],
    [10000, '10.0K'],
    [1000000, '1.0M'],
    [2500000, '2.5M'],
  ] as const)('formats %d as %s', (input, expected) => {
    expect(formatNumber(input)).toBe(expected);
  });
});

describe('formatDuration', () => {
  it.each([
    [0, '0s'],
    [30, '30s'],
    [59, '59s'],
    [60, '1m'],
    [90, '2m'],
    [3599, '60m'],
    [3600, '1.0h'],
    [7200, '2.0h'],
    [5400, '1.5h'],
  ] as const)('formats %d seconds as %s', (input, expected) => {
    expect(formatDuration(input)).toBe(expected);
  });
});

describe('formatDateLabel', () => {
  it('formats ISO date to short month + day', () => {
    const result = formatDateLabel('2025-01-15');
    expect(result).toBe('Jan 15');
  });

  it('formats another date correctly', () => {
    const result = formatDateLabel('2025-12-25');
    expect(result).toBe('Dec 25');
  });
});
