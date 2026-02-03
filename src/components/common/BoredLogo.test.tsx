import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { BoredLogo } from './BoredLogo';

describe('BoredLogo', () => {
  describe('default variant', () => {
    it('renders with default size', () => {
      const { container } = render(<BoredLogo />);
      const svg = container.querySelector('svg');
      
      expect(svg).toBeInTheDocument();
      expect(svg).toHaveAttribute('width', '48');
      expect(svg).toHaveAttribute('height', '48');
    });

    it('renders with custom size', () => {
      const { container } = render(<BoredLogo size={72} />);
      const svg = container.querySelector('svg');
      
      expect(svg).toHaveAttribute('width', '72');
      expect(svg).toHaveAttribute('height', '72');
    });

    it('applies className', () => {
      const { container } = render(<BoredLogo className="custom-class" />);
      const svg = container.querySelector('svg');
      
      expect(svg).toHaveClass('custom-class');
    });

    it('has correct viewBox', () => {
      const { container } = render(<BoredLogo />);
      const svg = container.querySelector('svg');
      
      expect(svg).toHaveAttribute('viewBox', '0 0 512 512');
    });

    it('has aria-label for accessibility', () => {
      const { container } = render(<BoredLogo />);
      const svg = container.querySelector('svg');
      
      expect(svg).toHaveAttribute('aria-label', 'Bored logo');
    });

    it('renders white background with black B', () => {
      const { container } = render(<BoredLogo />);
      const rect = container.querySelector('rect');
      const path = container.querySelector('path');
      
      expect(rect).toHaveAttribute('fill', 'white');
      expect(path).toHaveAttribute('fill', 'black');
    });

    it('does not include gradient defs', () => {
      const { container } = render(<BoredLogo />);
      const defs = container.querySelector('defs');
      
      expect(defs).not.toBeInTheDocument();
    });
  });

  describe('gradient variant', () => {
    it('renders gradient variant', () => {
      const { container } = render(<BoredLogo variant="gradient" />);
      const svg = container.querySelector('svg');
      const defs = container.querySelector('defs');
      
      expect(svg).toBeInTheDocument();
      expect(defs).toBeInTheDocument();
    });

    it('uses gradient fill for background', () => {
      const { container } = render(<BoredLogo variant="gradient" />);
      const rect = container.querySelector('rect');
      
      expect(rect).toHaveAttribute('fill', 'url(#bored-logo-gradient)');
    });

    it('renders white B on gradient background', () => {
      const { container } = render(<BoredLogo variant="gradient" />);
      const path = container.querySelector('path');
      
      expect(path).toHaveAttribute('fill', 'white');
    });

    it('uses custom gradientId', () => {
      const { container } = render(<BoredLogo variant="gradient" gradientId="custom-gradient" />);
      const rect = container.querySelector('rect');
      const gradient = container.querySelector('#custom-gradient');
      
      expect(rect).toHaveAttribute('fill', 'url(#custom-gradient)');
      expect(gradient).toBeInTheDocument();
    });

    it('applies size and className to gradient variant', () => {
      const { container } = render(<BoredLogo variant="gradient" size={28} className="sidebar-logo" />);
      const svg = container.querySelector('svg');
      
      expect(svg).toHaveAttribute('width', '28');
      expect(svg).toHaveAttribute('height', '28');
      expect(svg).toHaveClass('sidebar-logo');
    });
  });
});
