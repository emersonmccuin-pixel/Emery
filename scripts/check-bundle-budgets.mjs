import fs from 'node:fs'
import path from 'node:path'

const assetDir = path.resolve(process.cwd(), 'dist/assets')

const budgets = [
  {
    label: 'entry-js',
    pattern: /^index-.*\.js$/,
    maxBytes: 50_000,
  },
  {
    label: 'main-css',
    pattern: /^index-.*\.css$/,
    maxBytes: 95_000,
  },
  {
    label: 'store-js',
    pattern: /^store-.*\.js$/,
    maxBytes: 63_000,
  },
  {
    label: 'vendor-js',
    pattern: /^vendor-.*\.js$/,
    maxBytes: 240_000,
  },
  {
    label: 'terminal-vendor-js',
    pattern: /^terminal-vendor-.*\.js$/,
    maxBytes: 360_000,
  },
  {
    label: 'virtualizer-vendor-js',
    pattern: /^virtualizer-vendor-.*\.js$/,
    maxBytes: 5_000,
  },
  {
    label: 'markdown-vendor-js',
    pattern: /^markdown-vendor-.*\.js$/,
    maxBytes: 200_000,
  },
  {
    // AppSettingsPanel carries more surface area than the other lazy panels
    // (Accounts + Defaults + Integrations + Vault + Diagnostics tabs). It is
    // budgeted separately so the shared lazy-panel ceiling stays tight.
    label: 'lazy-panel-js-app-settings',
    pattern: /^AppSettingsPanel-.*\.js$/,
    maxBytes: 21_000,
  },
  {
    label: 'lazy-panel-js',
    pattern:
      /^(WorkItemsPanel|HistoryPanel|CreateProjectModal|WorktreeWorkItemPanel|LiveTerminal|ConfigurationPanel|OverviewPanel)-.*\.js$/,
    maxBytes: 20_000,
    allowMany: true,
  },
]

function formatBytes(bytes) {
  if (bytes >= 1_000_000) {
    return `${(bytes / 1_000_000).toFixed(2)} MB`
  }

  if (bytes >= 1_000) {
    return `${(bytes / 1_000).toFixed(2)} kB`
  }

  return `${bytes} B`
}

if (!fs.existsSync(assetDir)) {
  console.error(`Bundle budget check failed: missing asset directory ${assetDir}`)
  process.exit(1)
}

const assets = fs
  .readdirSync(assetDir)
  .map((name) => ({
    name,
    size: fs.statSync(path.join(assetDir, name)).size,
  }))
  .sort((left, right) => right.size - left.size)

const failures = []

console.log('Bundle budgets:')

for (const budget of budgets) {
  const matches = assets.filter((asset) => budget.pattern.test(asset.name))

  if (matches.length === 0) {
    failures.push(`${budget.label}: missing matching asset`)
    continue
  }

  for (const asset of matches) {
    const status = asset.size <= budget.maxBytes ? 'ok' : 'over'
    console.log(
      `- ${budget.label}: ${asset.name} = ${formatBytes(asset.size)} / budget ${formatBytes(budget.maxBytes)} [${status}]`,
    )

    if (asset.size > budget.maxBytes) {
      failures.push(
        `${budget.label}: ${asset.name} is ${formatBytes(asset.size)}, exceeds ${formatBytes(budget.maxBytes)}`,
      )
    }

    if (!budget.allowMany) {
      break
    }
  }
}

const unexpectedLargeJs = assets.filter(
  (asset) =>
    asset.name.endsWith('.js') &&
    asset.size > 60_000 &&
    !budgets.some((budget) => budget.pattern.test(asset.name)),
)

for (const asset of unexpectedLargeJs) {
  failures.push(
    `unexpected-large-js: ${asset.name} is ${formatBytes(asset.size)} and is not covered by a budget rule`,
  )
}

if (failures.length > 0) {
  console.error('\nBundle budget failures:')
  for (const failure of failures) {
    console.error(`- ${failure}`)
  }
  process.exit(1)
}

console.log('\nBundle budgets passed.')
