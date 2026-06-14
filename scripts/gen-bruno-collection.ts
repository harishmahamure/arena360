#!/usr/bin/env tsx
/**
 * Generates a Bruno API collection from apps/backend/docs/openapi.json.
 * Output: bruno/gaming-cafe-api/ (open as Collection in Bruno)
 */
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = process.cwd();
const SPEC_PATH = join(ROOT, 'apps/backend/docs/openapi.json');
const OUT_DIR = join(ROOT, 'bruno/gaming-cafe-api');

type JsonSchema = {
  $ref?: string;
  type?: string | string[];
  properties?: Record<string, JsonSchema>;
  required?: string[];
  items?: JsonSchema;
  enum?: unknown[];
  format?: string;
  example?: unknown;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  allOf?: JsonSchema[];
};

type OpenAPIParameter = {
  name: string;
  in: 'path' | 'query' | 'header' | 'cookie';
  required?: boolean;
  schema?: JsonSchema;
  example?: unknown;
};

type OpenAPIOperation = {
  tags?: string[];
  operationId?: string;
  summary?: string;
  description?: string;
  parameters?: OpenAPIParameter[];
  requestBody?: {
    required?: boolean;
    content?: Record<string, { schema?: JsonSchema }>;
  };
  security?: Record<string, unknown>[];
};

type OpenAPISpec = {
  info: { title: string; version: string; description?: string };
  servers?: { url: string; description?: string }[];
  paths: Record<string, Partial<Record<string, OpenAPIOperation>>>;
  components?: { schemas?: Record<string, JsonSchema> };
};

const HTTP_METHODS = ['get', 'post', 'put', 'patch', 'delete', 'head', 'options'] as const;

/** Bruno reserves "auth" — conflicts with collection.bru `auth {}` block. */
const BRUNO_FOLDER_NAMES: Record<string, string> = {
  auth: 'authentication',
};

const AUTH_OPERATION_ORDER = ['login_admin', 'register'] as const;

const OPERATION_DISPLAY_NAMES: Record<string, string> = {
  login_admin: 'Login Admin',
  register: 'Register User',
};

function loadSpec(): OpenAPISpec {
  if (!existsSync(SPEC_PATH)) {
    throw new Error(`OpenAPI spec not found at ${SPEC_PATH}. Run pnpm gen:api-types first.`);
  }
  return JSON.parse(readFileSync(SPEC_PATH, 'utf8')) as OpenAPISpec;
}

function resolveRef(ref: string, schemas: Record<string, JsonSchema>): JsonSchema {
  const prefix = '#/components/schemas/';
  if (!ref.startsWith(prefix)) {
    throw new Error(`Unsupported schema ref: ${ref}`);
  }
  const name = ref.slice(prefix.length);
  const schema = schemas[name];
  if (!schema) {
    throw new Error(`Schema not found: ${name}`);
  }
  return schema;
}

function primaryType(schema: JsonSchema): string | undefined {
  if (!schema.type) return undefined;
  if (Array.isArray(schema.type)) {
    return schema.type.find((t) => t !== 'null');
  }
  return schema.type;
}

function exampleValue(schema: JsonSchema, schemas: Record<string, JsonSchema>, depth = 0): unknown {
  if (depth > 10) return null;
  if (schema.$ref) {
    return exampleValue(resolveRef(schema.$ref, schemas), schemas, depth + 1);
  }
  const composite = schema.oneOf ?? schema.anyOf;
  if (composite?.[0]) {
    return exampleValue(composite[0], schemas, depth + 1);
  }
  if (schema.allOf?.length) {
    return Object.assign(
      {},
      ...schema.allOf.map((part) => exampleValue(part, schemas, depth + 1) as object),
    );
  }
  if (schema.example !== undefined) return schema.example;

  const type = primaryType(schema);
  switch (type) {
    case 'string':
      if (schema.format === 'uuid') return '00000000-0000-4000-8000-000000000001';
      if (schema.format === 'date-time') return '2026-01-01T00:00:00.000Z';
      if (schema.enum?.[0] !== undefined) return schema.enum[0];
      return 'string';
    case 'integer':
    case 'number':
      return 1;
    case 'boolean':
      return true;
    case 'array':
      return [exampleValue(schema.items ?? {}, schemas, depth + 1)];
    case 'object': {
      const obj: Record<string, unknown> = {};
      const required = new Set(schema.required ?? []);
      for (const [key, prop] of Object.entries(schema.properties ?? {})) {
        if (required.has(key)) {
          obj[key] = exampleValue(prop, schemas, depth + 1);
        }
      }
      return obj;
    }
    default:
      return null;
  }
}

