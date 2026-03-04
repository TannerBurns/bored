import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cn, formatRelativeTime, getTimeAgo, formatDuration, normalizeDependencies } from './utils';

describe('cn', () => {
  it('merges class names', () => {
    expect(cn('foo', 'bar')).toBe('foo bar');
  });

  it('handles conditional classes', () => {
    expect(cn('base', true && 'included', false && 'excluded')).toBe('base included');
  });

  it('handles undefined and null', () => {
    expect(cn('base', undefined, null, 'end')).toBe('base end');
  });

  it('handles empty input', () => {
    expect(cn()).toBe('');
  });

  it('handles object syntax', () => {
    expect(cn({ active: true, disabled: false })).toBe('active');
  });

  it('handles array syntax', () => {
    expect(cn(['foo', 'bar'])).toBe('foo bar');
  });

  it('handles mixed inputs', () => {
    expect(cn('base', ['arr1', 'arr2'], { obj: true })).toBe('base arr1 arr2 obj');
  });
});

describe('formatRelativeTime', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2024-06-15T12:00:00Z'));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns "just now" for times less than 60 seconds ago', () => {
    expect(formatRelativeTime(new Date('2024-06-15T11:59:30Z'))).toBe('just now');
    expect(formatRelativeTime(new Date('2024-06-15T11:59:59Z'))).toBe('just now');
    expect(formatRelativeTime(new Date('2024-06-15T12:00:00Z'))).toBe('just now');
  });

  it('returns minutes ago for 1-59 minutes', () => {
    expect(formatRelativeTime(new Date('2024-06-15T11:59:00Z'))).toBe('1m ago');
    expect(formatRelativeTime(new Date('2024-06-15T11:30:00Z'))).toBe('30m ago');
    expect(formatRelativeTime(new Date('2024-06-15T11:01:00Z'))).toBe('59m ago');
  });

  it('returns hours ago for 1-23 hours', () => {
    expect(formatRelativeTime(new Date('2024-06-15T11:00:00Z'))).toBe('1h ago');
    expect(formatRelativeTime(new Date('2024-06-15T00:00:00Z'))).toBe('12h ago');
    expect(formatRelativeTime(new Date('2024-06-14T13:00:00Z'))).toBe('23h ago');
  });

  it('returns "Yesterday" for 24-47 hours', () => {
    expect(formatRelativeTime(new Date('2024-06-14T12:00:00Z'))).toBe('Yesterday');
    expect(formatRelativeTime(new Date('2024-06-14T00:00:01Z'))).toBe('Yesterday');
  });

  it('returns days ago for 48+ hours', () => {
    expect(formatRelativeTime(new Date('2024-06-13T12:00:00Z'))).toBe('2d ago');
    expect(formatRelativeTime(new Date('2024-06-08T12:00:00Z'))).toBe('7d ago');
  });

  it('accepts string dates', () => {
    expect(formatRelativeTime('2024-06-15T11:59:00Z')).toBe('1m ago');
  });

  it('accepts Date objects', () => {
    expect(formatRelativeTime(new Date('2024-06-15T11:00:00Z'))).toBe('1h ago');
  });
});

describe('getTimeAgo', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns seconds ago for times less than 60 seconds', () => {
    const now = new Date('2024-01-15T12:00:00Z');
    vi.setSystemTime(now);

    expect(getTimeAgo(new Date('2024-01-15T11:59:30Z'))).toBe('30s ago');
    expect(getTimeAgo(new Date('2024-01-15T11:59:59Z'))).toBe('1s ago');
    expect(getTimeAgo(new Date('2024-01-15T11:59:01Z'))).toBe('59s ago');
  });

  it('returns 0s ago for current time', () => {
    const now = new Date('2024-01-15T12:00:00Z');
    vi.setSystemTime(now);

    expect(getTimeAgo(now)).toBe('0s ago');
  });

  it('returns minutes ago for times between 1 and 59 minutes', () => {
    const now = new Date('2024-01-15T12:00:00Z');
    vi.setSystemTime(now);

    expect(getTimeAgo(new Date('2024-01-15T11:59:00Z'))).toBe('1m ago');
    expect(getTimeAgo(new Date('2024-01-15T11:30:00Z'))).toBe('30m ago');
    expect(getTimeAgo(new Date('2024-01-15T11:01:00Z'))).toBe('59m ago');
  });

  it('returns hours ago for times between 1 and 23 hours', () => {
    const now = new Date('2024-01-15T12:00:00Z');
    vi.setSystemTime(now);

    expect(getTimeAgo(new Date('2024-01-15T11:00:00Z'))).toBe('1h ago');
    expect(getTimeAgo(new Date('2024-01-15T00:00:00Z'))).toBe('12h ago');
    expect(getTimeAgo(new Date('2024-01-14T13:00:00Z'))).toBe('23h ago');
  });

  it('returns days ago for times 24 hours or more', () => {
    const now = new Date('2024-01-15T12:00:00Z');
    vi.setSystemTime(now);

    expect(getTimeAgo(new Date('2024-01-14T12:00:00Z'))).toBe('1d ago');
    expect(getTimeAgo(new Date('2024-01-08T12:00:00Z'))).toBe('7d ago');
    expect(getTimeAgo(new Date('2023-12-16T12:00:00Z'))).toBe('30d ago');
  });
});

