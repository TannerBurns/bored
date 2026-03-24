import { useState, useEffect, useMemo } from 'react';
import { Modal } from '../common/Modal';
import { Button } from '../common/Button';
import { useChatStore } from '../../stores/chatStore';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { getBoards, getTickets, getColumns } from '../../lib/tauri';
import { getAgentIcon, getAgentBrandColor } from '../common/AgentIcons';
import { ScopeSelector } from '../common/ScopeSelector';
import type { ScopeType } from '../common/ScopeSelector';
import { cn } from '../../lib/utils';
import type { ChatMode, Board, Ticket, Column } from '../../types';

interface NewChatModalProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialMode?: ChatMode | null;
}

interface ModeOption {
  mode: ChatMode;
  label: string;
  description: string;
  color: string;
}

const MODE_OPTIONS: ModeOption[] = [
  {
    mode: 'general',
    label: 'General',
    description: 'Ask questions about code or run agent commands',
    color: 'ring-blue-500/50 bg-blue-500/5',
  },
  {
    mode: 'spec_builder',
    label: 'Spec Builder',
    description: 'Create specs and implementation plans',
    color: 'ring-purple-500/50 bg-purple-500/5',
  },
  {
    mode: 'ticket_builder',
    label: 'Ticket Builder',
    description: 'Generate tickets with tasks from conversation',
    color: 'ring-green-500/50 bg-green-500/5',
  },
  {
    mode: 'review',
    label: 'Review',
    description: 'Review completed work, run the app, create fix tasks',
    color: 'ring-orange-500/50 bg-orange-500/5',
  },
];

