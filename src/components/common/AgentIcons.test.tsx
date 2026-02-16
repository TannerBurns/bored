import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { ClaudeIcon, CursorIcon, AgentFallbackIcon, getAgentIcon, getAgentDisplayName, getAgentBrandColor } from './AgentIcons';

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

describe('getAgentDisplayName', () => {
  it('returns Cursor for cursor', () => {
    expect(getAgentDisplayName('cursor')).toBe('Cursor');
  });

  it('returns Claude for claude', () => {
    expect(getAgentDisplayName('claude')).toBe('Claude');
  });

  it('capitalizes unknown agent ID', () => {
    expect(getAgentDisplayName('openai')).toBe('Openai');
  });

  it('capitalizes single letter', () => {
    expect(getAgentDisplayName('x')).toBe('X');
  });

  it('handles empty string', () => {
    expect(getAgentDisplayName('')).toBe('');
  });
});

describe('getAgentBrandColor', () => {
  it('returns Claude brand color', () => {
    expect(getAgentBrandColor('claude')).toBe('#da7756');
  });

  it('returns undefined for unknown agent', () => {
    expect(getAgentBrandColor('unknown')).toBeUndefined();
  });

  it('uses provided brandColor over default', () => {
    expect(getAgentBrandColor('claude', '#ffffff')).toBe('#ffffff');
  });

  it('ignores empty string brandColor and falls back to default', () => {
    expect(getAgentBrandColor('claude', '')).toBe('#da7756');
  });

  it('ignores null brandColor and falls back to default', () => {
    expect(getAgentBrandColor('claude', null)).toBe('#da7756');
  });

  it('returns undefined for unknown agent with null brandColor', () => {
    expect(getAgentBrandColor('windsurf', null)).toBeUndefined();
  });
});

describe('getAgentDisplayName with override', () => {
  it('uses displayName when provided', () => {
    expect(getAgentDisplayName('claude', 'Claude Code')).toBe('Claude Code');
  });

  it('ignores empty displayName and falls back', () => {
    expect(getAgentDisplayName('claude', '')).toBe('Claude');
  });

  it('falls back to capitalized ID for unknown agent without displayName', () => {
    expect(getAgentDisplayName('windsurf')).toBe('Windsurf');
  });
});

describe('icon style prop', () => {
  it('ClaudeIcon passes style to svg', () => {
    const { container } = render(<ClaudeIcon style={{ color: 'red' }} />);
    const svg = container.querySelector('svg');
    expect(svg?.style.color).toBe('red');
  });

  it('CursorIcon passes style to svg', () => {
    const { container } = render(<CursorIcon style={{ color: 'blue' }} />);
    const svg = container.querySelector('svg');
    expect(svg?.style.color).toBe('blue');
  });

  it('AgentFallbackIcon passes style to svg', () => {
    const { container } = render(<AgentFallbackIcon style={{ color: 'green' }} />);
    const svg = container.querySelector('svg');
    expect(svg?.style.color).toBe('green');
  });
});
