import {
  AreaChart,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ResponsiveContainer,
  Cell,
  PieChart,
  Pie,
} from 'recharts';
import type { TooltipProps } from 'recharts';
import { useDashboardData, type TimeRange } from '../../hooks/useDashboardData';
import { cn } from '../../lib/utils';
import { getAgentDisplayName, getAgentIcon, getAgentBrandColor } from '../common/AgentIcons';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { useEffect } from 'react';
import { TicketIcon, TaskIcon, DollarIcon, TokenIcon, CommitIcon, CodeIcon, RunIcon, ClockIcon, CycleIcon, CostPerIcon } from './DashboardIcons';

type RenderLabel = TooltipProps<number, string>['labelFormatter'];
type RenderFormatter = TooltipProps<number, string>['formatter'];

const TIME_RANGES: { label: string; value: TimeRange }[] = [
  { label: '7d', value: 7 },
  { label: '30d', value: 30 },
  { label: '90d', value: 90 },
  { label: 'All', value: null },
];

const CHART_COLORS = {
  primary: '#8b5cf6',
  secondary: '#3b82f6',
  tertiary: '#10b981',
};

const BAR_COLORS = ['#8b5cf6', '#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#ec4899'];

const FALLBACK_AGENT_COLOR = '#8b5cf6';

function getAgentColor(agentType: string, agents: { id: string; brandColor: string | null }[]): string {
  const match = agents.find((a) => a.id === agentType);
  return getAgentBrandColor(agentType, match?.brandColor) || FALLBACK_AGENT_COLOR;
}

const TOOLTIP_CONTENT_STYLE: React.CSSProperties = {
  backgroundColor: 'var(--app-board-bg-solid)',
  border: '1px solid var(--app-glass-border)',
  borderRadius: '8px',
  fontSize: '12px',
  boxShadow: '0 4px 12px rgba(0,0,0,0.15)',
};

const TOOLTIP_TEXT_STYLE: React.CSSProperties = {
  color: 'var(--app-board-text)',
};

const TOOLTIP_LABEL_STYLE: React.CSSProperties = {
  color: 'var(--app-board-text)',
  fontWeight: 600,
};

export function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1.0) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toLocaleString();
}

export function formatDuration(secs: number): string {
  if (secs < 60) return `${Math.round(secs)}s`;
  if (secs < 3600) return `${Math.round(secs / 60)}m`;
  return `${(secs / 3600).toFixed(1)}h`;
}

export function formatCycleTime(hours: number): string {
  if (hours <= 0) return '--';
  if (hours < 1) return `${Math.round(hours * 60)}m`;
  if (hours < 24) return `${hours.toFixed(1)}h`;
  const days = hours / 24;
  return days < 10 ? `${days.toFixed(1)}d` : `${Math.round(days)}d`;
}

