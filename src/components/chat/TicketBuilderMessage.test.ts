import { describe, it, expect } from 'vitest';
import { parseTicketBuilderJsonFromText } from './TicketBuilderMessage';

describe('parseTicketBuilderJsonFromText', () => {
  it('accepts updates-only payloads (no top-level tickets)', () => {
    const raw = '{"updates":[{"ticket_id":"abc","title":"Renamed"}]}';
    const r = parseTicketBuilderJsonFromText(raw);
    expect(r).not.toBeNull();
    expect(r!.updates).toHaveLength(1);
    expect(r!.updates[0].ticket_id).toBe('abc');
    expect(r!.tickets).toEqual([]);
    expect(r!.epics).toEqual([]);
  });

  it('accepts epics-only payloads', () => {
    const raw =
      '{"epics":[{"name":"Phase 1","tickets":[{"title":"T","description":"D","priority":"low"}]}]}';
    const r = parseTicketBuilderJsonFromText(raw);
    expect(r).not.toBeNull();
    expect(r!.epics).toHaveLength(1);
    expect(r!.epics[0].name).toBe('Phase 1');
    expect(r!.epics[0].tickets).toHaveLength(1);
  });

  it('returns null when all top-level arrays are empty', () => {
    expect(parseTicketBuilderJsonFromText('{"tickets":[],"epics":[],"updates":[]}')).toBeNull();
  });
});
