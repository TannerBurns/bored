import { useState, useEffect, useCallback } from 'react';
import { useBoardStore } from '../stores/boardStore';
import { useSpecStore } from '../stores/specStore';
import { getProjects, getWorkspaces, getBoards, getTickets, getApiConfig, getRecentRunsWithContext, getColumns } from '../lib/tauri';
import { logger } from '../lib/logger';
import type { Project, Workspace, AgentRunWithContext } from '../types';

interface UseAppDataResult {
  projects: Project[];
  workspaces: Workspace[];
  recentRuns: AgentRunWithContext[];
  isDataLoaded: boolean;
  apiConfig: { url: string; token: string } | null;
  setProjects: React.Dispatch<React.SetStateAction<Project[]>>;
  setRecentRuns: React.Dispatch<React.SetStateAction<AgentRunWithContext[]>>;
  loadProjects: () => Promise<void>;
}

export function useAppData(
  setColumns: (columns: ReturnType<typeof getColumns> extends Promise<infer T> ? T : never) => void,
  setTickets: (tickets: ReturnType<typeof getTickets> extends Promise<infer T> ? T : never) => void
): UseAppDataResult {
  const [projects, setProjects] = useState<Project[]>([]);
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [recentRuns, setRecentRuns] = useState<AgentRunWithContext[]>([]);
  const [isDataLoaded, setIsDataLoaded] = useState(false);
  const [apiConfig, setApiConfig] = useState<{ url: string; token: string } | null>(null);

  const storeSetBoards = useBoardStore((s) => s.setBoards);
  const storeSetCurrentBoard = useBoardStore((s) => s.setCurrentBoard);

  const loadProjects = useCallback(async () => {
    try {
      const projectsData = await getProjects();
      setProjects(projectsData);
    } catch (error) {
      logger.error('Failed to load projects:', error);
    }
  }, []);

  useEffect(() => {
    const loadData = async () => {
      const getApiConfigWithRetry = async (maxRetries = 5): Promise<{ url: string; token: string; port: number }> => {
        let lastError: unknown;
        for (let attempt = 0; attempt < maxRetries; attempt++) {
          try {
            return await getApiConfig();
          } catch (error) {
            lastError = error;
            const delay = Math.min(100 * Math.pow(2, attempt), 2000);
            logger.debug(`API not ready, retrying in ${delay}ms (attempt ${attempt + 1}/${maxRetries})`);
            await new Promise(resolve => setTimeout(resolve, delay));
          }
        }
        throw lastError;
      };
      
      try {
        const config = await getApiConfigWithRetry();
        setApiConfig(config);
        
        const [projectsData, workspacesData, boardsData] = await Promise.all([
          getProjects(),
          getWorkspaces(),
          getBoards(),
        ]);
        setProjects(projectsData);
        setWorkspaces(workspacesData);
        storeSetBoards(boardsData);
        
        if (boardsData.length > 0) {
          const firstBoard = boardsData[0];
          storeSetCurrentBoard(firstBoard);
          
          const [columnsData, ticketsData] = await Promise.all([
            getColumns(firstBoard.id),
            getTickets(firstBoard.id),
          ]);
          setColumns(columnsData);
          setTickets(ticketsData);
        }
      } catch (error) {
        logger.error('Failed to load data:', error);
      }
      
      setIsDataLoaded(true);
    };
    
    loadData();
  }, [storeSetBoards, storeSetCurrentBoard, setColumns, setTickets]);

  return {
    projects,
    workspaces,
    recentRuns,
    isDataLoaded,
    apiConfig,
    setProjects,
    setRecentRuns,
    loadProjects,
  };
}

export function useAgentsData(
  activeNav: string,
  setProjects: React.Dispatch<React.SetStateAction<Project[]>>,
  setRecentRuns: React.Dispatch<React.SetStateAction<AgentRunWithContext[]>>
) {
  useEffect(() => {
    if (activeNav !== 'agents') return;
    
    const loadRecentRuns = async () => {
      try {
        const runs = await getRecentRunsWithContext(50);
        setRecentRuns(runs);
      } catch (error) {
        logger.error('Failed to load recent runs:', error);
      }
    };
    
    loadRecentRuns();
    const interval = setInterval(loadRecentRuns, 5000);
    return () => clearInterval(interval);
  }, [activeNav, setRecentRuns]);

  useEffect(() => {
    if (activeNav !== 'agents') return;
    
    const refreshProjects = async () => {
      try {
        const projectsData = await getProjects();
        setProjects(projectsData);
      } catch (error) {
        logger.error('Failed to refresh projects:', error);
      }
    };
    
    refreshProjects();
  }, [activeNav, setProjects]);
}

export function useSpecsData(activeNav: string) {
  const loadAllSpecs = useSpecStore((s) => s.loadAllSpecs);
  const selectSpec = useSpecStore((s) => s.selectSpec);
  const currentSpec = useSpecStore((s) => s.currentSpec);

  useEffect(() => {
    if (activeNav !== 'specs') return;
    loadAllSpecs();
  }, [activeNav, loadAllSpecs]);

  useEffect(() => {
    if (activeNav !== 'specs') {
      selectSpec(null);
    }
  }, [activeNav, selectSpec]);

  return { loadAllSpecs, selectSpec, currentSpec };
}
