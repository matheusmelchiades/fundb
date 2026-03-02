import * as vscode from 'vscode';
import { FunDBClient, ConnectionProfile, QueryResult } from './fundbClient';
import { log, logError } from '../util/logger';

export class ConnectionManager {
  private activeClient: FunDBClient | null = null;
  private activeProfileName: string | null = null;

  private readonly _onDidConnect = new vscode.EventEmitter<ConnectionProfile>();
  readonly onDidConnect = this._onDidConnect.event;

  private readonly _onDidDisconnect = new vscode.EventEmitter<void>();
  readonly onDidDisconnect = this._onDidDisconnect.event;

  private readonly _onDidChangeProfiles = new vscode.EventEmitter<void>();
  readonly onDidChangeProfiles = this._onDidChangeProfiles.event;

  getProfiles(): ConnectionProfile[] {
    const config = vscode.workspace.getConfiguration('fundb');
    const raw = config.get<Partial<ConnectionProfile>[]>('connections', []);
    return raw.map((p) => ({
      name: p.name || 'default',
      host: p.host || 'localhost',
      port: p.port || 5433,
      database: p.database || 'fundb',
      user: p.user || 'fundb',
      password: p.password || '',
      ssl: p.ssl || false,
    }));
  }

  private async saveProfiles(profiles: ConnectionProfile[]): Promise<void> {
    const config = vscode.workspace.getConfiguration('fundb');
    await config.update('connections', profiles, vscode.ConfigurationTarget.Global);
    this._onDidChangeProfiles.fire();
  }

  // ── Connection Wizard ──────────────────────────────────────────────────

  async newConnection(): Promise<void> {
    const profile = await this.promptConnectionForm();
    if (!profile) return;

    const profiles = this.getProfiles();
    const existing = profiles.findIndex((p) => p.name === profile.name);
    if (existing >= 0) {
      const overwrite = await vscode.window.showWarningMessage(
        `Profile "${profile.name}" already exists. Overwrite?`,
        'Overwrite',
        'Cancel',
      );
      if (overwrite !== 'Overwrite') return;
      profiles[existing] = profile;
    } else {
      profiles.push(profile);
    }

    await this.saveProfiles(profiles);
    log(`Connection profile saved: ${profile.name} (${profile.host}:${profile.port})`);
    vscode.window.showInformationMessage(`Connection "${profile.name}" saved.`);

    const connectNow = await vscode.window.showInformationMessage(
      `Connect to "${profile.name}" now?`,
      'Connect',
      'Later',
    );
    if (connectNow === 'Connect') {
      await this.connect(profile.name);
    }
  }

  async editConnection(profileName?: string): Promise<void> {
    const profiles = this.getProfiles();

    if (!profileName) {
      if (profiles.length === 0) {
        vscode.window.showInformationMessage('No connections to edit.');
        return;
      }
      const picked = await vscode.window.showQuickPick(
        profiles.map((p) => ({ label: p.name, description: `${p.host}:${p.port}` })),
        { placeHolder: 'Select connection to edit' },
      );
      if (!picked) return;
      profileName = picked.label;
    }

    const idx = profiles.findIndex((p) => p.name === profileName);
    if (idx < 0) {
      vscode.window.showErrorMessage(`Profile "${profileName}" not found.`);
      return;
    }

    const updated = await this.promptConnectionForm(profiles[idx]);
    if (!updated) return;

    // If connected to this profile, disconnect first
    if (this.activeProfileName === profileName) {
      await this.disconnect();
    }

    profiles[idx] = updated;
    await this.saveProfiles(profiles);
    vscode.window.showInformationMessage(`Connection "${updated.name}" updated.`);
  }

  async removeConnection(profileName?: string): Promise<void> {
    const profiles = this.getProfiles();

    if (!profileName) {
      if (profiles.length === 0) {
        vscode.window.showInformationMessage('No connections to remove.');
        return;
      }
      const picked = await vscode.window.showQuickPick(
        profiles.map((p) => ({ label: p.name, description: `${p.host}:${p.port}` })),
        { placeHolder: 'Select connection to remove' },
      );
      if (!picked) return;
      profileName = picked.label;
    }

    const confirm = await vscode.window.showWarningMessage(
      `Remove connection "${profileName}"?`,
      { modal: true },
      'Remove',
    );
    if (confirm !== 'Remove') return;

    if (this.activeProfileName === profileName) {
      await this.disconnect();
    }

    const filtered = profiles.filter((p) => p.name !== profileName);
    await this.saveProfiles(filtered);
    vscode.window.showInformationMessage(`Connection "${profileName}" removed.`);
  }

  // ── Connect / Disconnect ───────────────────────────────────────────────