export function NewChatModal({ open, onOpenChange, initialMode }: NewChatModalProps) {
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [selectedMode, setSelectedMode] = useState<ChatMode | null>(null);
  const [selectedAgent, setSelectedAgent] = useState('');
  const [selectedScope, setSelectedScope] = useState<{ type: ScopeType; id: string } | null>(null);
  const [selectedBoard, setSelectedBoard] = useState('');
  const [selectedTicket, setSelectedTicket] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);

  const [boards, setBoards] = useState<Board[]>([]);
  const [tickets, setTickets] = useState<Ticket[]>([]);

  const { agents, loadAgents } = useAgentRegistryStore();
  const { createChat, selectChat } = useChatStore();

  useEffect(() => {
    if (open) {
      setSelectedAgent('');
      setSelectedScope(null);
      setSelectedBoard('');
      setSelectedTicket('');
      loadAgents();
      getBoards().then(setBoards).catch(() => {});

      if (initialMode) {
        setSelectedMode(initialMode);
        setStep(2);
      } else {
        setSelectedMode(null);
        setStep(1);
      }
    }
  }, [open, loadAgents, initialMode]);

  // Load tickets when board changes (for review mode)
  useEffect(() => {
    if (selectedBoard && selectedMode === 'review') {
      Promise.all([
        getTickets(selectedBoard),
        getColumns(selectedBoard),
      ]).then(([allTickets, cols]) => {
        const reviewDoneCols = cols.filter(
          (c: Column) => c.name === 'Review' || c.name === 'Done'
        );
        const colIds = new Set(reviewDoneCols.map((c: Column) => c.id));
        const filtered = allTickets.filter((t: Ticket) => colIds.has(t.columnId));
        setTickets(filtered);
      }).catch(() => setTickets([]));
    } else {
      setTickets([]);
    }
  }, [selectedBoard, selectedMode]);

  const needsStep3 = selectedMode === 'spec_builder' || selectedMode === 'ticket_builder' || selectedMode === 'review';

  const handleModeSelect = (mode: ChatMode) => {
    setSelectedMode(mode);
    setStep(2);
  };

  const handleStep2Next = () => {
    if (!selectedAgent || !selectedScope) return;
    if (needsStep3) {
      setStep(3);
    } else {
      handleSubmit();
    }
  };

  const handleSubmit = async () => {
    if (!selectedMode || !selectedAgent || !selectedScope) return;
    setIsSubmitting(true);
    try {
      const chat = await createChat({
        agentType: selectedAgent,
        projectId: selectedScope.type === 'project' ? selectedScope.id : undefined,
        workspaceId: selectedScope.type === 'workspace' ? selectedScope.id : undefined,
        mode: selectedMode,
        boardId: selectedBoard || undefined,
        ticketId: selectedTicket || undefined,
      });
      await selectChat(chat.id);
      onOpenChange(false);
    } catch (e) {
      console.error('Failed to create chat:', e);
    } finally {
      setIsSubmitting(false);
    }
  };

  const availableAgents = useMemo(
    () => agents.filter((a) => a.isAvailable).sort((a, b) => a.displayName.localeCompare(b.displayName)),
    [agents],
  );

  const stepTitle =
    step === 1 ? 'Select Mode' : step === 2 ? 'Select Agent & Project' : 'Additional Options';

  return (
    <Modal open={open} onOpenChange={onOpenChange} title="New Chat" description={stepTitle} size="lg">
      {step === 1 && (
        <div className="grid grid-cols-2 gap-3">
          {MODE_OPTIONS.map((opt) => (
            <button
              key={opt.mode}
              onClick={() => handleModeSelect(opt.mode)}
              className={`text-left p-4 rounded-xl border border-board-border transition-all hover:ring-2 ${opt.color}`}
            >
              <div className="font-medium text-sm text-board-text">{opt.label}</div>
              <div className="text-xs text-board-text-muted mt-1">{opt.description}</div>
            </button>
          ))}
        </div>
      )}

      {step === 2 && (
        <div className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-board-text-muted mb-1.5">Agent</label>
            <div className="flex gap-2">
              {availableAgents.map((agent) => {
                const Icon = getAgentIcon(agent.id);
                const brandColor = getAgentBrandColor(agent.id, agent.brandColor);
                const isSelected = selectedAgent === agent.id;
                return (
                  <button
                    key={agent.id}
                    type="button"
                    onClick={() => setSelectedAgent(agent.id)}
                    className={cn(
                      'flex items-center gap-2 px-3 py-2 rounded-lg border text-sm transition-colors',
                      isSelected
                        ? 'border-board-accent bg-board-accent/10 text-board-accent'
                        : 'border-board-border bg-board-surface-raised text-board-text hover:border-board-border/80'
                    )}
                  >
                    <Icon
                      size={16}
                      style={brandColor ? { color: brandColor } : undefined}
                      className={!brandColor ? 'text-board-text-secondary' : undefined}
                    />
                    {agent.displayName}
                  </button>
                );
              })}
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-board-text-muted mb-1.5">
              {selectedMode === 'spec_builder' ? 'Project' : 'Scope'}
            </label>
            <ScopeSelector
              value={selectedScope}
              onChange={setSelectedScope}
              allowEmpty={false}
              emptyLabel="Select..."
              projectsOnly={selectedMode === 'spec_builder'}
            />
          </div>

          <div className="flex items-center justify-between pt-2">
            <Button variant="ghost" size="sm" onClick={() => setStep(1)}>
              Back
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={handleStep2Next}
              disabled={!selectedAgent || !selectedScope}
            >
              {needsStep3 ? 'Next' : 'Create Chat'}
            </Button>
          </div>
        </div>
      )}

      {step === 3 && (
        <div className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-board-text-muted mb-1.5">Board</label>
            <select
              value={selectedBoard}
              onChange={(e) => {
                setSelectedBoard(e.target.value);
                setSelectedTicket('');
              }}
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border"
            >
              <option value="">Select a board...</option>
              {boards.map((board) => (
                <option key={board.id} value={board.id}>
                  {board.name}
                </option>
              ))}
            </select>
          </div>

          {selectedMode === 'review' && (
            <div>
              <label className="block text-xs font-medium text-board-text-muted mb-1.5">Ticket</label>
              <select
                value={selectedTicket}
                onChange={(e) => setSelectedTicket(e.target.value)}
                disabled={!selectedBoard}
                className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text focus:outline-none focus:ring-2 focus:ring-board-accent border border-board-border disabled:opacity-50"
              >
                <option value="">{selectedBoard ? 'Select a ticket...' : 'Select a board first'}</option>
                {tickets.map((ticket) => (
                  <option key={ticket.id} value={ticket.id}>
                    {ticket.title}
                  </option>
                ))}
              </select>
            </div>
          )}

          <div className="flex items-center justify-between pt-2">
            <Button variant="ghost" size="sm" onClick={() => setStep(2)}>
              Back
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={handleSubmit}
              disabled={
                isSubmitting ||
                !selectedBoard ||
                (selectedMode === 'review' && !selectedTicket)
              }
            >
              {isSubmitting ? 'Creating...' : 'Create Chat'}
            </Button>
          </div>
        </div>
      )}
    </Modal>
  );
}