function requestBodyExample(
  operation: OpenAPIOperation,
  schemas: Record<string, JsonSchema>,
): string | undefined {
  const jsonSchema = operation.requestBody?.content?.['application/json']?.schema;
  if (!jsonSchema) return undefined;
  const example = exampleValue(jsonSchema, schemas);
  return JSON.stringify(example, null, 2);
}

function needsBearerAuth(operation: OpenAPIOperation): boolean {
  if (operation.security === undefined) return false;
  if (operation.security.length === 0) return false;
  return operation.security.some((entry) => 'bearer_auth' in entry);
}

function toBrunoUrl(path: string): string {
  return `{{baseUrl}}${path.replace(/\{([^}]+)\}/g, '{{$1}}')}`;
}

function toFolderName(tag: string): string {
  return BRUNO_FOLDER_NAMES[tag] ?? tag;
}

function toFolderLabel(tag: string): string {
  if (tag === 'auth') return 'Authentication';
  return tag
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

function toDisplayName(operationId: string, fallback: string): string {
  return OPERATION_DISPLAY_NAMES[operationId] ?? fallback;
}

function sortRoutes(tag: string, routes: { operation: OpenAPIOperation }[]): void {
  if (tag !== 'auth') return;
  routes.sort((a, b) => {
    const idA = a.operation.operationId ?? '';
    const idB = b.operation.operationId ?? '';
    const idxA = AUTH_OPERATION_ORDER.indexOf(idA as (typeof AUTH_OPERATION_ORDER)[number]);
    const idxB = AUTH_OPERATION_ORDER.indexOf(idB as (typeof AUTH_OPERATION_ORDER)[number]);
    const orderA = idxA === -1 ? 99 : idxA;
    const orderB = idxB === -1 ? 99 : idxB;
    return orderA - orderB || idA.localeCompare(idB);
  });
}

function sanitizeFilename(operationId: string, method: string): string {
  const base = operationId.replace(/[^a-zA-Z0-9_-]+/g, '-').replace(/^-+|-+$/g, '');
  return `${method}-${base || 'request'}.bru`;
}

function buildQueryBlock(params: OpenAPIParameter[]): string | undefined {
  const queryParams = params.filter((p) => p.in === 'query');
  if (queryParams.length === 0) return undefined;
  const lines = queryParams.map((p) => {
    const example = p.example ?? exampleValue(p.schema ?? {}, {});
    const value =
      typeof example === 'string' && p.name.includes('Date') ? '2026-01-01' : String(example ?? '');
    return `  ${p.name}: ${value}`;
  });
  return `query {\n${lines.join('\n')}\n}`;
}

function authTests(operationId: string): string | undefined {
  if (operationId === 'login_admin') {
    return `tests {
  test("should save accessToken from admin login response", function() {
    const body = res.getBody();
    const token = body?.data?.accessToken;
    if (token) {
      bru.setEnvVar("accessToken", token);
    }
  });
}`;
  }
  return undefined;
}

function buildBruRequest(options: {
  name: string;
  method: string;
  url: string;
  seq: number;
  body?: string;
  queryBlock?: string;
  bearerAuth: boolean;
  operationId: string;
  description?: string;
}): string {
  const method = options.method.toLowerCase();
  const hasBody = ['post', 'put', 'patch'].includes(method) && options.body;
  const lines: string[] = [];

  lines.push('meta {');
  lines.push(`  name: ${options.name}`);
  lines.push('  type: http');
  lines.push(`  seq: ${options.seq}`);
  lines.push('}');
  lines.push('');

  lines.push(`${method} {`);
  lines.push(`  url: ${options.url}`);
  lines.push(hasBody ? '  body: json' : '  body: none');
  lines.push(options.bearerAuth ? '  auth: inherit' : '  auth: none');
  lines.push('}');
  lines.push('');

  if (options.queryBlock) {
    lines.push(options.queryBlock);
    lines.push('');
  }

  if (hasBody && options.body) {
    lines.push('headers {');
    lines.push('  Content-Type: application/json');
    lines.push('}');
    lines.push('');
    lines.push('body:json {');
    lines.push(options.body);
    lines.push('}');
    lines.push('');
  }

  const tests = authTests(options.operationId);
  if (tests) {
    lines.push(tests);
    lines.push('');
  }

  if (options.description) {
    lines.push('docs {');
    lines.push(`  ${options.description.replace(/\n/g, ' ')}`);
    lines.push('}');
  }

  return `${lines.join('\n').trimEnd()}\n`;
}

function writeCollection(spec: OpenAPISpec): void {
  const schemas = spec.components?.schemas ?? {};
  const baseUrl = spec.servers?.[0]?.url ?? 'http://localhost:3000';

  if (existsSync(OUT_DIR)) {
    rmSync(OUT_DIR, { recursive: true, force: true });
  }
  mkdirSync(join(OUT_DIR, 'environments'), { recursive: true });

  writeFileSync(
    join(OUT_DIR, 'bruno.json'),
    `${JSON.stringify(
      {
        version: '1',
        name: spec.info.title,
        type: 'collection',
        ignore: ['node_modules', '.git'],
      },
      null,
      2,
    )}\n`,
  );

  writeFileSync(
    join(OUT_DIR, 'collection.bru'),
    `meta {
  name: ${spec.info.title}
}

auth {
  mode: bearer
}

auth:bearer {
  token: {{accessToken}}
}

vars:pre-request {
  baseUrl: ${baseUrl}
}
`,
  );

  writeFileSync(
    join(OUT_DIR, 'environments', 'Local.bru'),
    `vars {
  baseUrl: ${baseUrl}
  accessToken:
  username: admin
  password: your-password
  totp:
}
`,
  );

  writeFileSync(
    join(OUT_DIR, 'README.md'),
    `# ${spec.info.title} — Bruno Collection

Generated from \`apps/backend/docs/openapi.json\`. Regenerate:

\`\`\`bash
pnpm gen:bruno
\`\`\`

## Import

1. Open Bruno → **Open Collection**
2. Select this folder (\`bruno/gaming-cafe-api\`)

## Auth flow

1. Select **Local** environment
2. Run **authentication → Login Admin** (saves \`accessToken\`)
3. Secured routes inherit Bearer auth from collection settings

Edit credentials in **environments/Local.bru**.
`,
  );

  const routesByTag = new Map<
    string,
    {
      method: string;
      path: string;
      operation: OpenAPIOperation;
    }[]
  >();

  for (const [path, pathItem] of Object.entries(spec.paths)) {
    for (const method of HTTP_METHODS) {
      const operation = pathItem[method];
      if (!operation) continue;
      const tag = operation.tags?.[0] ?? 'default';
      const bucket = routesByTag.get(tag) ?? [];
      bucket.push({ method, path, operation });
      routesByTag.set(tag, bucket);
    }
  }

  const tagOrder = (a: string, b: string): number => {
    if (a === 'auth') return -1;
    if (b === 'auth') return 1;
    return a.localeCompare(b);
  };

  let folderSeq = 1;
  for (const [tag, routes] of [...routesByTag.entries()].sort(([a], [b]) => tagOrder(a, b))) {
    const folderName = toFolderName(tag);
    const folderDir = join(OUT_DIR, folderName);
    mkdirSync(folderDir, { recursive: true });

    writeFileSync(
      join(folderDir, 'folder.bru'),
      `meta {
  name: ${toFolderLabel(tag)}
  seq: ${folderSeq}
}
`,
    );
    folderSeq += 1;

    sortRoutes(tag, routes);
    if (tag !== 'auth') {
      routes.sort((a, b) => {
        const idA = a.operation.operationId ?? '';
        const idB = b.operation.operationId ?? '';
        return idA.localeCompare(idB) || a.path.localeCompare(b.path);
      });
    }

    let reqSeq = 1;
    for (const { method, path, operation } of routes) {
      const operationId = operation.operationId ?? `${method}_${path}`;
      const name = toDisplayName(operationId, operation.summary ?? operationId);
      const body = requestBodyExample(operation, schemas);

      // Wire auth login/verify env vars into example bodies
      let bodyText = body;
      if (operationId === 'login_admin' && bodyText) {
        bodyText = bodyText
          .replace('"username": "string"', '"username": "{{username}}"')
          .replace('"password": "string"', '"password": "{{password}}"')
          .replace('"totp": "string"', '"totp": "{{totp}}"');
      }

      const bru = buildBruRequest({
        name,
        method: method.toUpperCase(),
        url: toBrunoUrl(path),
        seq: reqSeq,
        body: bodyText,
        queryBlock: buildQueryBlock(operation.parameters ?? []),
        bearerAuth: needsBearerAuth(operation),
        operationId,
        description: operation.description,
      });

      writeFileSync(join(folderDir, sanitizeFilename(operationId, method)), bru);
      reqSeq += 1;
    }
  }

  const totalRoutes = [...routesByTag.values()].reduce((n, r) => n + r.length, 0);
  process.stdout.write(
    `Bruno collection written to ${OUT_DIR} (${totalRoutes} requests, ${routesByTag.size} folders)\n`,
  );
}

writeCollection(loadSpec());
