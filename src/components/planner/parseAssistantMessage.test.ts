import { describe, it, expect } from 'vitest';
import { parseAssistantMessage } from './parseAssistantMessage';

describe('parseAssistantMessage', () => {
  describe('structured JSON format', () => {
    it('parses observations and questions from JSON', () => {
      const content = JSON.stringify({
        observations: 'Found auth module in src/auth/',
        questions: '1. Which provider?\n   - A) Google\n   - B) GitHub',
      });

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBe('Found auth module in src/auth/');
      expect(result.questions).toContain('Which provider');
      expect(result.preamble).toBeNull();
    });

    it('parses observations-only JSON (no questions)', () => {
      const content = JSON.stringify({
        observations: 'Explored the codebase.',
      });

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBe('Explored the codebase.');
      expect(result.questions).toBeNull();
    });

    it('parses questions-only JSON (no observations)', () => {
      const content = JSON.stringify({
        questions: 'What framework?',
      });

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBeNull();
      expect(result.questions).toBe('What framework?');
    });

    it('treats empty string fields as null but keeps structure flag', () => {
      const content = JSON.stringify({
        observations: '',
        questions: '',
      });

      const result = parseAssistantMessage(content);

      // Fields exist so hasStructure is true, but values coerce to null
      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBeNull();
      expect(result.questions).toBeNull();
    });

    it('ignores JSON without observations or questions fields', () => {
      const content = JSON.stringify({ foo: 'bar', baz: 123 });

      const result = parseAssistantMessage(content);

      // Should fall through to legacy parsing (no structure found)
      expect(result.hasStructure).toBe(false);
    });

    it('falls through on invalid JSON starting with {', () => {
      const content = '{not valid json at all';

      const result = parseAssistantMessage(content);

      // Should not crash, falls through to legacy parsing
      expect(result.hasStructure).toBe(false);
    });
  });

  describe('legacy markdown format', () => {
    it('parses ## Observations and ## Questions headers', () => {
      const content =
        '## Observations\nFound patterns in the codebase.\n\n## Questions\nWhich approach do you prefer?';

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBe('Found patterns in the codebase.');
      expect(result.questions).toBe('Which approach do you prefer?');
    });

    it('parses observations only (no questions section)', () => {
      const content =
        '## Observations\nThe project uses React with TypeScript.';

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBe('The project uses React with TypeScript.');
      expect(result.questions).toBeNull();
    });

    it('stops observations at ```json block', () => {
      const content =
        '## Observations\nSome findings.\n\n```json\n{"spec_complete": true}\n```';

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.observations).toBe('Some findings.');
    });

    it('extracts preamble before first section header', () => {
      const content =
        'Here is a long preamble that exceeds twenty characters.\n\n## Observations\nFindings here.';

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(true);
      expect(result.preamble).toBe(
        'Here is a long preamble that exceeds twenty characters.'
      );
      expect(result.observations).toBe('Findings here.');
    });

    it('ignores short preamble (<=20 chars)', () => {
      const content = 'Short.\n\n## Observations\nFindings.';

      const result = parseAssistantMessage(content);

      expect(result.preamble).toBeNull();
    });
  });

  describe('unstructured content', () => {
    it('returns no structure for plain text', () => {
      const content = 'What authentication method would you prefer?';

      const result = parseAssistantMessage(content);

      expect(result.hasStructure).toBe(false);
      expect(result.observations).toBeNull();
      expect(result.questions).toBeNull();
      expect(result.preamble).toBeNull();
    });
  });
});