export function formatDateLabel(dateStr: string): string {
  const d = new Date(dateStr + 'T00:00:00');
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

export function DashboardView() {
  const {
    summary,
    trends,
    modelBreakdown,
    agentBreakdown,
    timeRange,
    setTimeRange,
    isLoading,
  } = useDashboardData();

  const agents = useAgentRegistryStore((s) => s.agents);
  const loadAgents = useAgentRegistryStore((s) => s.loadAgents);
  useEffect(() => { loadAgents(); }, [loadAgents]);

  if (isLoading && !summary) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="text-board-text-muted text-sm">Loading dashboard...</div>
      </div>
    );
  }

  const s = summary;
  const hasData = s && (s.totalRuns > 0 || s.ticketsCompleted > 0 || s.tasksCompleted > 0);

  return (
    <div className="flex-1 overflow-y-auto space-y-5 pb-6">
      {/* Time range selector */}
      <div className="flex items-center justify-between">
        <div className="flex gap-1">
          {TIME_RANGES.map((tr) => (
            <button
              key={tr.label}
              onClick={() => setTimeRange(tr.value)}
              className={cn(
                'px-3 py-1.5 text-sm font-medium rounded-lg transition-all duration-200',
                timeRange === tr.value
                  ? 'bg-board-accent text-white shadow-sm'
                  : 'glass text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
              )}
            >
              {tr.label}
            </button>
          ))}
        </div>
      </div>

      {!hasData ? (
        <EmptyState />
      ) : (
        <>
          {/* Stat cards -- grouped: work output, then cost & efficiency */}
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            {/* Row 1: Work output */}
            <StatCard
              label="Tickets Done"
              value={s!.ticketsCompleted}
              icon={<TicketIcon />}
              color="text-status-success"
            />
            <StatCard
              label="Tasks Done"
              value={s!.tasksCompleted}
              icon={<TaskIcon />}
              color="text-status-info"
            />
            <StatCard
              label="Commits"
              value={s!.totalCommits}
              icon={<CommitIcon />}
              color="text-cyan-400"
            />
            <StatCard
              label="Lines Changed"
              value={`+${formatNumber(s!.totalLinesAdded)} / -${formatNumber(s!.totalLinesRemoved)}`}
              icon={<CodeIcon />}
              color="text-sky-400"
            />
            <StatCard
              label="Cycle Time"
              value={formatCycleTime(s!.avgCycleTimeHours)}
              subtitle="created to done"
              icon={<CycleIcon />}
              color="text-blue-400"
            />
            {/* Row 2: Cost & efficiency */}
            <StatCard
              label="Total Cost"
              value={formatCost(s!.totalCostUsd)}
              icon={<DollarIcon />}
              color="text-emerald-400"
            />
            <StatCard
              label="Cost / Ticket"
              value={s!.ticketsCompleted > 0
                ? formatCost(s!.totalCostUsd / s!.ticketsCompleted)
                : '--'}
              icon={<CostPerIcon />}
              color="text-teal-400"
            />
            <StatCard
              label="Tokens Used"
              value={formatNumber(s!.totalInputTokens + s!.totalOutputTokens)}
              icon={<TokenIcon />}
              color="text-board-accent"
            />
            <StatCard
              label="Agent Runs"
              value={s!.totalRuns}
              subtitle={`${Math.round(s!.successRate * 100)}% success`}
              icon={<RunIcon />}
              color="text-purple-400"
            />
            <StatCard
              label="Avg Run Time"
              value={formatDuration(s!.avgRunDurationSecs)}
              icon={<ClockIcon />}
              color="text-orange-400"
            />
          </div>

          {/* Trend charts */}
          {trends.length > 1 && (
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
              <ChartCard title="Activity">
                <ResponsiveContainer width="100%" height={200}>
                  <AreaChart data={trends}>
                    <defs>
                      <linearGradient id="gradTickets" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor={CHART_COLORS.tertiary} stopOpacity={0.3} />
                        <stop offset="95%" stopColor={CHART_COLORS.tertiary} stopOpacity={0} />
                      </linearGradient>
                      <linearGradient id="gradTasks" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor={CHART_COLORS.secondary} stopOpacity={0.3} />
                        <stop offset="95%" stopColor={CHART_COLORS.secondary} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
                    <XAxis
                      dataKey="date"
                      tickFormatter={formatDateLabel}
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <YAxis
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                      width={30}
                      allowDecimals={false}
                    />
                    <Tooltip
                      contentStyle={TOOLTIP_CONTENT_STYLE} itemStyle={TOOLTIP_TEXT_STYLE} labelStyle={TOOLTIP_LABEL_STYLE}
                      labelFormatter={((label) => formatDateLabel(String(label))) as RenderLabel}
                    />
                    <Area
                      type="monotone"
                      dataKey="ticketsCompleted"
                      name="Tickets"
                      stroke={CHART_COLORS.tertiary}
                      fill="url(#gradTickets)"
                      strokeWidth={2}
                    />
                    <Area
                      type="monotone"
                      dataKey="tasksCompleted"
                      name="Tasks"
                      stroke={CHART_COLORS.secondary}
                      fill="url(#gradTasks)"
                      strokeWidth={2}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </ChartCard>

              <ChartCard title="Cost">
                <ResponsiveContainer width="100%" height={200}>
                  <AreaChart data={trends}>
                    <defs>
                      <linearGradient id="gradCost" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor={CHART_COLORS.tertiary} stopOpacity={0.4} />
                        <stop offset="95%" stopColor={CHART_COLORS.tertiary} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
                    <XAxis
                      dataKey="date"
                      tickFormatter={formatDateLabel}
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <YAxis
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                      width={40}
                      tickFormatter={(v) => `$${v}`}
                    />
                    <Tooltip
                      contentStyle={TOOLTIP_CONTENT_STYLE} itemStyle={TOOLTIP_TEXT_STYLE} labelStyle={TOOLTIP_LABEL_STYLE}
                      labelFormatter={((label) => formatDateLabel(String(label))) as RenderLabel}
                      formatter={((value) => [formatCost(Number(value)), 'Cost']) as RenderFormatter}
                    />
                    <Area
                      type="monotone"
                      dataKey="costUsd"
                      name="Cost"
                      stroke={CHART_COLORS.tertiary}
                      fill="url(#gradCost)"
                      strokeWidth={2}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </ChartCard>

              <ChartCard title="Token Usage">
                <ResponsiveContainer width="100%" height={200}>
                  <AreaChart data={trends}>
                    <defs>
                      <linearGradient id="gradTokens" x1="0" y1="0" x2="0" y2="1">
                        <stop offset="5%" stopColor={CHART_COLORS.primary} stopOpacity={0.4} />
                        <stop offset="95%" stopColor={CHART_COLORS.primary} stopOpacity={0} />
                      </linearGradient>
                    </defs>
                    <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,0.06)" />
                    <XAxis
                      dataKey="date"
                      tickFormatter={formatDateLabel}
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <YAxis
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                      width={40}
                      tickFormatter={(v) => formatNumber(v)}
                    />
                    <Tooltip
                      contentStyle={TOOLTIP_CONTENT_STYLE} itemStyle={TOOLTIP_TEXT_STYLE} labelStyle={TOOLTIP_LABEL_STYLE}
                      labelFormatter={((label) => formatDateLabel(String(label))) as RenderLabel}
                      formatter={((value) => [formatNumber(Number(value)), 'Tokens']) as RenderFormatter}
                    />
                    <Area
                      type="monotone"
                      dataKey="tokensUsed"
                      name="Tokens"
                      stroke={CHART_COLORS.primary}
                      fill="url(#gradTokens)"
                      strokeWidth={2}
                    />
                  </AreaChart>
                </ResponsiveContainer>
              </ChartCard>
            </div>
          )}

          {/* Breakdowns row */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Top models */}
            {modelBreakdown.length > 0 && (
              <ChartCard title="Top Models">
                <div className="space-y-2.5">
                  {modelBreakdown.slice(0, 8).map((model, i) => {
                    const maxCost = modelBreakdown[0]?.costUsd || 1;
                    const barPct = Math.max(2, (model.costUsd / maxCost) * 100);
                    const totalTokens = model.inputTokens + model.outputTokens;
                    return (
                      <div key={model.model} className="space-y-1">
                        <div className="flex items-center justify-between text-sm">
                          <span className="font-medium text-board-text truncate mr-3">
                            {model.model}
                          </span>
                          <div className="flex items-center gap-3 flex-shrink-0 text-xs">
                            <span className="text-board-text-muted">
                              {formatNumber(totalTokens)} tok
                            </span>
                            <span className="font-mono font-medium text-board-text">
                              {formatCost(model.costUsd)}
                            </span>
                          </div>
                        </div>
                        <div className="h-2 rounded-full bg-board-card-hover overflow-hidden">
                          <div
                            className="h-full rounded-full transition-all duration-500"
                            style={{
                              width: `${barPct}%`,
                              backgroundColor: BAR_COLORS[i % BAR_COLORS.length],
                              opacity: 0.8,
                            }}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </ChartCard>
            )}

            {/* Agent distribution */}
            {agentBreakdown.length > 0 && (
              <ChartCard title="Agent Distribution">
                <div className="flex items-center gap-6">
                  <ResponsiveContainer width="50%" height={200}>
                    <PieChart>
                      <Pie
                        data={agentBreakdown}
                        dataKey="runCount"
                        nameKey="agentType"
                        cx="50%"
                        cy="50%"
                        innerRadius={50}
                        outerRadius={80}
                        paddingAngle={2}
                      >
                        {agentBreakdown.map((entry) => (
                          <Cell key={entry.agentType} fill={getAgentColor(entry.agentType, agents)} />
                        ))}
                      </Pie>
                      <Tooltip
                        contentStyle={TOOLTIP_CONTENT_STYLE} itemStyle={TOOLTIP_TEXT_STYLE} labelStyle={TOOLTIP_LABEL_STYLE}
                        formatter={((value, name) => [
                          `${value} runs`,
                          getAgentDisplayName(String(name)),
                        ]) as RenderFormatter}
                      />
                    </PieChart>
                  </ResponsiveContainer>
                  <div className="flex-1 space-y-3">
                    {agentBreakdown.map((agent) => {
                      const successPct =
                        agent.runCount > 0
                          ? Math.round((agent.successCount / agent.runCount) * 100)
                          : 0;
                      const color = getAgentColor(agent.agentType, agents);
                      const Icon = getAgentIcon(agent.agentType);
                      return (
                        <div key={agent.agentType} className="flex items-center gap-3">
                          <Icon size={18} style={{ color, flexShrink: 0 }} />
                          <div className="flex-1 min-w-0">
                            <div className="text-sm font-medium text-board-text truncate">
                              {getAgentDisplayName(agent.agentType)}
                            </div>
                            <div className="text-xs text-board-text-muted">
                              {agent.runCount} runs &middot; {successPct}% success &middot; avg{' '}
                              {formatDuration(agent.avgDurationSecs)}
                            </div>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              </ChartCard>
            )}
          </div>
        </>
      )}
    </div>
  );
}

// ── Sub-components ─────────────────────────────────────────────────

function StatCard({
  label,
  value,
  subtitle,
  icon,
  color,
}: {
  label: string;
  value: string | number;
  subtitle?: string;
  icon: React.ReactNode;
  color?: string;
}) {
  return (
    <div className="glass rounded-xl p-4 flex flex-col gap-1.5 hover:bg-board-card-hover transition-colors">
      <div className="flex items-center gap-2">
        <span className={cn('opacity-70', color)}>{icon}</span>
        <span className="text-xs font-medium text-board-text-muted uppercase tracking-wider truncate">
          {label}
        </span>
      </div>
      <div className={cn('text-xl font-bold text-board-text', color)}>
        {typeof value === 'number' ? formatNumber(value) : value}
      </div>
      {subtitle && (
        <div className="text-xs text-board-text-muted">{subtitle}</div>
      )}
    </div>
  );
}

function ChartCard({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="glass rounded-xl p-4">
      <h3 className="text-sm font-semibold text-board-text mb-3">{title}</h3>
      {children}
    </div>
  );
}

function EmptyState() {
  return (
    <div className="flex-1 flex flex-col items-center justify-center gap-4 py-20">
      <div className="w-16 h-16 rounded-2xl glass-subtle flex items-center justify-center">
        <svg
          xmlns="http://www.w3.org/2000/svg"
          width="28"
          height="28"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="text-board-text-muted"
        >
          <path d="M3 3v18h18" />
          <path d="M18.7 8l-5.1 5.2-2.8-2.7L7 14.3" />
        </svg>
      </div>
      <div className="text-center">
        <p className="text-board-text font-medium">No activity yet</p>
        <p className="text-sm text-board-text-muted mt-1">
          Start running agents on tickets to see your dashboard come to life.
        </p>
      </div>
    </div>
  );
}
