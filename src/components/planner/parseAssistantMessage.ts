export interface ParsedMessage {
  hasStructure: boolean;
  observations: string | null;
  questions: string | null;
  preamble: string | null;
}

export function parseAssistantMessage(content: string): ParsedMessage {
  // Try structured JSON first (new format: {"observations": "...", "questions": "..."})
  if (content.startsWith('{')) {
    try {
      const json = JSON.parse(content);
      if (json.observations !== undefined || json.questions !== undefined) {
        return {
          hasStructure: true,
          observations: json.observations || null,
          questions: json.questions || null,
          preamble: null,
        };
      }
    } catch {
      // Fall through to legacy parsing
    }
  }

  // Legacy: scan for ## Observations / ## Questions headers
  const observationsStart = content.search(/##\s*Observations/i);
  const questionsStart = content.search(/##\s*Questions/i);
  
  let observations: string | null = null;
  let questions: string | null = null;
  
  if (observationsStart !== -1) {
    const headingEnd = content.indexOf('\n', observationsStart);
    if (headingEnd !== -1) {
      const afterHeading = content.substring(headingEnd + 1);
      const nextSection = afterHeading.search(/##\s*(Observations|Questions)/i);
      const jsonBlock = afterHeading.search(/```json/i);
      const end = nextSection !== -1 ? nextSection : (jsonBlock !== -1 ? jsonBlock : afterHeading.length);
      observations = afterHeading.substring(0, end).trim() || null;
    }
  }
  
  if (questionsStart !== -1) {
    const headingEnd = content.indexOf('\n', questionsStart);
    if (headingEnd !== -1) {
      const afterHeading = content.substring(headingEnd + 1);
      const nextSection = afterHeading.search(/##\s*(Observations|Questions)/i);
      const jsonBlock = afterHeading.search(/```json/i);
      const end = nextSection !== -1 ? nextSection : (jsonBlock !== -1 ? jsonBlock : afterHeading.length);
      questions = afterHeading.substring(0, end).trim() || null;
    }
  }
  
  const hasStructure = !!(observations || questions);
  
  let preamble: string | null = null;
  if (hasStructure) {
    const firstIdx = Math.min(
      observationsStart !== -1 ? observationsStart : Infinity,
      questionsStart !== -1 ? questionsStart : Infinity
    );
    if (firstIdx > 0 && firstIdx !== Infinity) {
      const before = content.substring(0, firstIdx).trim();
      if (before.length > 20) preamble = before;
    }
  }
  
  return { hasStructure, observations, questions, preamble };
}
