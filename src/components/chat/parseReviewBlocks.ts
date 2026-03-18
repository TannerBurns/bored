export interface ParsedFixTask {
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
  if (parsed.create_fix_task) {
    const t = parsed.create_fix_task as Record<string, unknown>;
    tasks.push({
      title: (t.title as string) || 'Fix task',
      description: t.description as string | undefined,
      acceptanceCriteria: (t.acceptance_criteria || t.acceptanceCriteria) as string[] | undefined,
    });
    return true;
  } else if ((parsed.create_fix_tasks as Record<string, unknown>)?.tasks) {
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

  for (const block of blocksToRemove) {
    cleanedContent = cleanedContent.replace(block, '');
  }

  return { cleanedContent: cleanedContent.trim(), tasks, commands };
}
