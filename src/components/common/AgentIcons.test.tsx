import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { ClaudeIcon, CursorIcon, CLAUDE_BRAND_COLOR } from './AgentIcons';

describe('ClaudeIcon', () => {
  it('renders with default size', () => {
    const { container } = render(<ClaudeIcon />);
    const svg = container.querySelector('svg');
    
    expect(svg).toBeInTheDocument();
    expect(svg).toHaveAttribute('width', '16');
    expect(svg).toHaveAttribute('height', '16');
  });

  it('renders with custom size', () => {
    const { container } = render(<ClaudeIcon size={24} />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveAttribute('width', '24');
    expect(svg).toHaveAttribute('height', '24');
  });

  it('applies className', () => {
    const { container } = render(<ClaudeIcon className="text-red-500" />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveClass('text-red-500');
  });

  it('has correct viewBox', () => {
    const { container } = render(<ClaudeIcon />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveAttribute('viewBox', '0 0 16 16');
  });
});

describe('CursorIcon', () => {
  it('renders with default size', () => {
    const { container } = render(<CursorIcon />);
    const svg = container.querySelector('svg');
    
    expect(svg).toBeInTheDocument();
    expect(svg).toHaveAttribute('width', '16');
    expect(svg).toHaveAttribute('height', '16');
  });

  it('renders with custom size', () => {
    const { container } = render(<CursorIcon size={32} />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveAttribute('width', '32');
    expect(svg).toHaveAttribute('height', '32');
  });

  it('applies className', () => {
    const { container } = render(<CursorIcon className="text-purple-500" />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveClass('text-purple-500');
  });

  it('has correct viewBox', () => {
    const { container } = render(<CursorIcon />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveAttribute('viewBox', '0 0 16 16');
  });
});

describe('CLAUDE_BRAND_COLOR', () => {
  it('exports correct brand color', () => {
    expect(CLAUDE_BRAND_COLOR).toBe('#da7756');
  });
});
