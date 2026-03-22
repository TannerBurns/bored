interface ParsedFixTask {
  title: string;
  description?: string;
  acceptanceCriteria?: string[];
  status?: string;
}

export interface ParsedCommand {
  type: 'run_command' | 'start_app' | 'stop_app';
  command?: string;
  port?: number;
}

export interface ParsedReviewBlocks {
  cleanedContent: string;
  tasks: ParsedFixTask[];
  commands: ParsedCommand[];
}

function processJsonMatch(
  parsed: Record<string, unknown>,
  tasks: ParsedFixTask[],
  commands: ParsedCommand[],
): boolean {
  if ((parsed.create_fix_tasks as Record<string, unknown>)?.tasks) {
    const cft = parsed.create_fix_tasks as Record<string, unknown>;
    for (const t of cft.tasks as Record<string, unknown>[]) {
      tasks.push({
        title: (t.title as string) || 'Fix task',
        description: t.description as string | undefined,
        acceptanceCriteria: (t.acceptance_criteria || t.acceptanceCriteria) as string[] | undefined,
      });
    }
    return true;
  } else if (parsed.run_command) {
    const rc = parsed.run_command as Record<string, unknown>;
    commands.push({ type: 'run_command', command: rc.command as string });
    return true;
  } else if (parsed.start_app) {
    const sa = parsed.start_app as Record<string, unknown>;
    commands.push({ type: 'start_app', command: sa.command as string, port: sa.port as number | undefined });
    return true;
  } else if (parsed.stop_app !== undefined) {
    commands.push({ type: 'stop_app' });
    return true;
  }
  return false;
}

export function parseReviewBlocks(content: string): ParsedReviewBlocks {
  const tasks: ParsedFixTask[] = [];
  const commands: ParsedCommand[] = [];
  let cleanedContent = content;
  const blocksToRemove: string[] = [];

  // Match code fences: ```json ... ``` or ``` ... ```
  const codeBlockRegex = /```(?:json)?\s*\n?\s*(\{[\s\S]*?\})\s*\n?\s*```/g;
  let match;
  while ((match = codeBlockRegex.exec(content)) !== null) {
    try {
      const parsed = JSON.parse(match[1]);
      if (processJsonMatch(parsed, tasks, commands)) {
        blocksToRemove.push(match[0]);
      }
    } catch {
      // Not valid JSON, skip
    }
  }

  // Match <json> tags: <json> ... </json> or <json> ... ```
  const jsonTagRegex = /<json>\s*\n?\s*(\{[\s\S]*?\})\s*\n?\s*(?:<\/json>|```)/g;
  while ((match = jsonTagRegex.exec(content)) !== null) {
    try {
      const parsed = JSON.parse(match[1]);
      if (processJsonMatch(parsed, tasks, commands)) {
        blocksToRemove.push(match[0]);
      }
    } catch {
      // Not valid JSON, skip
    }
  }

  // Match bare inline JSON objects with known action keys (no wrappers).
  // Only attempt if we haven't already found blocks via the wrapped patterns.
  // Anchors to `{"key"` so stray braces like ${REPO} don't confuse the match.
  if (blocksToRemove.length === 0) {
    const bareStartRegex = /\{\s*"(?:create_fix_tasks|run_command|start_app|stop_app)"/g;
    while ((match = bareStartRegex.exec(content)) !== null) {
      const startIdx = match.index;
      const balanced = extractBalancedJson(content.slice(startIdx));
      if (!balanced) continue;
      try {
        const parsed = JSON.parse(balanced);
        if (processJsonMatch(parsed, tasks, commands)) {
          blocksToRemove.push(balanced);
        }
      } catch {
        // Not valid JSON, skip
      }
    }
  }

  for (const block of blocksToRemove) {
    cleanedContent = cleanedContent.replace(block, '');
  }

  return { cleanedContent: cleanedContent.trim(), tasks, commands };
}

/** Walk from the opening brace and find the matching closing brace,
 *  skipping braces inside JSON string literals. */
function extractBalancedJson(text: string): string | null {
  let depth = 0;
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inString) {
      if (ch === '\\') { i++; continue; }
      if (ch === '"') inString = false;
      continue;
    }
    if (ch === '"') { inString = true; continue; }
    if (ch === '{') depth++;
    else if (ch === '}') depth--;
    if (depth === 0) return text.slice(0, i + 1);
  }
  return null;
}
