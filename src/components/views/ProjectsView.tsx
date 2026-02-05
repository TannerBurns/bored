import { ProjectsList } from '../settings';

export function ProjectsView() {
  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex-1 overflow-auto glass rounded-lg p-4">
        <ProjectsList />
      </div>
    </div>
  );
}
