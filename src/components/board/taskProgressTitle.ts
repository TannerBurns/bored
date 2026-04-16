export function taskProgressTitle(isEpic: boolean, done: number, total: number): string {
  return isEpic
    ? `${done} of ${total} child tickets in Done`
    : `${done} of ${total} tasks completed`;
}
