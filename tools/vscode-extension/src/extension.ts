import * as vscode from 'vscode';
import { ConnectionManager } from './connection/connectionManager';
import { QueryRunner } from './query/queryRunner';
import { ResultsPanel } from './views/resultsPanel';
import { StatusBar } from './views/statusBar';
import { SidebarProvider } from './views/sidebarProvider';
import { initLogger, log } from './util/logger';

let connectionManager: ConnectionManager;

export function activate(context: vscode.ExtensionContext): void {
  const outputChannel = initLogger();
  log('FunDB extension activated');

  connectionManager = new ConnectionManager();
  const resultsPanel = new ResultsPanel(context.extensionUri);
  const queryRunner = new QueryRunner(connectionManager, resultsPanel);
  const statusBar = new StatusBar(connectionManager);
  const sidebarProvider = new SidebarProvider(connectionManager);

  vscode.window.registerTreeDataProvider('fundb.explorer', sidebarProvider);

  context.subscriptions.push(
    outputChannel,
    vscode.commands.registerCommand('fundb.connect', (arg?: string | { label?: string }) => {
      const name = typeof arg === 'string' ? arg : typeof arg?.label === 'string' ? arg.label : undefined;
      return connectionManager.connect(name);
    }),
    vscode.commands.registerCommand('fundb.disconnect', () =>
      connectionManager.disconnect(),
    ),
    vscode.commands.registerCommand('fundb.newConnection', () =>
      connectionManager.newConnection(),
    ),
    vscode.commands.registerCommand('fundb.editConnection', (item?: { label?: string }) =>
      connectionManager.editConnection(typeof item?.label === 'string' ? item.label : undefined),
    ),
    vscode.commands.registerCommand('fundb.removeConnection', (item?: { label?: string }) =>
      connectionManager.removeConnection(typeof item?.label === 'string' ? item.label : undefined),
    ),
    vscode.commands.registerCommand('fundb.executeQuery', () =>
      queryRunner.executeQuery(),
    ),
    vscode.commands.registerCommand('fundb.executeSelection', () =>
      queryRunner.executeSelection(),
    ),
    vscode.commands.registerCommand('fundb.exportJson', () =>
      resultsPanel.exportJson(),
    ),
    vscode.commands.registerCommand('fundb.exportCsv', () =>
      resultsPanel.exportCsv(),
    ),
    vscode.commands.registerCommand('fundb.refreshExplorer', () =>
      sidebarProvider.refresh(),
    ),
    vscode.commands.registerCommand('fundb.browseCollection', async (collectionName: string) => {
      if (!connectionManager.isConnected) {
        vscode.window.showWarningMessage('Connect to FunDB first.');
        return;
      }
      const query = `SELECT * FROM ${collectionName} LIMIT 100`;
      resultsPanel.show();
      resultsPanel.postMessage({ type: 'loading', query });
      const start = Date.now();
      try {
        const result = await connectionManager.execute(query);
        resultsPanel.postMessage({
          type: 'results',
          columns: result.columns,
          rows: result.rows,
          commandTag: result.commandTag,
          rowCount: result.rowCount,
          elapsedMs: Date.now() - start,
          query,
        });
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : String(err);
        resultsPanel.postMessage({
          type: 'error',
          message,
          elapsedMs: Date.now() - start,
        });
      }
    }),
    statusBar,
    resultsPanel,
  );
}

export function deactivate(): void {
  log('FunDB extension deactivated');
  connectionManager?.dispose();
}
