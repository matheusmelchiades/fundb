import {
  connect as fundbConnect,
  FunDBConnection,
  ConnectionError,
  QueryError,
} from '@fundb/client';
import type {
  ConnectOptions,
  Column as FunDBColumn,
  QueryResult as FunDBQueryResult,
} from '@fundb/client';

export interface ConnectionProfile {
  name: string;
  host: string;
  port: number;
  database: string;
  user: string;
  password: string;
  ssl: boolean;
}

export interface Column {
  name: string;
  typeOid: number;
}

export interface QueryResult {
  columns: Column[];
  rows: (string | null)[][];
  commandTag: string;
  rowCount: number;
}

export class FunDBClient {
  private connection: FunDBConnection | null = null;
  private _connected = false;

  constructor(private profile: ConnectionProfile) {}

  async connect(): Promise<void> {
    const options: ConnectOptions = {
      host: this.profile.host || 'localhost',
      port: this.profile.port || 5433,
      database: this.profile.database || 'fundb',
      user: this.profile.user || 'fundb',
      password: this.profile.password || undefined,
      ssl: this.profile.ssl || false,
    };

    try {
      this.connection = await fundbConnect(options);
      this._connected = true;
    } catch (err) {
      this._connected = false;
      if (err instanceof ConnectionError) {
        throw new Error(`Failed to connect to FunDB: ${err.message}`);
      }
      throw new Error(`Failed to connect to FunDB: ${err}`);
    }
  }

  async execute(query: string): Promise<QueryResult> {
    if (!this.connection || !this._connected) {
      throw new Error('Not connected to FunDB');
    }

    try {
      const result: FunDBQueryResult = await this.connection.execute(query);

      const columns: Column[] = result.columns.map((c: FunDBColumn) => ({
        name: c.name,
        typeOid: c.typeOid,
      }));

      const rows: (string | null)[][] = result.rows.map((row: unknown[]) =>
        row.map((v) => (v === null || v === undefined ? null : String(v))),
      );

      return {
        columns,
        rows,
        commandTag: result.commandTag,
        rowCount: result.rowCount,
      };
    } catch (err) {
      if (err instanceof QueryError) {
        throw new Error(`Query failed: ${err.message}`);
      }
      throw new Error(`Query failed: ${err}`);
    }
  }

  async disconnect(): Promise<void> {
    if (this.connection && this._connected) {
      try {
        await this.connection.close();
      } finally {
        this.connection = null;
        this._connected = false;
      }
    }
  }

  get connected(): boolean {
    return this._connected;
  }

  get profileName(): string {
    return this.profile.name;
  }

  get displayLabel(): string {
    return `${this.profile.name} (fundb://${this.profile.user}@${this.profile.host}:${this.profile.port}/${this.profile.database})`;
  }
}
