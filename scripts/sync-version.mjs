import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const version = readFileSync(join(root, 'VERSION'), 'utf8').trim();
const parts = version.split('.').map((part) => Number(part));

if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0)) {
  throw new Error(`VERSION must be four numeric components, found "${version}"`);
}

const [major, minor, patch, build] = parts;
const semver = build === 0 ? `${major}.${minor}.${patch}` : `${major}.${minor}.${patch}+${build}`;

for (const packagePath of [
  join(root, 'apps/desktop/package.json'),
  join(root, 'apps/web/package.json'),
]) {
  const packageJson = JSON.parse(readFileSync(packagePath, 'utf8'));
  packageJson.version = semver;
  writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`);
}

const cargoPath = join(root, 'Cargo.toml');
const cargo = readFileSync(cargoPath, 'utf8');
const workspaceVersion = /(\[workspace\.package\][\s\S]*?version\s*=\s*)"[^"]+"/;
if (!workspaceVersion.test(cargo)) {
  throw new Error('could not update workspace version in Cargo.toml');
}
const updatedCargo = cargo.replace(
  workspaceVersion,
  `$1"${major}.${minor}.${patch}"`,
);
writeFileSync(cargoPath, updatedCargo);

console.log(`${version} -> desktop ${semver}, cargo ${major}.${minor}.${patch}`);
