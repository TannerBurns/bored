import { useState } from 'react';
import { usePlannerStore } from '../../stores/plannerStore';
import { Button } from '../common/Button';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { PlanViewer } from './PlanViewer';
import type { Scratchpad, Exploration } from '../../types';

interface ScratchpadDetailProps {
  scratchpad: Scratchpad;
  onClose: () => void;
}

export function ScratchpadDetail({ scratchpad, onClose }: ScratchpadDetailProps) {
  const { approvePlan, deleteScratchpad } = usePlannerStore();
  const [activeTab, setActiveTab] = useState<'input' | 'exploration' | 'plan'>('input');
  const [isDeleting, setIsDeleting] = useState(false);

  const handleApprove = async () => {
    try {
      await approvePlan(scratchpad.id);
    } catch (error) {
      console.error('Failed to approve plan:', error);
    }
  };

  const handleDelete = async () => {
    if (!confirm('Are you sure you want to delete this scratchpad?')) return;
    
    setIsDeleting(true);
    try {
      await deleteScratchpad(scratchpad.id);
      onClose();
    } catch (error) {
      console.error('Failed to delete scratchpad:', error);
    } finally {
      setIsDeleting(false);
    }
  };

  const canApprove = scratchpad.status === 'awaiting_approval' && scratchpad.planMarkdown;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b dark:border-gray-700">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            {scratchpad.name}
          </h2>
          <p className="text-sm text-gray-500 capitalize">
            Status: {scratchpad.status.replace('_', ' ')}
          </p>
        </div>
        <div className="flex gap-2">
          {canApprove && (
            <Button onClick={handleApprove} variant="primary">
              Approve Plan
            </Button>
          )}
          <Button 
            onClick={handleDelete} 
            variant="secondary" 
            disabled={isDeleting}
            className="text-red-500 hover:text-red-600 border-red-300 hover:border-red-400"
          >
            {isDeleting ? 'Deleting...' : 'Delete'}
          </Button>
          <Button onClick={onClose} variant="secondary">
            Close
          </Button>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex border-b dark:border-gray-700">
        <button
          onClick={() => setActiveTab('input')}
          className={`px-4 py-2 text-sm font-medium ${
            activeTab === 'input'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          User Input
        </button>
        <button
          onClick={() => setActiveTab('exploration')}
          className={`px-4 py-2 text-sm font-medium ${
            activeTab === 'exploration'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          Exploration ({scratchpad.explorationLog?.length || 0})
        </button>
        <button
          onClick={() => setActiveTab('plan')}
          className={`px-4 py-2 text-sm font-medium ${
            activeTab === 'plan'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          Plan
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === 'input' && (
          <div className="prose dark:prose-invert max-w-none">
            <h3>Original Request</h3>
            <p className="whitespace-pre-wrap">{scratchpad.userInput}</p>
          </div>
        )}

        {activeTab === 'exploration' && (
          <ExplorationLog explorations={scratchpad.explorationLog || []} />
        )}

        {activeTab === 'plan' && (
          scratchpad.planMarkdown ? (
            <PlanViewer
              markdown={scratchpad.planMarkdown}
              planJson={scratchpad.planJson}
            />
          ) : (
            <div className="text-gray-500 text-center py-8">
              No plan generated yet
            </div>
          )
        )}
      </div>
    </div>
  );
}

function ExplorationLog({ explorations }: { explorations: Exploration[] }) {
  if (explorations.length === 0) {
    return (
      <div className="text-gray-500 text-center py-8">
        No explorations yet
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {explorations.map((exploration, idx) => (
        <div key={idx} className="border dark:border-gray-700 rounded-lg overflow-hidden">
          <div className="bg-gray-100 dark:bg-gray-800 px-4 py-2">
            <h4 className="font-medium text-gray-900 dark:text-white">
              Query {idx + 1}
            </h4>
            <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
              {exploration.query}
            </p>
          </div>
          <div className="p-4">
            <MarkdownViewer content={exploration.response} />
          </div>
        </div>
      ))}
    </div>
  );
}
