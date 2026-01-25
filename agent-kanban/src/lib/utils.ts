import { clsx, type ClassValue } from 'clsx';

export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}
