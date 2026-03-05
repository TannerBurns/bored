import { describe, it, expect } from 'vitest';
import { looksLikePlanResponse } from './PlanBuilderMessage';

describe('looksLikePlanResponse', () => {
  it('returns true when content has both epics and overview keys', () => {
    const content = JSON.stringify({
      overview: 'Build a project',
      epics: [{ title: 'Epic 1', description: 'Desc', dependsOn: [], tickets: [] }],
    });
    expect(looksLikePlanResponse(content)).toBe(true);
  });

  it('returns false for plain text without plan structure', () => {
    expect(looksLikePlanResponse('Hello, how can I help you?')).toBe(false);
  });

  it('returns false when only epics key is present', () => {
    expect(looksLikePlanResponse('{"epics":[]}')).toBe(false);
  });

  it('returns false when only overview key is present', () => {
    expect(looksLikePlanResponse('{"overview":"Plan"}')).toBe(false);
  });

  it('returns true for content with preamble text before JSON', () => {
    const content = `Here is the generated plan:\n\`\`\`json\n{"overview":"Plan","epics":[]}\n\`\`\``;
    expect(looksLikePlanResponse(content)).toBe(true);
  });

  it('returns false for empty string', () => {
    expect(looksLikePlanResponse('')).toBe(false);
  });

  it('returns true when keys appear in raw JSON without code block', () => {
    const content = '{"overview":"Build it","epics":[{"title":"E1","description":"D","dependsOn":[],"tickets":[]}]}';
    expect(looksLikePlanResponse(content)).toBe(true);
  });
});
