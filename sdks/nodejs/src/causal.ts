import { CausalEdge, CausalPath as ICausalPath } from './types';
import { CausalError } from './errors';

export class CausalPath implements ICausalPath {
  readonly nodes: string[];
  readonly edges: CausalEdge[];
  readonly totalStrength: number;
  readonly minStability: number;

  constructor(
    nodes: string[],
    edges: CausalEdge[],
    totalStrength: number,
    minStability: number,
  ) {
    this.nodes = nodes;
    this.edges = edges;
    this.totalStrength = totalStrength;
    this.minStability = minStability;
  }

  toMermaid(): string {
    const lines: string[] = ['flowchart LR'];
    for (const edge of this.edges) {
      const src = edge.sourceId.replace(/-/g, '_');
      const tgt = edge.targetId.replace(/-/g, '_');
      const label = `${edge.causalType} (${edge.strength.toFixed(2)})`;
      lines.push(`    ${src} -->|"${label}"| ${tgt}`);
    }
    return lines.join('\n');
  }

  toJson(): string {
    return JSON.stringify(
      {
        nodes: this.nodes,
        edges: this.edges,
        totalStrength: this.totalStrength,
        minStability: this.minStability,
      },
      null,
      2,
    );
  }

  static fromRaw(data: unknown): CausalPath {
    if (typeof data !== 'object' || data === null) {
      throw new CausalError('Invalid causal path data: expected an object');
    }

    const raw = data as Record<string, unknown>;

    const nodes = parseStringArray(raw['nodes'] ?? raw['nodes'], 'nodes');
    const edges = parseCausalEdges(raw['edges'] ?? raw['edges']);
    const totalStrength = parseNumber(
      raw['totalStrength'] ?? raw['total_strength'],
      'totalStrength',
    );
    const minStability = parseNumber(
      raw['minStability'] ?? raw['min_stability'],
      'minStability',
    );

    return new CausalPath(nodes, edges, totalStrength, minStability);
  }
}

function parseStringArray(value: unknown, field: string): string[] {
  if (!Array.isArray(value)) {
    throw new CausalError(`Invalid causal path data: '${field}' must be an array`);
  }
  return value.map((item, idx) => {
    if (typeof item !== 'string') {
      throw new CausalError(
        `Invalid causal path data: '${field}[${idx}]' must be a string`,
      );
    }
    return item;
  });
}

function parseNumber(value: unknown, field: string): number {
  if (typeof value !== 'number') {
    throw new CausalError(
      `Invalid causal path data: '${field}' must be a number, got ${typeof value}`,
    );
  }
  return value;
}

function parseCausalEdges(value: unknown): CausalEdge[] {
  if (!Array.isArray(value)) {
    throw new CausalError("Invalid causal path data: 'edges' must be an array");
  }
  return value.map((item, idx) => parseCausalEdge(item, idx));
}

function parseCausalEdge(item: unknown, idx: number): CausalEdge {
  if (typeof item !== 'object' || item === null) {
    throw new CausalError(`Invalid causal path data: 'edges[${idx}]' must be an object`);
  }

  const raw = item as Record<string, unknown>;

  const sourceId =
    typeof raw['sourceId'] === 'string'
      ? raw['sourceId']
      : typeof raw['source_id'] === 'string'
        ? raw['source_id']
        : null;

  const targetId =
    typeof raw['targetId'] === 'string'
      ? raw['targetId']
      : typeof raw['target_id'] === 'string'
        ? raw['target_id']
        : null;

  const causalType =
    typeof raw['causalType'] === 'string'
      ? raw['causalType']
      : typeof raw['causal_type'] === 'string'
        ? raw['causal_type']
        : null;

  const strength =
    typeof raw['strength'] === 'number' ? raw['strength'] : null;

  if (sourceId === null) {
    throw new CausalError(`Invalid causal path data: 'edges[${idx}].sourceId' must be a string`);
  }
  if (targetId === null) {
    throw new CausalError(`Invalid causal path data: 'edges[${idx}].targetId' must be a string`);
  }
  if (causalType === null) {
    throw new CausalError(`Invalid causal path data: 'edges[${idx}].causalType' must be a string`);
  }
  if (strength === null) {
    throw new CausalError(`Invalid causal path data: 'edges[${idx}].strength' must be a number`);
  }

  const edge: CausalEdge = { sourceId, targetId, causalType, strength };

  const stabilityScore =
    typeof raw['stabilityScore'] === 'number'
      ? raw['stabilityScore']
      : typeof raw['stability_score'] === 'number'
        ? raw['stability_score']
        : undefined;

  if (stabilityScore !== undefined) {
    edge.stabilityScore = stabilityScore;
  }

  return edge;
}
