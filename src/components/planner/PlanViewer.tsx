import { MarkdownViewer } from '../common/MarkdownViewer';
import type { ProjectPlan } from '../../types';

interface PlanViewerProps {
  markdown: string;
  planJson?: ProjectPlan;
}

export function PlanViewer({ markdown, planJson }: PlanViewerProps) {
  return (
    <div className="space-y-6">
      {/* Markdown View */}
      <div className="prose dark:prose-invert max-w-none">
        <MarkdownViewer content={markdown} />
      </div>

      {/* Structured View (if available) */}
      {planJson && (
        <div className="border-t dark:border-gray-700 pt-6 mt-6">
          <h3 className="text-lg font-semibold mb-4 text-gray-900 dark:text-white">
            Work Breakdown
          </h3>
          
          <div className="space-y-4">
            {planJson.epics.map((epic, epicIdx) => (
              <div
                key={epicIdx}
                className="border dark:border-gray-700 rounded-lg overflow-hidden"
              >
                <div className="bg-purple-50 dark:bg-purple-900/20 px-4 py-3">
                  <div className="flex items-center gap-2">
                    <span className="bg-purple-500 text-white text-xs font-medium px-2 py-0.5 rounded">
                      Epic {epicIdx + 1}
                    </span>
                    {epic.dependsOn && (
                      <span className="text-xs text-gray-500 dark:text-gray-400">
                        depends on: {epic.dependsOn}
                      </span>
                    )}
                  </div>
                  <h4 className="font-medium text-gray-900 dark:text-white mt-2">
                    {epic.title}
                  </h4>
                  <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
                    {epic.description}
                  </p>
                </div>

                <div className="divide-y dark:divide-gray-700">
                  {epic.tickets.map((ticket, ticketIdx) => (
                    <div key={ticketIdx} className="px-4 py-3 bg-white dark:bg-gray-900">
                      <div className="flex items-start gap-3">
                        <span className="text-gray-400 text-sm font-mono">
                          {epicIdx + 1}.{ticketIdx + 1}
                        </span>
                        <div className="flex-1">
                          <h5 className="font-medium text-gray-900 dark:text-white">
                            {ticket.title}
                          </h5>
                          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
                            {ticket.description}
                          </p>
                          {ticket.acceptanceCriteria && ticket.acceptanceCriteria.length > 0 && (
                            <ul className="mt-2 text-sm text-gray-600 dark:text-gray-300 list-disc list-inside">
                              {ticket.acceptanceCriteria.map((criteria, i) => (
                                <li key={i}>{criteria}</li>
                              ))}
                            </ul>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
