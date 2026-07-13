import { spawnSync } from 'node:child_process'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), '..')
const manifestPath = join(repositoryRoot, 'rust-core', 'Cargo.toml')
const separatorIndex = process.argv.indexOf('--', 2)
const cargoArgs = process.argv.slice(2)
const manifestArgs = ['--manifest-path', manifestPath]

if (separatorIndex >= 0) {
  const localSeparatorIndex = separatorIndex - 2
  cargoArgs.splice(localSeparatorIndex, 0, ...manifestArgs)
} else {
  cargoArgs.push(...manifestArgs)
}

const environment = { ...process.env }
if (process.platform === 'darwin') {
  environment.MACOSX_DEPLOYMENT_TARGET ??= '12.0'
}

const result = spawnSync(environment.CARGO ?? 'cargo', cargoArgs, {
  cwd: repositoryRoot,
  env: environment,
  stdio: 'inherit',
})

if (result.error) {
  console.error(`无法启动 Cargo: ${result.error.message}`)
  process.exit(1)
}

process.exit(result.status ?? 1)
