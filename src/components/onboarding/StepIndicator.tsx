import { cn } from '../../lib/utils';

interface StepIndicatorProps {
  currentStep: number;
  totalSteps: number;
  stepLabels?: string[];
}

export function StepIndicator({ currentStep, totalSteps, stepLabels }: StepIndicatorProps) {
  return (
    <div className="w-full">
      <div className="flex items-start">
        {Array.from({ length: totalSteps }, (_, i) => {
          const stepNum = i + 1;
          const isCompleted = stepNum < currentStep;
          const isCurrent = stepNum === currentStep;
          const isLast = i === totalSteps - 1;
          
          return (
            <div key={i} className={cn('flex items-center', isLast ? '' : 'flex-1')}>
              {/* Step circle and label */}
              <div className="flex flex-col items-center">
                <div
                  className={cn(
                    'w-8 h-8 rounded-full flex items-center justify-center text-sm font-medium transition-all duration-300',
                    isCompleted && 'bg-status-success text-white',
                    isCurrent && 'bg-board-accent text-white ring-4 ring-board-accent/20',
                    !isCompleted && !isCurrent && 'bg-board-surface-raised text-board-text-muted border border-board-border'
                  )}
                >
                  {isCompleted ? (
                    <svg
                      xmlns="http://www.w3.org/2000/svg"
                      width="16"
                      height="16"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="2.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    >
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  ) : (
                    stepNum
                  )}
                </div>
                {stepLabels && (
                  <div
                    className={cn(
                      'text-xs text-center mt-2 whitespace-nowrap transition-colors',
                      isCurrent ? 'text-board-text font-medium' : 'text-board-text-muted'
                    )}
                  >
                    {stepLabels[i]}
                  </div>
                )}
              </div>
              
              {/* Connector line */}
              {!isLast && (
                <div
                  className={cn(
                    'flex-1 h-0.5 mx-3 mt-4 -translate-y-1/2 transition-colors duration-300',
                    stepNum < currentStep ? 'bg-status-success' : 'bg-board-border'
                  )}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
