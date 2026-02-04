import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StepIndicator } from './StepIndicator';

describe('StepIndicator', () => {
  describe('step circles', () => {
    it('renders correct number of steps', () => {
      const { container } = render(<StepIndicator currentStep={1} totalSteps={3} />);
      const circles = container.querySelectorAll('.rounded-full');
      expect(circles).toHaveLength(3);
    });

    it('shows step numbers for non-completed steps', () => {
      render(<StepIndicator currentStep={2} totalSteps={3} />);
      expect(screen.getByText('2')).toBeInTheDocument();
      expect(screen.getByText('3')).toBeInTheDocument();
    });

    it('shows checkmark for completed steps', () => {
      const { container } = render(<StepIndicator currentStep={3} totalSteps={3} />);
      const checkmarks = container.querySelectorAll('svg polyline');
      expect(checkmarks.length).toBeGreaterThanOrEqual(2);
    });

    it('applies current step styling', () => {
      const { container } = render(<StepIndicator currentStep={2} totalSteps={3} />);
      const circles = container.querySelectorAll('.rounded-full');
      expect(circles[1]).toHaveClass('bg-board-accent');
      expect(circles[1]).toHaveClass('ring-4');
    });

    it('applies completed step styling', () => {
      const { container } = render(<StepIndicator currentStep={3} totalSteps={3} />);
      const circles = container.querySelectorAll('.rounded-full');
      expect(circles[0]).toHaveClass('bg-status-success');
      expect(circles[1]).toHaveClass('bg-status-success');
    });

    it('applies pending step styling', () => {
      const { container } = render(<StepIndicator currentStep={1} totalSteps={3} />);
      const circles = container.querySelectorAll('.rounded-full');
      expect(circles[1]).toHaveClass('bg-board-surface-raised');
      expect(circles[2]).toHaveClass('bg-board-surface-raised');
    });
  });

  describe('step labels', () => {
    it('renders labels when provided', () => {
      render(
        <StepIndicator 
          currentStep={1} 
          totalSteps={3} 
          stepLabels={['First', 'Second', 'Third']} 
        />
      );
      expect(screen.getByText('First')).toBeInTheDocument();
      expect(screen.getByText('Second')).toBeInTheDocument();
      expect(screen.getByText('Third')).toBeInTheDocument();
    });

    it('does not render labels when not provided', () => {
      render(<StepIndicator currentStep={1} totalSteps={3} />);
      expect(screen.queryByText('First')).not.toBeInTheDocument();
    });

    it('applies current label styling', () => {
      render(
        <StepIndicator 
          currentStep={2} 
          totalSteps={3} 
          stepLabels={['First', 'Second', 'Third']} 
        />
      );
      const secondLabel = screen.getByText('Second');
      expect(secondLabel).toHaveClass('text-board-text');
      expect(secondLabel).toHaveClass('font-medium');
    });

    it('applies non-current label styling', () => {
      render(
        <StepIndicator 
          currentStep={2} 
          totalSteps={3} 
          stepLabels={['First', 'Second', 'Third']} 
        />
      );
      const firstLabel = screen.getByText('First');
      expect(firstLabel).toHaveClass('text-board-text-muted');
    });
  });

  describe('connector lines', () => {
    it('renders connector lines between steps', () => {
      const { container } = render(<StepIndicator currentStep={1} totalSteps={3} />);
      const connectors = container.querySelectorAll('.h-0\\.5');
      expect(connectors).toHaveLength(2);
    });

    it('does not render connector after last step', () => {
      const { container } = render(<StepIndicator currentStep={1} totalSteps={2} />);
      const connectors = container.querySelectorAll('.h-0\\.5');
      expect(connectors).toHaveLength(1);
    });

    it('applies completed connector styling', () => {
      const { container } = render(<StepIndicator currentStep={3} totalSteps={3} />);
      const connectors = container.querySelectorAll('.h-0\\.5');
      expect(connectors[0]).toHaveClass('bg-status-success');
      expect(connectors[1]).toHaveClass('bg-status-success');
    });

    it('applies pending connector styling', () => {
      const { container } = render(<StepIndicator currentStep={1} totalSteps={3} />);
      const connectors = container.querySelectorAll('.h-0\\.5');
      expect(connectors[0]).toHaveClass('bg-board-border');
      expect(connectors[1]).toHaveClass('bg-board-border');
    });

    it('applies mixed connector styling for partial progress', () => {
      const { container } = render(<StepIndicator currentStep={2} totalSteps={3} />);
      const connectors = container.querySelectorAll('.h-0\\.5');
      expect(connectors[0]).toHaveClass('bg-status-success');
      expect(connectors[1]).toHaveClass('bg-board-border');
    });
  });

  describe('edge cases', () => {
    it('handles single step', () => {
      const { container } = render(<StepIndicator currentStep={1} totalSteps={1} />);
      const circles = container.querySelectorAll('.rounded-full');
      const connectors = container.querySelectorAll('.h-0\\.5');
      expect(circles).toHaveLength(1);
      expect(connectors).toHaveLength(0);
    });

    it('handles many steps', () => {
      const { container } = render(<StepIndicator currentStep={5} totalSteps={10} />);
      const circles = container.querySelectorAll('.rounded-full');
      expect(circles).toHaveLength(10);
    });
  });
});
