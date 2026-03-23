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
  } else if ((parsed.create_fix_task as Record<string, unknown>)?.title !== undefined) {
    const t = parsed.create_fix_task as Record<string, unknown>;
    tasks.push({
      title: (t.title as string) || 'Fix task',
      description: t.description as string | undefined,
      acceptanceCriteria: (t.acceptance_criteria || t.acceptanceCriteria) as string[] | undefined,
    });
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

  // Bare inline JSON — only if no wrapped blocks were found.
  // Anchored to `{"key"` so stray braces like ${REPO} don't match.
  if (blocksToRemove.length === 0) {
    const bareStartRegex = /\{\s*"(?:create_fix_tasks|create_fix_task|run_command|start_app|stop_app)"/g;
    while ((match = bareStartRegex.exec(content)) !== null) {
      const startIdx = match.index;
      const balanced = extractBalancedJson(content.slice(startIdx));
      if (balanced) {
        try {
          const parsed = JSON.parse(balanced);
          if (processJsonMatch(parsed, tasks, commands)) {
            blocksToRemove.push(balanced);
            continue;
          }
        } catch {
          // Invalid JSON — fall through to malformed handling below
        }
      }

      // Malformed JSON fallback: the model often emits unescaped `"` inside
      // backtick-quoted code in the description, producing invalid JSON that
      // neither extractBalancedJson nor JSON.parse can handle. Extract title
      // via string scanning and strip the block from content.
      if (match[0].includes('create_fix_task')) {
        const rest = content.slice(startIdx);
        const extracted = extractFixTaskFromMalformed(rest);
        if (extracted) {
          tasks.push(extracted.task);
          blocksToRemove.push(rest.trimEnd());
        }
      }
    }
  }

  for (const block of blocksToRemove) {
    cleanedContent = cleanedContent.replace(block, '');
  }

  return { cleanedContent: cleanedContent.trim(), tasks, commands };
}

/** Extract a fix task from malformed JSON using string scanning.
 *  Handles the common case where the model emits unescaped `"` inside
 *  backtick-quoted code in the description string. */
function extractFixTaskFromMalformed(
  text: string,
): { task: ParsedFixTask } | null {
  const titleMarker = '"title"';
  const titleIdx = text.indexOf(titleMarker);
  if (titleIdx === -1) return null;

  const afterTitle = text.slice(titleIdx + titleMarker.length);
  const colonMatch = afterTitle.match(/^\s*:\s*"/);
  if (!colonMatch) return null;

  const titleStart = colonMatch[0].length;
  const titleBody = afterTitle.slice(titleStart);
  const titleEnd = titleBody.indexOf('"');
  if (titleEnd === -1) return null;

  const title = titleBody.slice(0, titleEnd);
  if (!title) return null;

  let description: string | undefined;
  const descMarker = '"description"';
  const descIdx = text.indexOf(descMarker, titleIdx + titleMarker.length);
  if (descIdx !== -1) {
    const afterDesc = text.slice(descIdx + descMarker.length);
    const descColonMatch = afterDesc.match(/^\s*:\s*"/);
    if (descColonMatch) {
      const descBody = afterDesc.slice(descColonMatch[0].length);
      // Walk backward from end of text past `}`, `]`, whitespace to find
      // the `"` that closes the description value.
      let end = descBody.length;
      while (end > 0 && '}] \n\r'.includes(descBody[end - 1])) end--;
      if (end > 0 && descBody[end - 1] === '"') end--;
      if (end > 0) {
        description = descBody
          .slice(0, end)
          .replace(/\\n/g, '\n')
          .replace(/\\t/g, '\t')
          .replace(/\\"/g, '"')
          .replace(/\\\\/g, '\\');
      }
    }
  }

  return { task: { title, description } };
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
