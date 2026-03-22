import { useState, useEffect, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Task, AgentRun } from '../types';
import { deriveStageGroups, type StageGroup, type StageGroupStatus } from '../components/chat/stageLabels';
import type { RunStatus } from '../types';

export interface TaskWithStages {
  task: Task;
  stages: StageGroup[];
  currentStage: string | null;
}

export interface UseTaskExecutionReturn {
  tasks: TaskWithStages[];
  isAllComplete: boolean;
  isLoading: boolean;
}

const POLL_INTERVAL = 3000;

export function useTaskExecution(
  taskIds: string[],
  ticketId: string | undefined,
): UseTaskExecutionReturn {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [agentRuns, setAgentRuns] = useState<AgentRun[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const taskIdsKey = useMemo(() => taskIds.join(','), [taskIds]);
  const taskIdsRef = useRef(taskIds);
  taskIdsRef.current = taskIds;

  const isAllComplete = useMemo(
    () =>
      tasks.length > 0 &&
      tasks.every(
        (t) => t.status === 'completed' || t.status === 'failed',
      ),
    [tasks],
  );

  useEffect(() => {
    if (taskIds.length === 0) {
      setIsLoading(false);
      return;
    }

    let cancelled = false;

    const poll = async () => {
      try {
        const fetched = await invoke<Task[]>('get_tasks_by_ids', {
          taskIds: taskIdsRef.current,
        });
        if (cancelled) return;
        setTasks(fetched);
        setIsLoading(false);

        if (ticketId && fetched.some((t) => t.runId)) {
          const runs = await invoke<AgentRun[]>('get_agent_runs', {
            ticketId,
          });
          if (!cancelled) setAgentRuns(runs);
        }
      } catch {
        if (!cancelled) setIsLoading(false);
      }
    };

    poll();
    const interval = setInterval(() => {
      if (!isAllComplete) poll();
    }, POLL_INTERVAL);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  }, [taskIdsKey, ticketId, isAllComplete]);

  const tasksWithStages = useMemo<TaskWithStages[]>(() => {
    return tasks.map((task) => {
      if (!task.runId) {
        return { task, stages: [], currentStage: null };
      }

      const parentRun = agentRuns.find((r) => r.id === task.runId);
      const subRuns = agentRuns
        .filter((r) => r.parentRunId === task.runId)
        .sort(
          (a, b) =>
            new Date(a.startedAt).getTime() -
            new Date(b.startedAt).getTime(),
        );

      if (subRuns.length === 0 && parentRun) {
        const status = mapRunStatus(parentRun.status as RunStatus);
        const label = runStatusLabel(parentRun.status as RunStatus);
        return {
          task,
          stages: [{ label, status }],
          currentStage: status === 'running' ? label : null,
        };
      }

      const stages = deriveStageGroups(subRuns);
      const runningGroup = stages.find((s) => s.status === 'running');
      const currentStage = runningGroup?.label ?? null;

      return { task, stages, currentStage };
    });
  }, [tasks, agentRuns]);

  return { tasks: tasksWithStages, isAllComplete, isLoading };
}

function mapRunStatus(status: RunStatus): StageGroupStatus {
  switch (status) {
    case 'running': return 'running';
    case 'finished': return 'finished';
    case 'error':
    case 'aborted': return 'error';
    case 'queued':
    case 'paused':
    default: return 'pending';
  }
}

function runStatusLabel(status: RunStatus): string {
  switch (status) {
    case 'running': return 'Starting';
    case 'finished': return 'Finished';
    case 'error': return 'Failed';
    case 'aborted': return 'Aborted';
    case 'queued': return 'Queued';
    case 'paused': return 'Paused';
    default: return 'Running';
  }
}
