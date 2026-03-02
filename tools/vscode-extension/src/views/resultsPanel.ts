import * as vscode from 'vscode';

export class ResultsPanel implements vscode.Disposable {
  private panel: vscode.WebviewPanel | null = null;
  private lastData: unknown = null;

  constructor(private extensionUri: vscode.Uri) {}

  show(): void {
    if (this.panel) {
      this.panel.reveal(vscode.ViewColumn.Beside, true);
      return;
    }

    this.panel = vscode.window.createWebviewPanel(
      'fundb.results',
      'FunDB Results',
      { viewColumn: vscode.ViewColumn.Beside, preserveFocus: true },
      {
        enableScripts: true,
        retainContextWhenHidden: true,
      },
    );

    this.panel.webview.html = this.getHtml();

    this.panel.webview.onDidReceiveMessage((msg) => {
      if (msg.type === 'export') {
        this.handleExport(msg.format);
      }
    });

    this.panel.onDidDispose(() => {
      this.panel = null;
    });
  }

  postMessage(data: unknown): void {
    this.lastData = data;
    this.panel?.webview.postMessage(data);
  }

  exportJson(): void {
    this.handleExport('json');
  }

  exportCsv(): void {
    this.handleExport('csv');
  }

  private async handleExport(format: 'json' | 'csv'): Promise<void> {
    const data = this.lastData as {
      type: string;
      columns?: { name: string }[];
      rows?: (string | null)[][];
    } | null;

    if (!data || data.type !== 'results' || !data.columns || !data.rows) {
      vscode.window.showWarningMessage('No results to export.');
      return;
    }

    let content: string;
    let language: string;

    if (format === 'json') {
      const dicts = data.rows.map((row) => {
        const obj: Record<string, string | null> = {};
        data.columns!.forEach((col, i) => {
          obj[col.name] = row[i];
        });
        return obj;
      });
      content = JSON.stringify(dicts, null, 2);
      language = 'json';
    } else {
      const escapeCsv = (v: string) => {
        if (v.includes(',') || v.includes('"') || v.includes('\n')) {
          return `"${v.replace(/"/g, '""')}"`;
        }
        return v;
      };
      const header = data.columns.map((c) => escapeCsv(c.name)).join(',');
      const body = data.rows.map((row) =>
        row.map((v) => (v === null ? '' : escapeCsv(v))).join(','),
      );
      content = [header, ...body].join('\n');
      language = 'csv';
    }

    const doc = await vscode.workspace.openTextDocument({
      content,
      language,
    });
    await vscode.window.showTextDocument(doc, { preview: true });
  }

