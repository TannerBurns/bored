import { useState } from 'react';
import { Modal } from '../common/Modal';
import { Button } from '../common/Button';
import { Input } from '../common/Input';
import { usePlannerStore } from '../../stores/plannerStore';

interface CreateScratchpadModalProps {
  boardId: string;
  /** Required - the project this scratchpad is scoped to */
  projectId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

type AgentPref = 'cursor' | 'claude' | 'any';

export function CreateScratchpadModal({
  boardId,
  projectId,
  open,
  onOpenChange,
}: CreateScratchpadModalProps) {
  const { createScratchpad, isLoading } = usePlannerStore();
  const [name, setName] = useState('');
  const [userInput, setUserInput] = useState('');
  const [agentPref, setAgentPref] = useState<AgentPref>('any');
  const [model, setModel] = useState('');
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!name.trim()) {
      setError('Name is required');
      return;
    }

    if (!userInput.trim()) {
      setError('Please describe what you want to build');
      return;
    }

    try {
      await createScratchpad({
        boardId,
        projectId,
        name: name.trim(),
        userInput: userInput.trim(),
        agentPref: agentPref !== 'any' ? agentPref : undefined,
        model: model.trim() || undefined,
      });
      
      setName('');
      setUserInput('');
      setAgentPref('any');
      setModel('');
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <Modal open={open} onOpenChange={onOpenChange} title="New Scratchpad">
      <form onSubmit={handleSubmit} className="space-y-4">
        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            Name
          </label>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="e.g., User Authentication Feature"
            autoFocus
          />
        </div>

        <div>
          <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
            What do you want to build?
          </label>
          <textarea
            value={userInput}
            onChange={(e) => setUserInput(e.target.value)}
            placeholder="Describe the feature or functionality you want to implement. Be as detailed as possible - include requirements, constraints, and any specific implementation preferences."
            className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg 
                     bg-white dark:bg-gray-800 text-gray-900 dark:text-white
                     focus:ring-2 focus:ring-blue-500 focus:border-blue-500
                     min-h-[200px] resize-y"
          />
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Agent Preference
            </label>
            <select
              value={agentPref}
              onChange={(e) => setAgentPref(e.target.value as AgentPref)}
              className="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-lg 
                       bg-white dark:bg-gray-800 text-gray-900 dark:text-white
                       focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
            >
              <option value="any">Any Agent</option>
              <option value="cursor">Cursor</option>
              <option value="claude">Claude Code</option>
            </select>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-1">
              Model (optional)
            </label>
            <Input
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="e.g., opus, sonnet"
            />
          </div>
        </div>

        {error && (
          <div className="text-red-500 text-sm">{error}</div>
        )}

        <div className="flex justify-end gap-3 pt-4">
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" disabled={isLoading}>
            {isLoading ? 'Creating...' : 'Create Scratchpad'}
          </Button>
        </div>
      </form>
    </Modal>
  );
}
