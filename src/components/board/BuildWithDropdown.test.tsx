import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BuildWithDropdown } from './BuildWithDropdown';

describe('BuildWithDropdown', () => {
  const mockOnSelect = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
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
});