describe('formatDuration', () => {
  it('returns seconds only for durations less than 60 seconds', () => {
    const start = new Date('2024-01-15T12:00:00Z');

    expect(formatDuration(start, new Date('2024-01-15T12:00:00Z'))).toBe('0s');
    expect(formatDuration(start, new Date('2024-01-15T12:00:01Z'))).toBe('1s');
    expect(formatDuration(start, new Date('2024-01-15T12:00:30Z'))).toBe('30s');
    expect(formatDuration(start, new Date('2024-01-15T12:00:59Z'))).toBe('59s');
  });

  it('returns minutes and seconds for durations between 1 and 59 minutes', () => {
    const start = new Date('2024-01-15T12:00:00Z');

    expect(formatDuration(start, new Date('2024-01-15T12:01:00Z'))).toBe('1m 0s');
    expect(formatDuration(start, new Date('2024-01-15T12:01:30Z'))).toBe('1m 30s');
    expect(formatDuration(start, new Date('2024-01-15T12:30:45Z'))).toBe('30m 45s');
    expect(formatDuration(start, new Date('2024-01-15T12:59:59Z'))).toBe('59m 59s');
  });

  it('returns hours and minutes for durations of 60 minutes or more', () => {
    const start = new Date('2024-01-15T12:00:00Z');

    expect(formatDuration(start, new Date('2024-01-15T13:00:00Z'))).toBe('1h 0m');
    expect(formatDuration(start, new Date('2024-01-15T13:30:00Z'))).toBe('1h 30m');
    expect(formatDuration(start, new Date('2024-01-15T14:45:00Z'))).toBe('2h 45m');
    expect(formatDuration(start, new Date('2024-01-16T12:00:00Z'))).toBe('24h 0m');
  });

  it('handles large durations', () => {
    const start = new Date('2024-01-15T00:00:00Z');
    const end = new Date('2024-01-17T12:30:00Z');

    expect(formatDuration(start, end)).toBe('60h 30m');
  });
});

describe('normalizeDependencies', () => {
  it('returns empty array for null', () => {
    expect(normalizeDependencies(null)).toEqual([]);
  });

  it('returns empty array for undefined', () => {
    expect(normalizeDependencies(undefined)).toEqual([]);
  });

  it('returns empty array for empty string', () => {
    expect(normalizeDependencies('')).toEqual([]);
  });

  it('wraps string in array (legacy format)', () => {
    expect(normalizeDependencies('Epic A')).toEqual(['Epic A']);
  });

  it('returns array as-is when valid', () => {
    expect(normalizeDependencies(['Epic A', 'Epic B'])).toEqual(['Epic A', 'Epic B']);
  });

  it('returns empty array for empty array', () => {
    expect(normalizeDependencies([])).toEqual([]);
  });

  it('filters out empty strings from array', () => {
    expect(normalizeDependencies(['Epic A', '', 'Epic B', ''])).toEqual(['Epic A', 'Epic B']);
  });

  it('filters out null-ish values from array', () => {
    expect(normalizeDependencies(['Epic A', null as unknown as string, 'Epic B'])).toEqual(['Epic A', 'Epic B']);
  });
});
