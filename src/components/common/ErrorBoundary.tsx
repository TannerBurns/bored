import React from 'react';
import { logger } from '../../lib/logger';

interface ErrorBoundaryProps {
  children: React.ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

/**
 * Top-level error boundary that catches rendering errors and displays
 * a recovery UI instead of a blank screen.
 */
export class ErrorBoundary extends React.Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    logger.error('React ErrorBoundary caught an error:', error, errorInfo.componentStack);
  }

  handleReload = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="flex h-screen items-center justify-center app-gradient-bg text-board-text p-8">
          <div className="max-w-md w-full bg-board-column rounded-xl p-6 shadow-xl border border-board-border space-y-4">
            <h2 className="text-lg font-semibold text-board-text">Something went wrong</h2>
            <p className="text-sm text-board-text-muted">
              An unexpected error occurred. You can try recovering or reload the app.
            </p>
            {this.state.error && (
              <pre className="text-xs text-status-error bg-board-surface rounded-lg p-3 overflow-auto max-h-32">
                {this.state.error.message}
              </pre>
            )}
            <div className="flex gap-3">
              <button
                onClick={this.handleReload}
                className="px-4 py-2 bg-board-accent text-white text-sm rounded-lg hover:bg-board-accent-hover transition-colors"
              >
                Try Again
              </button>
              <button
                onClick={() => window.location.reload()}
                className="px-4 py-2 bg-board-surface text-board-text-muted text-sm rounded-lg border border-board-border hover:text-board-text transition-colors"
              >
                Reload App
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
