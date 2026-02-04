import { useMemo, useCallback, useEffect, useRef } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  Handle,
  Position,
  useNodesState,
  useEdgesState,
  type Node,
  type Edge,
  type NodeProps,
  type ReactFlowInstance,
} from '@xyflow/react';
import dagre from 'dagre';
import type { PlanEpic } from '../../types';
import { normalizeDependencies } from '../../lib/utils';

import '@xyflow/react/dist/style.css';

interface ExecutionGraphProps {
  epics: PlanEpic[];
}

interface EpicNodeData {
  title: string;
  description: string;
  ticketCount: number;
  isRoot: boolean;
  [key: string]: unknown;
}

function EpicNode({ data }: NodeProps<Node<EpicNodeData>>) {
  const { title, description, ticketCount, isRoot } = data;
  
  return (
    <div
      className={`
        glass rounded-lg p-3 min-w-[180px] max-w-[240px] border transition-all duration-200
        hover:shadow-lg cursor-pointer
        ${isRoot 
          ? 'border-board-accent shadow-[0_0_0_1px_rgba(var(--color-board-accent),0.3)]' 
          : 'border-board-border'
        }
      `}
    >
      <Handle 
        type="target" 
        position={Position.Top} 
        className="!bg-board-accent !w-2 !h-2 !border-0"
      />
      <div className="flex items-start justify-between gap-2">
        <div className="font-medium text-board-text text-sm leading-tight">
          {title}
        </div>
        <span className="flex-shrink-0 bg-board-accent/20 text-board-accent text-xs font-medium px-1.5 py-0.5 rounded">
          {ticketCount}
        </span>
      </div>
      {description && (
        <div className="text-xs text-board-text-muted mt-1.5 line-clamp-2">
          {description}
        </div>
      )}
      {isRoot && (
        <div className="text-xs text-status-success mt-2 flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-status-success" />
          Root
        </div>
      )}
      <Handle 
        type="source" 
        position={Position.Bottom} 
        className="!bg-board-accent !w-2 !h-2 !border-0"
      />
    </div>
  );
}

const nodeTypes = {
  epic: EpicNode,
};

function getLayoutedElements(
  nodes: Node<EpicNodeData>[],
  edges: Edge[],
  direction: 'TB' | 'LR' = 'TB'
): { nodes: Node<EpicNodeData>[]; edges: Edge[] } {
  const dagreGraph = new dagre.graphlib.Graph();
  dagreGraph.setDefaultEdgeLabel(() => ({}));
  
  const nodeWidth = 200;
  const nodeHeight = 100;
  
  dagreGraph.setGraph({ 
    rankdir: direction, 
    nodesep: 50, 
    ranksep: 80,
    marginx: 20,
    marginy: 20,
  });

  nodes.forEach((node) => {
    dagreGraph.setNode(node.id, { width: nodeWidth, height: nodeHeight });
  });

  edges.forEach((edge) => {
    dagreGraph.setEdge(edge.source, edge.target);
  });

  dagre.layout(dagreGraph);

  const layoutedNodes = nodes.map((node) => {
    const nodeWithPosition = dagreGraph.node(node.id);
    return {
      ...node,
      position: {
        x: nodeWithPosition.x - nodeWidth / 2,
        y: nodeWithPosition.y - nodeHeight / 2,
      },
    };
  });

  return { nodes: layoutedNodes, edges };
}

function buildGraph(epics: PlanEpic[]): { nodes: Node<EpicNodeData>[]; edges: Edge[] } {
  const titleToId = new Map<string, string>();
  
  const nodes: Node<EpicNodeData>[] = epics.map((epic, index) => {
    const id = `epic-${index}`;
    titleToId.set(epic.title, id);
    const deps = normalizeDependencies(epic.dependsOn);
    
    return {
      id,
      type: 'epic',
      position: { x: 0, y: 0 }, // Will be set by dagre
      data: {
        title: epic.title,
        description: epic.description,
        ticketCount: epic.tickets.length,
        isRoot: deps.length === 0,
      },
    };
  });

  const edges: Edge[] = [];
  epics.forEach((epic, index) => {
    const targetId = `epic-${index}`;
    const deps = normalizeDependencies(epic.dependsOn);
    
    deps.forEach((depTitle) => {
      const sourceId = titleToId.get(depTitle);
      if (sourceId) {
        edges.push({
          id: `edge-${sourceId}-${targetId}`,
          source: sourceId,
          target: targetId,
          type: 'smoothstep',
          animated: false,
          style: {
            stroke: 'var(--color-board-accent)',
            strokeWidth: 2,
          },
        });
      }
    });
  });

  return getLayoutedElements(nodes, edges);
}

