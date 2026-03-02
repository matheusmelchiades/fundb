import * as vscode from 'vscode';

let channel: vscode.OutputChannel;

export function initLogger(): vscode.OutputChannel {
  channel = vscode.window.createOutputChannel('FunDB');
  return channel;
}

export function log(message: string): void {
  const ts = new Date().toISOString().slice(11, 23);
  channel?.appendLine(`[${ts}] ${message}`);
}

export function logError(message: string, err?: unknown): void {
  const detail = err instanceof Error ? err.message : String(err ?? '');
  log(`ERROR ${message}${detail ? ': ' + detail : ''}`);
}