  async connect(profileName?: string): Promise<void> {
    const profiles = this.getProfiles();

    if (profiles.length === 0) {
      const action = await vscode.window.showInformationMessage(
        'No FunDB connections configured.',
        'New Connection',
      );
      if (action === 'New Connection') {
        await this.newConnection();
      }
      return;
    }

    let profile: ConnectionProfile;

    if (profileName) {
      const found = profiles.find((p) => p.name === profileName);
      if (!found) {
        vscode.window.showErrorMessage(`Connection "${profileName}" not found.`);
        return;
      }
      profile = found;
    } else if (profiles.length === 1) {
      profile = profiles[0];
    } else {
      const picked = await vscode.window.showQuickPick(
        profiles.map((p) => ({
          label: p.name,
          description: `${p.host}:${p.port}/${p.database}`,
          profile: p,
        })),
        { placeHolder: 'Select a FunDB connection' },
      );
      if (!picked) return;
      profile = picked.profile;
    }

    if (this.activeClient) {
      await this.disconnect();
    }

    const client = new FunDBClient(profile);

    try {
      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: `Connecting to FunDB (${profile.name})...`,
          cancellable: false,
        },
        async () => {
          await client.connect();
        },
      );

      this.activeClient = client;
      this.activeProfileName = profile.name;
      this._onDidConnect.fire(profile);
      log(`Connected to ${client.displayLabel}`);
      vscode.window.showInformationMessage(`Connected to FunDB: ${client.displayLabel}`);
    } catch (err) {
      logError(`Connection failed to ${profile.host}:${profile.port}`, err);
      vscode.window.showErrorMessage(`${err}`);
    }
  }

  async disconnect(): Promise<void> {
    if (this.activeClient) {
      const label = this.activeClient.displayLabel;
      await this.activeClient.disconnect();
      this.activeClient = null;
      this.activeProfileName = null;
      this._onDidDisconnect.fire();
      log(`Disconnected from ${label}`);
    }
  }

  async execute(query: string): Promise<QueryResult> {
    if (!this.activeClient) {
      throw new Error('Not connected to FunDB');
    }
    return this.activeClient.execute(query);
  }

  get isConnected(): boolean {
    return this.activeClient?.connected ?? false;
  }

  get connectedProfileName(): string | null {
    return this.activeProfileName;
  }

  get displayLabel(): string {
    return this.activeClient?.displayLabel ?? '';
  }

  dispose(): void {
    this.activeClient?.disconnect();
    this._onDidConnect.dispose();
    this._onDidDisconnect.dispose();
    this._onDidChangeProfiles.dispose();
  }

  // ── Input Form ─────────────────────────────────────────────────────────

  private async promptConnectionForm(
    defaults?: ConnectionProfile,
  ): Promise<ConnectionProfile | undefined> {
    const name = await vscode.window.showInputBox({
      title: 'Connection Name',
      prompt: 'A name for this connection profile',
      value: defaults?.name ?? '',
      placeHolder: 'e.g. local, staging, production',
      validateInput: (v) => (v.trim() ? undefined : 'Name is required'),
    });
    if (name === undefined) return;

    const host = await vscode.window.showInputBox({
      title: 'Host',
      prompt: 'FunDB server hostname',
      value: defaults?.host ?? 'localhost',
      placeHolder: 'localhost',
    });
    if (host === undefined) return;

    const portStr = await vscode.window.showInputBox({
      title: 'Port',
      prompt: 'FunDB server port',
      value: String(defaults?.port ?? 5433),
      placeHolder: '5433',
      validateInput: (v) => {
        const n = Number(v);
        return Number.isInteger(n) && n > 0 && n <= 65535
          ? undefined
          : 'Must be a valid port (1-65535)';
      },
    });
    if (portStr === undefined) return;

    const database = await vscode.window.showInputBox({
      title: 'Database',
      prompt: 'Database name',
      value: defaults?.database ?? 'fundb',
      placeHolder: 'fundb',
    });
    if (database === undefined) return;

    const user = await vscode.window.showInputBox({
      title: 'User',
      prompt: 'Username',
      value: defaults?.user ?? 'fundb',
      placeHolder: 'fundb',
    });
    if (user === undefined) return;

    const password = await vscode.window.showInputBox({
      title: 'Password',
      prompt: 'Password (leave empty if none)',
      value: defaults?.password ?? '',
      password: true,
    });
    if (password === undefined) return;

    const sslPick = await vscode.window.showQuickPick(
      [
        { label: 'No', description: 'Connect without SSL', value: false },
        { label: 'Yes', description: 'Use SSL/TLS', value: true },
      ],
      {
        title: 'SSL',
        placeHolder: 'Use SSL?',
      },
    );
    if (!sslPick) return;

    return {
      name: name.trim(),
      host: host || 'localhost',
      port: Number(portStr) || 5433,
      database: database || 'fundb',
      user: user || 'fundb',
      password: password || '',
      ssl: sslPick.value,
    };
  }
}