export function ExecutionGraph({ epics }: ExecutionGraphProps) {
  const { nodes: layoutedNodes, edges: layoutedEdges } = useMemo(
    () => buildGraph(epics),
    [epics]
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(layoutedNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(layoutedEdges);
  const reactFlowInstance = useRef<ReactFlowInstance<Node<EpicNodeData>, Edge> | null>(null);

  useEffect(() => {
    setNodes(layoutedNodes);
    setEdges(layoutedEdges);
  }, [layoutedNodes, layoutedEdges, setNodes, setEdges]);
  
  // Fit view after state update (separate effect ensures nodes are set first)
  useEffect(() => {
    if (nodes.length > 0 && reactFlowInstance.current) {
      setTimeout(() => {
        reactFlowInstance.current?.fitView({ padding: 0.2, duration: 200 });
      }, 100);
    }
  }, [nodes]);

  const rootCount = useMemo(
    () => epics.filter(e => normalizeDependencies(e.dependsOn).length === 0).length,
    [epics]
  );

  const phaseCount = useMemo(() => {
    const titleToEpic = new Map<string, PlanEpic>();
    epics.forEach(e => titleToEpic.set(e.title, e));
    
    const levels = new Map<string, number>();
    
    function getLevel(epic: PlanEpic): number {
      if (levels.has(epic.title)) return levels.get(epic.title)!;
      
      const deps = normalizeDependencies(epic.dependsOn);
      if (deps.length === 0) {
        levels.set(epic.title, 0);
        return 0;
      }
      
      let maxDepLevel = 0;
      for (const depTitle of deps) {
        const depEpic = titleToEpic.get(depTitle);
        if (depEpic) {
          maxDepLevel = Math.max(maxDepLevel, getLevel(depEpic) + 1);
        }
      }
      levels.set(epic.title, maxDepLevel);
      return maxDepLevel;
    }

    epics.forEach(e => getLevel(e));
    
    const maxLevel = Math.max(...Array.from(levels.values()), 0);
    return maxLevel + 1;
  }, [epics]);

  const onInit = useCallback((instance: ReactFlowInstance<Node<EpicNodeData>, Edge>) => {
    reactFlowInstance.current = instance;
    setTimeout(() => instance.fitView({ padding: 0.2 }), 150);
  }, []);

  return (
    <div className="space-y-4">
      {/* Summary */}
      <div className="text-sm">
        {rootCount === 1 ? (
          <span className="text-status-success flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-status-success" />
            Sequential execution: 1 root epic, {phaseCount} phases total
          </span>
        ) : rootCount === epics.length ? (
          <span className="text-status-warning flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-status-warning" />
            All {rootCount} epics are root (no dependencies) - all can run in parallel
          </span>
        ) : (
          <span className="text-board-text-secondary flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-status-info" />
            {rootCount} root epic{rootCount !== 1 ? 's' : ''} (can start immediately), {phaseCount} phases total
          </span>
        )}
      </div>

      {/* Graph */}
      <div className="h-[400px] rounded-lg overflow-hidden border border-board-border glass-subtle">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onInit={onInit}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{
            padding: 0.2,
            minZoom: 0.5,
            maxZoom: 1.5,
          }}
          minZoom={0.25}
          maxZoom={2}
          proOptions={{ hideAttribution: true }}
          className="bg-transparent"
        >
          <Background 
            color="var(--color-board-border)" 
            gap={20} 
            size={1}
          />
          <Controls 
            className="!bg-board-bg !border-board-border !shadow-lg [&>button]:!bg-board-bg [&>button]:!border-board-border [&>button]:!text-board-text [&>button:hover]:!bg-board-hover"
            showInteractive={false}
          />
        </ReactFlow>
      </div>
    </div>
  );
}
