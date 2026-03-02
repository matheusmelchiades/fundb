import { Column } from '../connection/fundbClient';

function escapeCsv(value: string): string {
  if (value.includes(',') || value.includes('"') || value.includes('\n')) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

export function toJson(columns: Column[], rows: (string | null)[][]): string {
  const dicts = rows.map((row) => {
    const obj: Record<string, string | null> = {};
    columns.forEach((col, i) => {
      obj[col.name] = row[i];
    });
    return obj;
  });
  return JSON.stringify(dicts, null, 2);
}

export function toCsv(columns: Column[], rows: (string | null)[][]): string {
  const header = columns.map((c) => escapeCsv(c.name)).join(',');
  const body = rows.map((row) =>
    row.map((v) => (v === null ? '' : escapeCsv(v))).join(','),
  );
  return [header, ...body].join('\n');
}
