import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BuildWithDropdown } from './BuildWithDropdown';
import type { AgentInfo } from '../../types';

const mockLoadAgents = vi.fn().mockResolvedValue([]);

const MOCK_AGENTS: AgentInfo[] = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null, availableModels: [] },
  { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0', brandColor: '#da7756', availableModels: [] },
];

let storeAgents: AgentInfo[] = MOCK_AGENTS;

vi.mock('../../stores/agentRegistryStore', () => ({
  useAgentRegistryStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector({
      agents: storeAgents,
      agentsLoading: false,
      agentsLoaded: true,
      loadAgents: mockLoadAgents,
    }),
}));

describe('BuildWithDropdown', () => {
  const mockOnSelect = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    storeAgents = MOCK_AGENTS;
  });

  it('renders Build with button', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);
    expect(screen.getByText('Build with')).toBeInTheDocument();
  });

  it('opens dropdown on click', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);

    const button = screen.getByText('Build with');
    fireEvent.click(button);
    
    expect(screen.getByText('Cursor')).toBeInTheDocument();
    expect(screen.getByText('Claude')).toBeInTheDocument();
  });

  it('calls onSelect with cursor when Cursor is clicked', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);

    fireEvent.click(screen.getByText('Build with'));
    fireEvent.click(screen.getByText('Cursor'));
    
    expect(mockOnSelect).toHaveBeenCalledWith('cursor');
    expect(mockOnSelect).toHaveBeenCalledTimes(1);
  });

  it('calls onSelect with claude when Claude is clicked', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);

    fireEvent.click(screen.getByText('Build with'));
    fireEvent.click(screen.getByText('Claude'));
    
    expect(mockOnSelect).toHaveBeenCalledWith('claude');
    expect(mockOnSelect).toHaveBeenCalledTimes(1);
  });

  it('closes dropdown after selection', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);

    fireEvent.click(screen.getByText('Build with'));
    expect(screen.getByText('Cursor')).toBeInTheDocument();
    
    fireEvent.click(screen.getByText('Cursor'));
    
    expect(screen.queryByRole('button', { name: 'Cursor' })).not.toBeInTheDocument();
  });

  it('does not open dropdown when disabled', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} disabled />);
    
    const button = screen.getByText('Build with');
    fireEvent.click(button);
    
    expect(screen.queryByText('Cursor')).not.toBeInTheDocument();
  });

  it('shows disabled reason in title when disabled', () => {
    render(
      <BuildWithDropdown 
        onSelect={mockOnSelect} 
        disabled 
        disabledReason="No project assigned" 
      />
    );
    
    const button = screen.getByText('Build with').closest('button');
    expect(button).toHaveAttribute('title', 'No project assigned');
  });

  it('closes dropdown on escape key', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);

    fireEvent.click(screen.getByText('Build with'));
    expect(screen.getByText('Cursor')).toBeInTheDocument();
    
    fireEvent.keyDown(document, { key: 'Escape' });
    
    expect(screen.queryByText('Cursor')).not.toBeInTheDocument();
  });

  it('closes dropdown on outside click', () => {
    render(
      <div>
        <BuildWithDropdown onSelect={mockOnSelect} />
        <button data-testid="outside">Outside</button>
      </div>
    );

    fireEvent.click(screen.getByText('Build with'));
    expect(screen.getByText('Cursor')).toBeInTheDocument();
    
    fireEvent.mouseDown(screen.getByTestId('outside'));
    
    expect(screen.queryByText('Cursor')).not.toBeInTheDocument();
  });

  it('toggles dropdown open and closed', () => {
    render(<BuildWithDropdown onSelect={mockOnSelect} />);

    const button = screen.getByText('Build with');
    
    fireEvent.click(button);
    expect(screen.getByText('Cursor')).toBeInTheDocument();
    
    fireEvent.click(button);
    expect(screen.queryByText('Cursor')).not.toBeInTheDocument();
  });

  describe('CLI availability', () => {
    it('does not call onSelect when Cursor CLI is unavailable', () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0', brandColor: '#da7756', availableModels: [] },
      ];

      render(<BuildWithDropdown onSelect={mockOnSelect} />);

      fireEvent.click(screen.getByText('Build with'));

      const cursorButton = screen.getByText('Cursor').closest('button');
      expect(cursorButton).toBeDisabled();

      fireEvent.click(cursorButton!);
      expect(mockOnSelect).not.toHaveBeenCalled();
    });

    it('does not call onSelect when Claude CLI is unavailable', () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<BuildWithDropdown onSelect={mockOnSelect} />);

      fireEvent.click(screen.getByText('Build with'));

      const claudeButton = screen.getByText('Claude').closest('button');
      expect(claudeButton).toBeDisabled();

      fireEvent.click(claudeButton!);
      expect(mockOnSelect).not.toHaveBeenCalled();
    });

    it('shows "(not installed)" text when an agent is unavailable', () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0', brandColor: '#da7756', availableModels: [] },
      ];

      render(<BuildWithDropdown onSelect={mockOnSelect} />);

      fireEvent.click(screen.getByText('Build with'));

      expect(screen.getByText('(not installed)')).toBeInTheDocument();
    });

    it('shows both as unavailable when both CLIs are not installed', () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<BuildWithDropdown onSelect={mockOnSelect} />);

      fireEvent.click(screen.getByText('Build with'));

      const notInstalledLabels = screen.getAllByText('(not installed)');
      expect(notInstalledLabels).toHaveLength(2);

      const cursorButton = screen.getByText('Cursor').closest('button');
      const claudeButton = screen.getByText('Claude').closest('button');
      expect(cursorButton).toBeDisabled();
      expect(claudeButton).toBeDisabled();
    });

    it('allows Cursor selection when available but Claude is not', () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756', availableModels: [] },
      ];

      render(<BuildWithDropdown onSelect={mockOnSelect} />);

      fireEvent.click(screen.getByText('Build with'));

      const cursorButton = screen.getByText('Cursor').closest('button');
      expect(cursorButton).not.toBeDisabled();

      fireEvent.click(screen.getByText('Cursor'));
      expect(mockOnSelect).toHaveBeenCalledWith('cursor');
    });

    it('allows Claude selection when available but Cursor is not', () => {
      storeAgents = [
        { id: 'cursor', displayName: 'Cursor', isAvailable: false, version: null, brandColor: null, availableModels: [] },
        { id: 'claude', displayName: 'Claude', isAvailable: true, version: '1.0', brandColor: '#da7756', availableModels: [] },
      ];

      render(<BuildWithDropdown onSelect={mockOnSelect} />);

      fireEvent.click(screen.getByText('Build with'));

      const claudeButton = screen.getByText('Claude').closest('button');
      expect(claudeButton).not.toBeDisabled();

      fireEvent.click(screen.getByText('Claude'));
      expect(mockOnSelect).toHaveBeenCalledWith('claude');
    });
  });

  describe('overflow-aware dropdown direction', () => {
    const origGetComputedStyle = window.getComputedStyle;

    afterEach(() => {
      window.getComputedStyle = origGetComputedStyle;
    });

    function makeDOMRect(overrides: Partial<DOMRect>): DOMRect {
      return {
        top: 0, bottom: 0, left: 0, right: 0,
        width: 0, height: 0, x: 0, y: 0,
        toJSON: () => ({}),
        ...overrides,
      } as DOMRect;
    }

    function mockClippingAncestor(
      element: HTMLElement,
      prop: 'overflow' | 'overflowY',
    ) {
      const orig = origGetComputedStyle;
      window.getComputedStyle = ((elt: Element, pseudo?: string | null): CSSStyleDeclaration => {
        const real = orig.call(window, elt, pseudo);
        if (elt === element) {
          return new Proxy(real, {
            get(target, p, receiver) {
              if (p === prop) return 'hidden';
              return Reflect.get(target, p, receiver);
            },
          }) as CSSStyleDeclaration;
        }
        return real;
      }) as typeof window.getComputedStyle;
    }

    it('opens upward when overflow-hidden ancestor constrains space below', () => {
      const { container } = render(
        <div data-testid="clipping-ancestor">
          <BuildWithDropdown onSelect={mockOnSelect} />
        </div>
      );

      const ancestor = screen.getByTestId('clipping-ancestor');
      const button = screen.getByText('Build with').closest('button')!;

      vi.spyOn(button, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 400, bottom: 440 })
      );
      vi.spyOn(ancestor, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 50, bottom: 470 })
      );
      mockClippingAncestor(ancestor, 'overflow');

      fireEvent.click(button);

      expect(screen.getByText('Cursor')).toBeInTheDocument();
      expect(container.querySelector('.bottom-full')).not.toBeNull();
      expect(container.querySelector('.top-full')).toBeNull();
    });

    it('opens downward when overflow-hidden ancestor has enough space below', () => {
      const { container } = render(
        <div data-testid="clipping-ancestor">
          <BuildWithDropdown onSelect={mockOnSelect} />
        </div>
      );

      const ancestor = screen.getByTestId('clipping-ancestor');
      const button = screen.getByText('Build with').closest('button')!;

      vi.spyOn(button, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 200, bottom: 240 })
      );
      vi.spyOn(ancestor, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 50, bottom: 500 })
      );
      mockClippingAncestor(ancestor, 'overflow');

      fireEvent.click(button);

      expect(screen.getByText('Cursor')).toBeInTheDocument();
      expect(container.querySelector('.top-full')).not.toBeNull();
      expect(container.querySelector('.bottom-full')).toBeNull();
    });

    it('detects overflowY: hidden on ancestor', () => {
      const { container } = render(
        <div data-testid="clipping-ancestor">
          <BuildWithDropdown onSelect={mockOnSelect} />
        </div>
      );

      const ancestor = screen.getByTestId('clipping-ancestor');
      const button = screen.getByText('Build with').closest('button')!;

      vi.spyOn(button, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 400, bottom: 440 })
      );
      vi.spyOn(ancestor, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 50, bottom: 470 })
      );
      mockClippingAncestor(ancestor, 'overflowY');

      fireEvent.click(button);

      expect(screen.getByText('Cursor')).toBeInTheDocument();
      expect(container.querySelector('.bottom-full')).not.toBeNull();
    });

    it('falls back to viewport when no clipping ancestor exists', () => {
      const { container } = render(
        <BuildWithDropdown onSelect={mockOnSelect} />
      );

      const button = screen.getByText('Build with').closest('button')!;

      // Place button well above the viewport bottom (JSDOM innerHeight=768)
      vi.spyOn(button, 'getBoundingClientRect').mockReturnValue(
        makeDOMRect({ top: 200, bottom: 240 })
      );

      fireEvent.click(button);

      expect(screen.getByText('Cursor')).toBeInTheDocument();
      expect(container.querySelector('.top-full')).not.toBeNull();
      expect(container.querySelector('.bottom-full')).toBeNull();
    });
  });
});
