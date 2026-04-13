import fs from 'node:fs'
import path from 'node:path'

const rootDir = process.cwd()
const ignoredDirs = new Set([
  '.git',
  '.codex-logs',
  '.claude',
  'dist',
  'node_modules',
  'src-tauri/target',
])

const scanExtensions = new Set([
  '.css',
  '.html',
  '.js',
  '.json',
  '.md',
  '.mjs',
  '.ps1',
  '.rs',
  '.toml',
  '.ts',
  '.tsx',
  '.yaml',
  '.yml',
])

const patchArtifactPattern = /\.(orig|rej)$/
const conflictMarkerPattern = /^(<<<<<<< |=======$|>>>>>>> )/

const failures = []

function shouldIgnore(relativePath) {
  return [...ignoredDirs].some(
    (ignoredDir) =>
      relativePath === ignoredDir || relativePath.startsWith(`${ignoredDir}${path.sep}`),
  )
}

function walk(currentDir) {
  const entries = fs.readdirSync(currentDir, { withFileTypes: true })

  for (const entry of entries) {
    const absolutePath = path.join(currentDir, entry.name)
    const relativePath = path.relative(rootDir, absolutePath)

    if (shouldIgnore(relativePath)) {
      continue
    }

    if (entry.isDirectory()) {
      walk(absolutePath)
      continue
    }

    if (patchArtifactPattern.test(entry.name)) {
      failures.push(`patch-artifact: ${relativePath}`)
      continue
    }

    if (!scanExtensions.has(path.extname(entry.name))) {
      continue
    }

    const content = fs.readFileSync(absolutePath, 'utf8')
    const lines = content.split(/\r?\n/)

    for (let index = 0; index < lines.length; index += 1) {
      if (conflictMarkerPattern.test(lines[index])) {
        failures.push(`merge-marker: ${relativePath}:${index + 1}`)
      }
    }
  }
}

walk(rootDir)

if (failures.length > 0) {
  console.error('Repository health check failed:')
  for (const failure of failures) {
    console.error(`- ${failure}`)
  }
  process.exit(1)
}

console.log('Repository health check passed.')
