type LogLevel = 'debug' | 'info' | 'warn' | 'error';

const LEVELS: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };

function getLogLevel(): LogLevel {
  if (typeof localStorage === 'undefined') return 'info';
  const level = localStorage.getItem('LOG_LEVEL') || 'info';
  return level as LogLevel;
}

function shouldLog(level: LogLevel): boolean {
  const currentLevel = getLogLevel();
  return LEVELS[level] >= LEVELS[currentLevel];
}

export const logger = {
  debug: (msg: string, ...args: unknown[]) => {
    if (shouldLog('debug')) {
      console.log(`[DEBUG] ${msg}`, ...args);
    }
  },
  info: (msg: string, ...args: unknown[]) => {
    if (shouldLog('info')) {
      console.log(`[INFO] ${msg}`, ...args);
    }
  },
  warn: (msg: string, ...args: unknown[]) => {
    if (shouldLog('warn')) {
      console.warn(`[WARN] ${msg}`, ...args);
    }
  },
  error: (msg: string, ...args: unknown[]) => {
    if (shouldLog('error')) {
      console.error(`[ERROR] ${msg}`, ...args);
    }
  },
};
