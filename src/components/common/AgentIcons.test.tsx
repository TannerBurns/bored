import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { ClaudeIcon, CursorIcon, AgentFallbackIcon, getAgentIcon, CLAUDE_BRAND_COLOR } from './AgentIcons';

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

describe('AgentFallbackIcon', () => {
  it('renders with default size', () => {
    const { container } = render(<AgentFallbackIcon />);
    const svg = container.querySelector('svg');
    
    expect(svg).toBeInTheDocument();
    expect(svg).toHaveAttribute('width', '16');
    expect(svg).toHaveAttribute('height', '16');
  });

  it('renders with custom size', () => {
    const { container } = render(<AgentFallbackIcon size={20} />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveAttribute('width', '20');
  });

  it('applies className', () => {
    const { container } = render(<AgentFallbackIcon className="text-gray-400" />);
    const svg = container.querySelector('svg');
    
    expect(svg).toHaveClass('text-gray-400');
  });
});

describe('getAgentIcon', () => {
  it('returns ClaudeIcon for claude', () => {
    expect(getAgentIcon('claude')).toBe(ClaudeIcon);
  });

  it('returns CursorIcon for cursor', () => {
    expect(getAgentIcon('cursor')).toBe(CursorIcon);
  });

  it('returns AgentFallbackIcon for unknown agent', () => {
    expect(getAgentIcon('unknown-agent')).toBe(AgentFallbackIcon);
  });

  it('returns AgentFallbackIcon for empty string', () => {
    expect(getAgentIcon('')).toBe(AgentFallbackIcon);
  });

  it('returned components render correctly', () => {
    const Icon = getAgentIcon('claude');
    const { container } = render(<Icon size={24} />);
    const svg = container.querySelector('svg');
    expect(svg).toBeInTheDocument();
    expect(svg).toHaveAttribute('width', '24');
  });
});

describe('CLAUDE_BRAND_COLOR', () => {
  it('exports correct brand color', () => {
    expect(CLAUDE_BRAND_COLOR).toBe('#da7756');
  });
});
