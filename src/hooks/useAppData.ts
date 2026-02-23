import { useState, useEffect, useCallback } from 'react';
import { useBoardStore } from '../stores/boardStore';
import { useSpecStore } from '../stores/specStore';
import { getProjects, getBoards, getTickets, getApiConfig, getRecentRunsWithContext, getColumns } from '../lib/tauri';
import { logger } from '../lib/logger';
import type { Project, AgentRunWithContext } from '../types';

interface UseAppDataResult {
  projects: Project[];
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
  const [recentRuns, setRecentRuns] = useState<AgentRunWithContext[]>([]);
  const [isDataLoaded, setIsDataLoaded] = useState(false);
  const [apiConfig, setApiConfig] = useState<{ url: string; token: string } | null>(null);

  const { setBoards: storeSetBoards, setCurrentBoard: storeSetCurrentBoard } = useBoardStore();

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
        
        const [projectsData, boardsData] = await Promise.all([
          getProjects(),
          getBoards(),
        ]);
        setProjects(projectsData);
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
  const { loadAllSpecs, selectSpec, currentSpec } = useSpecStore();

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
