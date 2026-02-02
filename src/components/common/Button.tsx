import { ButtonHTMLAttributes, forwardRef } from 'react';
import { cn } from '../../lib/utils';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md' | 'lg';
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant = 'primary', size = 'md', children, ...props }, ref) => {
    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center rounded-xl font-medium transition-all duration-200',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-board-accent focus-visible:ring-offset-2 focus-visible:ring-offset-board-bg',
          'disabled:pointer-events-none disabled:opacity-50',
          'active:scale-[0.98]',
          {
            'accent-gradient text-white shadow-md hover:shadow-lg hover:scale-[1.02] glow-accent': variant === 'primary',
            'glass text-board-text hover:bg-board-card-hover hover:shadow-md': variant === 'secondary',
            'text-board-text-secondary hover:bg-board-card-hover hover:text-board-text': variant === 'ghost',
            'bg-status-error/90 text-white hover:bg-status-error shadow-md hover:shadow-lg': variant === 'danger',
          },
          {
            'h-8 px-3 text-sm': size === 'sm',
            'h-10 px-4 text-sm': size === 'md',
            'h-12 px-6 text-base': size === 'lg',
          },
          className
        )}
        {...props}
      >
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';
