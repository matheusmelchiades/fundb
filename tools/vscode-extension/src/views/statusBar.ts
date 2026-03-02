import * as vscode from 'vscode';
import { ConnectionManager } from '../connection/connectionManager';

export class StatusBar implements vscode.Disposable {
  private item: vscode.StatusBarItem;
  private disposables: vscode.Disposable[] = [];

  constructor(private connectionManager: ConnectionManager) {
    this.item = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      100,
    );

    this.disposables.push(
      connectionManager.onDidConnect(() => this.update()),
      connectionManager.onDidDisconnect(() => this.update()),
    );

    this.update();
    this.item.show();
  }

  private update(): void {
    if (this.connectionManager.isConnected) {
      this.item.text = `$(database) FunDB: ${this.connectionManager.displayLabel}`;
      this.item.tooltip = 'Click to disconnect from FunDB';
      this.item.command = 'fundb.disconnect';
      this.item.backgroundColor = undefined;
    } else {
      this.item.text = '$(database) FunDB: Not Connected';
      this.item.tooltip = 'Click to connect to FunDB';
      this.item.command = 'fundb.connect';
      this.item.backgroundColor = undefined;
    }
  }

  dispose(): void {
    this.item.dispose();
    this.disposables.forEach((d) => d.dispose());
  }
}
