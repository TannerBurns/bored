import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BuildWithDropdown } from './BuildWithDropdown';

// Mock the useCliAvailability hook
const mockUseCliAvailability = vi.fn();
vi.mock('../../hooks/useCliAvailability', () => ({
  useCliAvailability: () => mockUseCliAvailability(),
}));

describe('BuildWithDropdown', () => {
  const mockOnSelect = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    // Default: both CLIs available
    mockUseCliAvailability.mockReturnValue({
      cursorAvailable: true,
      claudeAvailable: true,
      loading: false,
    });
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
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: true,
        loading: false,
      });

      render(<BuildWithDropdown onSelect={mockOnSelect} />);
      fireEvent.click(screen.getByText('Build with'));

      const cursorButton = screen.getByText('Cursor').closest('button');
      expect(cursorButton).toBeDisabled();

      fireEvent.click(cursorButton!);
      expect(mockOnSelect).not.toHaveBeenCalled();
    });

    it('does not call onSelect when Claude CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: true,
        claudeAvailable: false,
        loading: false,
      });

      render(<BuildWithDropdown onSelect={mockOnSelect} />);
      fireEvent.click(screen.getByText('Build with'));

      const claudeButton = screen.getByText('Claude').closest('button');
      expect(claudeButton).toBeDisabled();

      fireEvent.click(claudeButton!);
      expect(mockOnSelect).not.toHaveBeenCalled();
    });

    it('shows "(not installed)" text when Cursor CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: true,
        loading: false,
      });

      render(<BuildWithDropdown onSelect={mockOnSelect} />);
      fireEvent.click(screen.getByText('Build with'));

      expect(screen.getByText('(not installed)')).toBeInTheDocument();
    });

    it('shows "(not installed)" text when Claude CLI is unavailable', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: true,
        claudeAvailable: false,
        loading: false,
      });

      render(<BuildWithDropdown onSelect={mockOnSelect} />);
      fireEvent.click(screen.getByText('Build with'));

      expect(screen.getByText('(not installed)')).toBeInTheDocument();
    });

    it('shows both as unavailable when both CLIs are not installed', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: false,
        loading: false,
      });

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
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: true,
        claudeAvailable: false,
        loading: false,
      });

      render(<BuildWithDropdown onSelect={mockOnSelect} />);
      fireEvent.click(screen.getByText('Build with'));

      const cursorButton = screen.getByText('Cursor').closest('button');
      expect(cursorButton).not.toBeDisabled();

      fireEvent.click(screen.getByText('Cursor'));
      expect(mockOnSelect).toHaveBeenCalledWith('cursor');
    });

    it('allows Claude selection when available but Cursor is not', () => {
      mockUseCliAvailability.mockReturnValue({
        cursorAvailable: false,
        claudeAvailable: true,
        loading: false,
      });

      render(<BuildWithDropdown onSelect={mockOnSelect} />);
      fireEvent.click(screen.getByText('Build with'));

      const claudeButton = screen.getByText('Claude').closest('button');
      expect(claudeButton).not.toBeDisabled();

      fireEvent.click(screen.getByText('Claude'));
      expect(mockOnSelect).toHaveBeenCalledWith('claude');
    });
  });
});
