import * as vscode from 'vscode';
import { ConnectionManager } from '../connection/connectionManager';

type TreeItemKind = 'profile' | 'category' | 'collection' | 'column' | 'placeholder';

class FunDBTreeItem extends vscode.TreeItem {
  constructor(
    label: string,
    public readonly kind: TreeItemKind,
    collapsible: vscode.TreeItemCollapsibleState = vscode.TreeItemCollapsibleState.None,
  ) {
    super(label, collapsible);
  }
}

interface CollectionInfo {
  name: string;
  rowCount: number;
  columns: string[];
}

export class SidebarProvider implements vscode.TreeDataProvider<FunDBTreeItem> {
  private readonly _onDidChangeTreeData = new vscode.EventEmitter<FunDBTreeItem | undefined>();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private collections: CollectionInfo[] = [];

  constructor(private connectionManager: ConnectionManager) {
    connectionManager.onDidConnect(() => {
      this.loadCollections();
    });
    connectionManager.onDidDisconnect(() => {
      this.collections = [];
      this._onDidChangeTreeData.fire(undefined);
    });
    connectionManager.onDidChangeProfiles(() => {
      this._onDidChangeTreeData.fire(undefined);
    });
  }

  refresh(): void {
    if (this.connectionManager.isConnected) {
      this.loadCollections();
    } else {
      this._onDidChangeTreeData.fire(undefined);
    }
  }

  private async loadCollections(): Promise<void> {
    try {
      const result = await this.connectionManager.execute(
        "SELECT name FROM _collections ORDER BY name",
      );
      const names = result.rows.map((row) => row[0] ?? '').filter(Boolean);

      // Load row count + columns for each collection in parallel
      const infos: CollectionInfo[] = await Promise.all(
        names.map(async (name) => {
          let rowCount = 0;
          let columns: string[] = [];
          try {
            const countResult = await this.connectionManager.execute(
              `SELECT * FROM ${name} LIMIT 1`,
            );
            rowCount = countResult.rowCount;
            columns = countResult.columns.map((c) => c.name);

            // If LIMIT 1 returned 1 row, do a full scan count
            if (rowCount >= 1) {
              const fullResult = await this.connectionManager.execute(
                `SELECT * FROM ${name}`,
              );
              rowCount = fullResult.rowCount;
            }
          } catch {
            // Collection might be empty or inaccessible
          }
          return { name, rowCount, columns };
        }),
      );

      this.collections = infos;
    } catch {
      this.collections = [];
    }
    this._onDidChangeTreeData.fire(undefined);
  }

  getTreeItem(element: FunDBTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(element?: FunDBTreeItem): FunDBTreeItem[] {
    if (!element) {
      return this.getRootItems();
    }

    if (element.kind === 'profile' && element.contextValue === 'connectedProfile') {
      const collectionsItem = new FunDBTreeItem(
        'Collections',
        'category',
        vscode.TreeItemCollapsibleState.Expanded,
      );
      collectionsItem.iconPath = new vscode.ThemeIcon('folder');
      return [collectionsItem];
    }

    if (element.kind === 'category' && element.label === 'Collections') {
      if (this.collections.length === 0) {
        const item = new FunDBTreeItem('No collections found', 'placeholder');
        item.iconPath = new vscode.ThemeIcon('info');
        return [item];
      }
      return this.collections.map((col) => {
        const item = new FunDBTreeItem(
          col.name,
          'collection',
          col.columns.length > 0
            ? vscode.TreeItemCollapsibleState.Collapsed
            : vscode.TreeItemCollapsibleState.None,
        );
        item.iconPath = new vscode.ThemeIcon('table');
        item.contextValue = 'collection';
        item.description = `${col.rowCount} rows`;
        item.tooltip = `${col.name} — ${col.rowCount} rows, ${col.columns.length} columns\nClick to view data`;
        item.command = {
          command: 'fundb.browseCollection',
          title: 'View Data',
          arguments: [col.name],
        };
        return item;
      });
    }

    // Column children of a collection
    if (element.kind === 'collection') {
      const col = this.collections.find((c) => c.name === element.label);
      if (col && col.columns.length > 0) {
        return col.columns.map((colName) => {
          const item = new FunDBTreeItem(colName, 'column');
          item.iconPath = colName.startsWith('_')
            ? new vscode.ThemeIcon('key')
            : new vscode.ThemeIcon('symbol-field');
          item.tooltip = colName.startsWith('_') ? `System column: ${colName}` : colName;
          return item;
        });
      }
    }

    return [];
  }

  private getRootItems(): FunDBTreeItem[] {
    const profiles = this.connectionManager.getProfiles();
    const connectedName = this.connectionManager.connectedProfileName;

    if (profiles.length === 0) {
      const item = new FunDBTreeItem('No connections configured', 'placeholder');
      item.iconPath = new vscode.ThemeIcon('info');
      item.command = {
        command: 'fundb.newConnection',
        title: 'New Connection',
      };
      item.tooltip = 'Click to add a connection';
      return [item];
    }

    return profiles.map((p) => {
      const isConnected = p.name === connectedName;
      const item = new FunDBTreeItem(
        p.name,
        'profile',
        isConnected
          ? vscode.TreeItemCollapsibleState.Expanded
          : vscode.TreeItemCollapsibleState.None,
      );

      item.description = `${p.host}:${p.port}`;
      item.contextValue = isConnected ? 'connectedProfile' : 'profile';

      if (isConnected) {
        item.iconPath = new vscode.ThemeIcon('database', new vscode.ThemeColor('charts.green'));
        item.tooltip = `Connected — ${p.user}@${p.host}:${p.port}/${p.database}`;
      } else {
        item.iconPath = new vscode.ThemeIcon('database');
        item.tooltip = `${p.user}@${p.host}:${p.port}/${p.database} — Click to connect`;
        item.command = {
          command: 'fundb.connect',
          title: 'Connect',
          arguments: [p.name],
        };
      }

      return item;
    });
  }
}
