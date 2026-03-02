import * as vscode from 'vscode';
import { ConnectionManager } from '../connection/connectionManager';
import { ResultsPanel } from '../views/resultsPanel';
import { log, logError } from '../util/logger';

export class QueryRunner {
  constructor(
    private connectionManager: ConnectionManager,
    private resultsPanel: ResultsPanel,
  ) {}

  async executeQuery(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showWarningMessage('No active editor.');
      return;
    }
    const query = editor.document.getText().trim();
    if (!query) {
      vscode.window.showWarningMessage('Editor is empty.');
      return;
    }
    await this.run(query);
  }

  async executeSelection(): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.selection.isEmpty) {
      vscode.window.showWarningMessage('No text selected.');
      return;
    }
    const query = editor.document.getText(editor.selection).trim();
    if (!query) {
      vscode.window.showWarningMessage('Selection is empty.');
      return;
    }
    await this.run(query);
  }

  private async run(query: string): Promise<void> {
    if (!this.connectionManager.isConnected) {
      const action = await vscode.window.showWarningMessage(
        'Not connected to FunDB.',
        'Connect',
      );
      if (action === 'Connect') {
        await this.connectionManager.connect();
      }
      if (!this.connectionManager.isConnected) return;
    }

    this.resultsPanel.show();
    this.resultsPanel.postMessage({ type: 'loading', query });

    const preview = query.length > 80 ? query.substring(0, 80) + '...' : query;
    log(`Executing query: ${preview}`);

    const start = Date.now();
    try {
      const result = await this.connectionManager.execute(query);
      const elapsedMs = Date.now() - start;
      log(`Query OK: ${result.commandTag} — ${result.rowCount} row(s) in ${elapsedMs}ms`);
      this.resultsPanel.postMessage({
        type: 'results',
        columns: result.columns,
        rows: result.rows,
        commandTag: result.commandTag,
        rowCount: result.rowCount,
        elapsedMs,
        query,
      });
    } catch (err) {
      const elapsedMs = Date.now() - start;
      logError(`Query failed (${elapsedMs}ms)`, err);
      this.resultsPanel.postMessage({
        type: 'error',
        message: `${err}`,
        elapsedMs,
        query,
      });
    }
  }
}
