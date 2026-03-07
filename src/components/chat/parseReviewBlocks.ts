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

export function parseReviewBlocks(content: string): ParsedReviewBlocks {
  const tasks: ParsedFixTask[] = [];
  const commands: ParsedCommand[] = [];
  let cleanedContent = content;
  const blocksToRemove: string[] = [];

  const codeBlockRegex = /```(?:json)?\s*\n?\s*(\{[\s\S]*?\})\s*\n?\s*```/g;
  let match;
  while ((match = codeBlockRegex.exec(content)) !== null) {
    try {
      const parsed = JSON.parse(match[1]);
      if (parsed.create_fix_task) {
        const t = parsed.create_fix_task;
        tasks.push({
          title: t.title || 'Fix task',
          description: t.description,
          acceptanceCriteria: t.acceptance_criteria || t.acceptanceCriteria,
        });
        blocksToRemove.push(match[0]);
      } else if (parsed.create_fix_tasks?.tasks) {
        for (const t of parsed.create_fix_tasks.tasks) {
          tasks.push({
            title: t.title || 'Fix task',
            description: t.description,
            acceptanceCriteria: t.acceptance_criteria || t.acceptanceCriteria,
          });
        }
        blocksToRemove.push(match[0]);
      } else if (parsed.run_command) {
        commands.push({
          type: 'run_command',
          command: parsed.run_command.command,
        });
        blocksToRemove.push(match[0]);
      } else if (parsed.start_app) {
        commands.push({
          type: 'start_app',
          command: parsed.start_app.command,
          port: parsed.start_app.port,
        });
        blocksToRemove.push(match[0]);
      } else if (parsed.stop_app !== undefined) {
        commands.push({ type: 'stop_app' });
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
