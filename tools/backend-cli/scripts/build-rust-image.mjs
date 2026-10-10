#!/usr/bin/env node
// A small allowlisted tar context also supports Docker installations without
// Buildx/per-Dockerfile ignore support. Frontend .dockerignore stays independent.
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../../', import.meta.url));
const args = process.argv.slice(2);
let target;
let tag;
let buildBase;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--target') target = args[++i];
  else if (args[i] === '--tag') tag = args[++i];
  else if (args[i] === '--build-base') buildBase = args[++i];
  else throw new Error(`Unknown argument: ${args[i]}`);
}
if (!['gateway', 'storage', 'backend', 'builder'].includes(target) || !tag || tag.startsWith('-')) {
  throw new Error(
    'Usage: pnpm rust:image --target gateway|storage|backend|builder --tag REGISTRY/IMAGE:VERSION [--build-base BUILDER_IMAGE]',
  );
}
const archive = spawn(
  'tar',
  [
    '-c',
    '--format=ustar',
    '--no-xattrs',
    '--exclude=target',
    '--exclude=node_modules',
    '--exclude=.env',
    '--exclude=.env.*',
    '--exclude=data',
    '--exclude=*.pem',
    '--exclude=*.p12',
    '--exclude=*.pfx',
    '--exclude=*.log',
    '-f',
    '-',
    'Cargo.toml',
    'Cargo.lock',
    'crates',
    'tools/backend-cli',
    'apps/backend',
    'apps/tenant-gateway',
    'apps/tenant-storage',
    'infra/docker/rust.Dockerfile',
  ],
  {
    cwd: root,
    env: { ...process.env, COPYFILE_DISABLE: '1', LC_ALL: 'C' },
    stdio: ['ignore', 'pipe', 'inherit'],
  },
);
const docker = spawn(
  'docker',
  [
    'build',
    '-f',
    'infra/docker/rust.Dockerfile',
    '--target',
    target,
    '-t',
    tag,
    ...(buildBase ? ['--build-arg', `RUST_BUILD_BASE=${buildBase}`] : []),
    '-',
  ],
  {
    cwd: root,
    stdio: ['pipe', 'inherit', 'inherit'],
  },
);
archive.stdout.pipe(docker.stdin);
// Docker can reject the build before it has read the complete archive.
docker.stdin.on('error', (error) => {
  if (error.code !== 'EPIPE') {
    console.error(error);
  }
});
// A rejected build can stop reading before tar finishes. Unblock the producer
// so the helper exits instead of leaving a paused archive process behind.
docker.once('exit', () => {
  archive.stdout.destroy();
  archive.kill('SIGTERM');
});
docker.once('error', () => {
  archive.stdout.destroy();
  archive.kill('SIGTERM');
});
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.on(signal, () => {
    archive.kill(signal);
    docker.kill(signal);
  });
}
const completed = (child) =>
  new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('exit', (code) => resolve(code ?? 1));
  });
const [archiveCode, dockerCode] = await Promise.all([completed(archive), completed(docker)]);
process.exitCode = archiveCode || dockerCode;
