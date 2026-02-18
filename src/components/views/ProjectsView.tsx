import { ProjectsList } from '../settings';

interface ProjectsViewProps {
  onProjectsChange?: () => void;
}

export function ProjectsView({ onProjectsChange }: ProjectsViewProps) {
  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex-1 overflow-auto glass rounded-lg p-4">
        <ProjectsList onProjectsChange={onProjectsChange} />
      </div>
    </div>
  );
}
