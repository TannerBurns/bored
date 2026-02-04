import { useState, useCallback } from 'react';
import * as Dialog from '@radix-ui/react-dialog';
import { StepIndicator } from './StepIndicator';
import { WelcomeStep } from './WelcomeStep';
import { CreateBoardStep } from './CreateBoardStep';
import { GetStartedStep } from './GetStartedStep';
import { getProjects } from '../../lib/tauri';
import type { Project } from '../../types';

interface OnboardingWizardProps {
  projects: Project[];
  onComplete: () => void;
  onProjectsChange: () => Promise<void>;
}

const STEP_LABELS = ['Add Project', 'Create Board', 'Get Started'];
const TOTAL_STEPS = 3;

export function OnboardingWizard({ 
  projects: initialProjects, 
  onComplete,
  onProjectsChange,
}: OnboardingWizardProps) {
  const [currentStep, setCurrentStep] = useState(1);
  const [projects, setProjects] = useState<Project[]>(initialProjects);

  const handleProjectAdded = useCallback(async () => {
    const updatedProjects = await getProjects();
    setProjects(updatedProjects);
    await onProjectsChange();
  }, [onProjectsChange]);

  const goToStep = (step: number) => {
    if (step >= 1 && step <= TOTAL_STEPS) {
      setCurrentStep(step);
    }
  };

  const handleSkip = () => {
    onComplete();
  };

  return (
    <Dialog.Root open={true} onOpenChange={() => {}}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 bg-black/70 backdrop-blur-md z-50" />
        <Dialog.Content
          onInteractOutside={(e) => e.preventDefault()}
          onEscapeKeyDown={(e) => e.preventDefault()}
          className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-50 w-full max-w-2xl max-h-[90vh] overflow-y-auto rounded-2xl glass-intense p-6 shadow-2xl focus:outline-none"
        >
          {/* Accent border at top */}
          <div className="absolute top-0 left-6 right-6 h-px bg-board-accent/40" />

          {/* Header with step indicator */}
          <div className="mb-6">
            <Dialog.Title className="sr-only">Onboarding Wizard</Dialog.Title>
            <StepIndicator
              currentStep={currentStep}
              totalSteps={TOTAL_STEPS}
              stepLabels={STEP_LABELS}
            />
          </div>

          {/* Step content */}
          <div className="min-h-[400px]">
            {currentStep === 1 && (
              <WelcomeStep
                projects={projects}
                onProjectAdded={handleProjectAdded}
                onNext={() => goToStep(2)}
                onSkip={handleSkip}
              />
            )}

            {currentStep === 2 && (
              <CreateBoardStep
                onNext={() => goToStep(3)}
                onBack={() => goToStep(1)}
                onSkip={handleSkip}
                defaultName={projects[0]?.name || ''}
              />
            )}

            {currentStep === 3 && (
              <GetStartedStep
                onComplete={onComplete}
                onBack={() => goToStep(2)}
              />
            )}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
