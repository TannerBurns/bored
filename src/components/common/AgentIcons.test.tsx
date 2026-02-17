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
  it('capitalizes agent ID when no display name provided', () => {
    expect(getAgentDisplayName('cursor')).toBe('Cursor');
    expect(getAgentDisplayName('claude')).toBe('Claude');
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
  it('returns undefined when no brand color provided', () => {
    expect(getAgentBrandColor('claude')).toBeUndefined();
  });

  it('returns undefined for unknown agent', () => {
    expect(getAgentBrandColor('unknown')).toBeUndefined();
  });

  it('uses provided brandColor from backend', () => {
    expect(getAgentBrandColor('claude', '#da7756')).toBe('#da7756');
  });

  it('returns provided brandColor for any agent', () => {
    expect(getAgentBrandColor('windsurf', '#00ff00')).toBe('#00ff00');
  });

  it('ignores empty string brandColor', () => {
    expect(getAgentBrandColor('claude', '')).toBeUndefined();
  });

  it('ignores null brandColor', () => {
    expect(getAgentBrandColor('claude', null)).toBeUndefined();
  });
});

describe('getAgentDisplayName with override', () => {
  it('uses displayName when provided', () => {
    expect(getAgentDisplayName('claude', 'Claude Code')).toBe('Claude Code');
  });

  it('ignores empty displayName and falls back to capitalized ID', () => {
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
