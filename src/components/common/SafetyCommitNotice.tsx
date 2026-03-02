import { cn } from '../../lib/utils';
import type { AgentRun } from '../../types';

interface SafetyCommit {
  commit_hash?: string;
  created_at?: string;
  target_branch?: string;
  detour_branch?: string;
  merged_to_target?: boolean;
  branch?: string;
}

type NoticeVariant = 'success' | 'warning' | 'info';

function getVariant(sc: SafetyCommit): NoticeVariant {
  const isDetour = !!sc.target_branch;
  if (isDetour && sc.merged_to_target) return 'success';
  if (isDetour) return 'warning';
  return 'info';
}

const VARIANT_STYLES: Record<NoticeVariant, {
  bg: string; border: string; text: string; textMuted: string; codeBg: string;
}> = {
  success: {
    bg: 'bg-emerald-500/10',
    border: 'border-emerald-500/25',
    text: 'text-emerald-400',
    textMuted: 'text-emerald-400/80',
    codeBg: 'bg-emerald-500/15 text-emerald-300',
  },
  warning: {
    bg: 'bg-amber-500/10',
    border: 'border-amber-500/25',
    text: 'text-amber-400',
    textMuted: 'text-amber-400/80',
    codeBg: 'bg-amber-500/15 text-amber-300',
  },
  info: {
    bg: 'bg-blue-500/10',
    border: 'border-blue-500/25',
    text: 'text-blue-400',
    textMuted: 'text-blue-400/80',
    codeBg: 'bg-blue-500/15 text-blue-300',
  },
};

function VariantIcon({ variant }: { variant: NoticeVariant }) {
  return (
    <svg className="w-3.5 h-3.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      {variant === 'success' ? (
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
      ) : variant === 'warning' ? (
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5Z" />
      ) : (
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      )}
    </svg>
  );
}

function Code({ children, className }: { children: React.ReactNode; className: string }) {
  return <code className={cn('px-1 py-0.5 rounded', className)}>{children}</code>;
}

export function SafetyCommitNotice({ run, className }: { run: AgentRun; className?: string }) {
  const meta = run.metadata as Record<string, unknown> | undefined;
  if (!meta?.safety_commit) return null;

  const sc = meta.safety_commit as SafetyCommit;
  const variant = getVariant(sc);
  const styles = VARIANT_STYLES[variant];
  const isDetour = !!sc.target_branch;
  const hasCommitHash = !!sc.commit_hash;

  let title: string;
  let body: React.ReactNode;

  if (isDetour && sc.merged_to_target) {
    title = 'Merged to target';
    body = (
      <>
        Agent's work{hasCommitHash ? ' (including uncommitted changes)' : ''} merged into{' '}
        <Code className={styles.codeBg}>{sc.target_branch}</Code>.
        {hasCommitHash && ' No work was lost.'}
      </>
    );
  } else if (isDetour) {
    title = 'Needs manual merge';
    body = (
      <>
        Agent's work is on <Code className={styles.codeBg}>{sc.detour_branch}</Code>.
        {' '}Merge into <Code className={styles.codeBg}>{sc.target_branch}</Code>
        {' '}manually with <Code className={styles.codeBg}>git merge {sc.detour_branch}</Code>.
      </>
    );
  } else {
    title = 'Uncommitted changes saved';
    body = (
      <>
        The agent left some changes uncommitted. They've been automatically saved
        {sc.branch && (
          <>{' '}on <Code className={styles.codeBg}>{sc.branch}</Code></>
        )}.
        {' '}No work was lost.
      </>
    );
  }

  return (
    <div className={cn('p-2.5 rounded-lg border', styles.bg, styles.border, className)}>
      <p className={cn('text-xs font-medium flex items-center gap-1.5', styles.text)}>
        <VariantIcon variant={variant} />
        {title}
      </p>
      <p className={cn('text-xs mt-1', styles.textMuted)}>
        {body}
        {hasCommitHash && (
          <span className="ml-1">
            Commit: <Code className={styles.codeBg}>{sc.commit_hash}</Code>
          </span>
        )}
      </p>
    </div>
  );
}
