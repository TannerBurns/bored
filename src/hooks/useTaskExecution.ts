import { useState, useEffect, useRef, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Task, AgentRun } from '../types';
import { deriveStageGroups, type StageGroup } from '../components/chat/stageLabels';

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

        const hasRunning = fetched.some((t) => t.status === 'in_progress');
        if (hasRunning && ticketId) {
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
  }, [taskIds.join(','), ticketId, isAllComplete]);

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
        return {
          task,
          stages: [{ label: 'Running', status: parentRun.status as 'running' }],
          currentStage: 'Running',
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