  private getHtml(): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }

  body {
    font-family: var(--vscode-font-family, Inter, -apple-system, sans-serif);
    background: var(--vscode-editor-background);
    color: var(--vscode-editor-foreground);
  }

  .toolbar {
    padding: 8px 12px;
    display: flex;
    align-items: center;
    gap: 8px;
    border-bottom: 1px solid var(--vscode-panel-border);
    background: var(--vscode-editor-background);
    position: sticky;
    top: 0;
    z-index: 10;
  }

  .toolbar button {
    background: #E8872B;
    color: #fff;
    border: none;
    padding: 4px 12px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    font-family: inherit;
  }

  .toolbar button:hover {
    background: #C46A15;
  }

  .info {
    padding: 6px 12px;
    font-size: 12px;
    color: var(--vscode-descriptionForeground, #6B7280);
    border-bottom: 1px solid var(--vscode-panel-border);
    font-family: 'JetBrains Mono', var(--vscode-editor-font-family, monospace);
  }

  .info .query-preview {
    opacity: 0.7;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 600px;
  }

  .table-wrapper {
    overflow: auto;
    max-height: calc(100vh - 100px);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-family: 'JetBrains Mono', var(--vscode-editor-font-family, monospace);
    font-size: 13px;
  }

  th {
    background: var(--vscode-editorGroupHeader-tabsBackground, #1A1A2E);
    color: #E8872B;
    text-align: left;
    padding: 6px 12px;
    position: sticky;
    top: 0;
    font-weight: 600;
    white-space: nowrap;
    border-bottom: 2px solid #E8872B;
  }

  td {
    padding: 4px 12px;
    border-bottom: 1px solid var(--vscode-panel-border);
    white-space: pre;
    max-width: 400px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  tr:hover td {
    background: var(--vscode-list-hoverBackground);
  }

  td.null {
    color: var(--vscode-descriptionForeground, #6B7280);
    font-style: italic;
  }

  .status-bar {
    padding: 4px 12px;
    font-size: 11px;
    color: var(--vscode-descriptionForeground, #6B7280);
    border-top: 1px solid var(--vscode-panel-border);
    position: sticky;
    bottom: 0;
    background: var(--vscode-editor-background);
  }

  .error {
    color: var(--vscode-errorForeground, #E11D48);
    padding: 16px;
    white-space: pre-wrap;
    font-family: 'JetBrains Mono', var(--vscode-editor-font-family, monospace);
    font-size: 13px;
  }

  .loading {
    padding: 16px;
    color: var(--vscode-descriptionForeground, #6B7280);
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--vscode-panel-border);
    border-top-color: #E8872B;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .empty {
    padding: 16px;
    color: var(--vscode-descriptionForeground, #6B7280);
    text-align: center;
  }
</style>
</head>
<body>
  <div class="toolbar">
    <button id="export-json">Export JSON</button>
    <button id="export-csv">Export CSV</button>
  </div>
  <div class="info" id="info"></div>
  <div id="content">
    <div class="empty">Run a query to see results here.</div>
  </div>
  <div class="status-bar" id="status"></div>

  <script>
    const vscode = acquireVsCodeApi();

    document.getElementById('export-json').addEventListener('click', () => {
      vscode.postMessage({ type: 'export', format: 'json' });
    });

    document.getElementById('export-csv').addEventListener('click', () => {
      vscode.postMessage({ type: 'export', format: 'csv' });
    });

    window.addEventListener('message', (event) => {
      const msg = event.data;
      const content = document.getElementById('content');
      const info = document.getElementById('info');
      const status = document.getElementById('status');

      if (msg.type === 'loading') {
        const preview = msg.query ? msg.query.substring(0, 80) : '';
        info.innerHTML = '<span class="query-preview">' + escapeHtml(preview) + '</span>';
        content.innerHTML = '<div class="loading"><div class="spinner"></div>Executing query...</div>';
        status.textContent = '';
        return;
      }

      if (msg.type === 'error') {
        content.innerHTML = '<div class="error">' + escapeHtml(msg.message) + '</div>';
        status.textContent = msg.elapsedMs != null ? 'Failed in ' + msg.elapsedMs + 'ms' : '';
        return;
      }

      if (msg.type === 'results') {
        const { columns, rows, commandTag, rowCount, elapsedMs, query } = msg;

        const preview = query ? query.substring(0, 80) : '';
        info.innerHTML = '<span class="query-preview">' + escapeHtml(preview) + '</span>';

        if (!columns || columns.length === 0) {
          content.innerHTML = '<div class="empty">' + escapeHtml(commandTag || 'OK') + '</div>';
          status.textContent = rowCount + ' row(s) affected \\u00B7 ' + elapsedMs + 'ms';
          return;
        }

        let html = '<div class="table-wrapper"><table><thead><tr>';
        for (const col of columns) {
          html += '<th>' + escapeHtml(col.name) + '</th>';
        }
        html += '</tr></thead><tbody>';

        for (const row of rows) {
          html += '<tr>';
          for (const val of row) {
            if (val === null || val === undefined) {
              html += '<td class="null">NULL</td>';
            } else {
              html += '<td>' + escapeHtml(String(val)) + '</td>';
            }
          }
          html += '</tr>';
        }

        html += '</tbody></table></div>';
        content.innerHTML = html;
        status.textContent = rowCount + ' row(s) \\u00B7 ' + commandTag + ' \\u00B7 ' + elapsedMs + 'ms';
      }
    });

    function escapeHtml(text) {
      const div = document.createElement('div');
      div.textContent = text;
      return div.innerHTML;
    }
  </script>
</body>
</html>`;
  }

  dispose(): void {
    this.panel?.dispose();
  }
}
