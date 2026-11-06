// Temporary experiment (removed in the next commit): reads the launch records (.jsonl) that
// probe.mjs wrote and the kernel messages (.kernel.log) the workflow kept, and prints a Markdown
// summary for the job summary. Run it from ui/:
//   node e2e-experiment/report.mjs test-results/firefox-webgl/*
import { readFile } from 'node:fs/promises'
import { basename } from 'node:path'

// The probe of Firefox 157 on X11 (toolkit/xre/gfxtest/glxtest.cpp) logs these with MOZ_GFX_DEBUG=1.
const PROBE_START = 'GLX_TEST: childgltest start'
const PROBE_END = 'GLX_TEST: childgltest finished'
const EGL_DONE = 'GLX_TEST: x11_egltest finished'
const GLX_DONE = 'GLX_TEST: glxtest finished'
// What the browser prints when the probe fails (widget/gtk/GfxInfo.cpp): a critical note for a
// probe that hangs or cannot be read, and with gfx.logging.level 2 a warning for a probe that
// fails, and the probe's own errors as `glxtest:` warnings. The probe's own warnings (no DRM
// device, no libEGL) also print as `glxtest:` lines on healthy launches, so only its errors count.
const PROBE_FAILED = new RegExp(
  [
    'ManageChildProcess\\(glxtest\\)',
    'Failed to get GPU info from glxtest',
    'glxtest: (X error|Unable to open a connection|GLX extension missing|No visuals found)',
    'glxtest: (glGetString returned null|EGL test failed|libGL\\.so\\.1 missing|no glXGetProcAddress)',
  ].join('|'),
)
// Browser output lines worth showing for a launch that failed.
const DETAIL_LINE = /GLX_TEST|glxtest|\[GFX|webgl|egl|glx|mesa|libgl|xlib|x error|error|fail/i
// The most launches shown in full for each variant, and the most lines of each kind in one. A
// job summary holds at most 1 MiB; the job log holds the whole report.
const DETAIL_LIMIT = 5
const DETAIL_LINES = 40
const DETAIL_WIDTH = 200
// The runner's counters probe.mjs records for the start of each launch, with their labels.
const STALLS = [
  ['cpuStall', 'CPU stall ms'],
  ['ioStall', 'Disk stall ms'],
  ['memoryStall', 'Memory stall ms'],
  ['steal', 'Steal ms'],
  ['majorFaults', 'Major faults'],
]

/** Every record of every launch file. */
async function readRecords(paths) {
  const texts = await Promise.all(paths.map((path) => readFile(path, 'utf8')))
  return texts.flatMap((text) =>
    text
      .split('\n')
      .filter((line) => line !== '')
      .map((line) => JSON.parse(line)),
  )
}

/** The rows whose kernel log names a crash, a kill or a hung task, with those lines. */
async function readKernelLogs(paths) {
  const logs = await Promise.all(
    paths.map(async (path) => [basename(path, '.kernel.log'), await readFile(path, 'utf8')]),
  )
  return logs.filter(([, text]) => text.trim() !== '')
}

/** The context the probe got: EGL, GLX after EGL failed, or ? when it logged neither. */
function probePath(arrival) {
  if (arrival(EGL_DONE) !== undefined) {
    return 'EGL'
  }
  return arrival(GLX_DONE) === undefined ? '?' : 'GLX'
}

/** What one launch shows. */
function analyze(record) {
  const output = record.output ?? []
  const arrival = (text) => output.find((line) => line.text.includes(text))?.milliseconds
  const probeStart = arrival(PROBE_START)
  const probeEnd = arrival(PROBE_END)
  const [first, second] = record.pages ?? []
  return {
    record,
    launchFailed: !record.launched,
    errored: record.error !== undefined,
    webgl2Null: first?.webgl2.created === false,
    secondNull: second?.webgl2.created === false,
    probeFailed: output.some((line) => PROBE_FAILED.test(line.text)),
    probeMilliseconds:
      probeStart === undefined || probeEnd === undefined ? undefined : probeEnd - probeStart,
    probeDone: probeEnd,
    path: probePath(arrival),
  }
}

/** Whether a launch is worth a signature and a detail block. */
function isUnusual(launch) {
  return launch.errored || launch.webgl2Null || launch.probeFailed
}

/** The value at `fraction` of the sorted values (nearest rank). */
function percentile(values, fraction) {
  const sorted = values.toSorted((first, second) => first - second)
  return sorted[Math.max(0, Math.ceil(fraction * sorted.length) - 1)]
}

/** The median, 95th percentile and maximum of the defined values, or n/a. */
function spread(values) {
  const defined = values.filter((value) => value !== undefined)
  if (defined.length === 0) {
    return 'n/a'
  }
  return [0.5, 0.95, 1].map((fraction) => String(percentile(defined, fraction))).join(' / ')
}

/** A count with its share of `total`. */
function share(count, total) {
  return total === 0 ? '0' : `${String(count)} (${((100 * count) / total).toFixed(1)}%)`
}

/** Counts of the distinct values, most common first, as `value (count)`. */
function tally(values) {
  const counts = new Map()
  for (const value of values) {
    counts.set(value, (counts.get(value) ?? 0) + 1)
  }
  return [...counts]
    .toSorted((first, second) => second[1] - first[1])
    .map(([value, count]) => `${value} (${String(count)})`)
    .join(', ')
}

/** A Markdown table cell: one line, with its pipes escaped. */
function cell(text) {
  return String(text).replaceAll('|', '\\|').replaceAll('\n', ' ')
}

/** A Markdown table. */
function table(header, rows) {
  return [header, header.map(() => '---'), ...rows]
    .map((row) => `| ${row.map(cell).join(' | ')} |`)
    .join('\n')
}

/** The launches of each variant, in the order the variants first appear. */
function byVariant(launches) {
  const groups = new Map()
  for (const launch of launches) {
    groups.set(launch.record.variant, [...(groups.get(launch.record.variant) ?? []), launch])
  }
  return groups
}

/** One row of the main table. */
function summaryRow(variant, launches) {
  const nulls = launches.filter((launch) => launch.webgl2Null)
  return [
    variant,
    launches.length,
    launches.filter((launch) => launch.errored).length,
    share(nulls.length, launches.length),
    nulls.filter((launch) => launch.secondNull).length,
    launches.filter((launch) => launch.probeFailed).length,
    spread(launches.map((launch) => launch.probeMilliseconds)),
    spread(launches.map((launch) => launch.probeDone)),
    spread(launches.map((launch) => launch.record.launchMilliseconds)),
    tally(launches.map((launch) => launch.path)),
  ]
}

/** One row of the cold-start table: the first wave of each runner against the later waves. */
function coldRow(variant, launches) {
  const first = launches.filter((launch) => launch.record.wave === 1)
  const later = launches.filter((launch) => launch.record.wave > 1)
  return [
    variant,
    first.length,
    first.filter((launch) => launch.webgl2Null).length,
    first.filter((launch) => launch.probeFailed).length,
    spread(first.map((launch) => launch.probeMilliseconds)),
    spread(first.map((launch) => launch.record.launchMilliseconds)),
    spread(first.map((launch) => launch.record.startupStalls?.ioStall)),
    later.length,
    spread(later.map((launch) => launch.probeMilliseconds)),
    spread(later.map((launch) => launch.record.launchMilliseconds)),
    spread(later.map((launch) => launch.record.startupStalls?.ioStall)),
  ]
}

/** One row of the stalls table: how much the runner stalled while each launch started. */
function stallRow(variant, launches) {
  return [
    variant,
    ...STALLS.map(([counter]) =>
      spread(launches.map((launch) => launch.record.startupStalls?.[counter])),
    ),
  ]
}

/** The stalls of one launch in a few words. */
function stallText(stalls = {}) {
  const parts = STALLS.filter(([counter]) => stalls[counter] !== undefined).map(
    ([counter, label]) => `${label.toLowerCase()} ${String(stalls[counter])}`,
  )
  return parts.length === 0 ? 'none recorded' : parts.join(', ')
}

/** The reason a page gave for no context, or what it created. */
function contextText(context) {
  if (context === null || context === undefined) {
    return '-'
  }
  return context.created ? `created: ${context.renderer}` : (context.reason ?? 'no reason given')
}

/** How a launch ended, in a few words. */
function outcome(launch) {
  if (launch.launchFailed) {
    return 'launch failed'
  }
  if (launch.webgl2Null) {
    return 'no WebGL2'
  }
  if (launch.errored) {
    return 'error after launch'
  }
  return launch.probeFailed ? 'probe failed, WebGL2 created' : 'WebGL2 created'
}

/** The rows of the signature table: each distinct unusual launch and how often it happened. */
function signatureRows(variant, launches) {
  const signatures = launches.filter(isUnusual).map((launch) => {
    const [first, second] = launch.record.pages
    if (launch.launchFailed) {
      return [outcome(launch), launch.record.error?.split('\n')[0] ?? '', '-', '-', '-']
    }
    return [
      outcome(launch),
      contextText(first?.webgl2),
      contextText(first?.webgl),
      contextText(second?.webgl2),
      launch.record.output.find((line) => PROBE_FAILED.test(line.text))?.text ?? '-',
    ]
  })
  const counts = new Map()
  for (const signature of signatures) {
    const key = JSON.stringify(signature)
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  return [...counts].map(([key, count]) => [variant, count].concat(JSON.parse(key)))
}

/** At most DETAIL_LINES lines, each cut to DETAIL_WIDTH characters. */
function bounded(lines) {
  return lines.slice(0, DETAIL_LINES).map((line) => line.slice(0, DETAIL_WIDTH))
}

/** A collapsed block with everything one launch recorded. */
function detail(launch) {
  const { record } = launch
  const lines = [
    `launch ${String(record.launch)}, wave ${String(record.wave)}, slot ${String(record.slot)}, started ${String(record.startedMilliseconds)} ms into the run`,
    `launch took ${String(record.launchMilliseconds ?? 'n/a')} ms, probe ${String(launch.probeMilliseconds ?? 'n/a')} ms, done ${String(launch.probeDone ?? 'n/a')} ms after the launch started, path ${launch.path}`,
    `runner while the wave started: ${stallText(record.startupStalls)}`,
  ]
  if (record.error !== undefined) {
    lines.push(`error: ${record.error}`)
  }
  if (record.closeError !== undefined) {
    lines.push(`close error: ${record.closeError}`)
  }
  for (const [i, page] of record.pages.entries()) {
    lines.push(
      `page ${String(i + 1)}: webgl2 ${contextText(page.webgl2)}; webgl ${contextText(page.webgl)}`,
      ...bounded(page.console).map((message) => `  console ${message}`),
    )
  }
  lines.push(`browser output (${String(record.outputLines)} lines, the matching ones):`)
  const matching = record.output.filter((line) => DETAIL_LINE.test(line.text))
  lines.push(
    ...bounded(
      matching.map((line) => `  ${String(line.milliseconds).padStart(6)} ms  ${line.text}`),
    ),
  )
  if (record.aboutSupport !== undefined) {
    const support = record.aboutSupport.lines ?? [`could not be read: ${record.aboutSupport.error}`]
    lines.push('about:support:', ...bounded(support).map((line) => `  ${line}`))
  }
  const title = `${record.row} launch ${String(record.launch)}: ${outcome(launch)}`
  return `<details><summary>${title}</summary>\n\n\`\`\`text\n${lines.join('\n')}\n\`\`\`\n\n</details>`
}

/** The kernel messages of each row that has any, collapsed. */
function kernelSection(logs) {
  if (logs.length === 0) {
    return 'No kernel message named a segfault, a trap, gfxtest, an out-of-memory kill or a hung task.'
  }
  return logs
    .map(([row, text]) => {
      const lines = bounded(text.trimEnd().split('\n'))
      return `<details><summary>${row}</summary>\n\n\`\`\`text\n${lines.join('\n')}\n\`\`\`\n\n</details>`
    })
    .join('\n\n')
}

const paths = process.argv.slice(2)
const launches = (await readRecords(paths.filter((path) => path.endsWith('.jsonl')))).map(analyze)
const kernelLogs = await readKernelLogs(paths.filter((path) => path.endsWith('.kernel.log')))
const groups = byVariant(launches)
const variants = [...groups]
const sections = [
  '## Firefox WebGL2 experiment',
  `Launches: ${String(launches.length)}. Jobs: ${String(new Set(launches.map((launch) => launch.record.row)).size)}. Times in milliseconds as median / 95th percentile / maximum. "Probe" is Firefox's startup graphics probe, timed from its first to its last log line; "probe done" counts from the start of the launch. Firefox gives up on the probe 4000 ms after it starts waiting for it.`,
  table(
    [
      'Variant',
      'Launches',
      'Errors',
      'No WebGL2 on page 1',
      'Of those, no WebGL2 on page 2',
      'Probe failed',
      'Probe ms',
      'Probe done ms',
      'Launch ms',
      'Probe path',
    ],
    variants.map(([variant, group]) => summaryRow(variant, group)),
  ),
  '### First wave on each runner (cold) and later waves',
  table(
    [
      'Variant',
      'Wave 1 launches',
      'No WebGL2',
      'Probe failed',
      'Probe ms',
      'Launch ms',
      'Disk stall ms',
      'Later launches',
      'Later probe ms',
      'Later launch ms',
      'Later disk stall ms',
    ],
    variants.map(([variant, group]) => coldRow(variant, group)),
  ),
  '### Runner stalls while the browsers of a wave started',
  'From the start of the wave to the end of each launch. CPU, disk and memory stall: time in which some task waited for that resource (/proc/pressure). Steal: time the hypervisor held the CPUs, summed over them. Major faults: page faults that read the disk.',
  table(
    ['Variant', ...STALLS.map(([, label]) => label)],
    variants.map(([variant, group]) => stallRow(variant, group)),
  ),
  '### Failure signatures',
  table(
    [
      'Variant',
      'Count',
      'Outcome',
      'WebGL2, page 1',
      'WebGL 1, page 1',
      'WebGL2, page 2',
      'Probe line',
    ],
    variants.flatMap(([variant, group]) => signatureRows(variant, group)),
  ),
  '### Renderers of the created contexts',
  table(
    ['Variant', 'Renderer (count)'],
    variants.map(([variant, group]) => [
      variant,
      tally(
        group
          .filter((launch) => launch.record.pages?.[0]?.webgl2.created)
          .map((launch) => launch.record.pages[0].webgl2.renderer),
      ),
    ]),
  ),
  '### Kernel messages',
  kernelSection(kernelLogs),
  '### Launches in full',
  ...variants.flatMap(([, group]) => group.filter(isUnusual).slice(0, DETAIL_LIMIT).map(detail)),
  ...variants.map(([variant, group]) => {
    const first = group.find((launch) => launch.record.launch === 1)
    return first === undefined || isUnusual(first)
      ? ''
      : detail(first).replace('<summary>', `<summary>Control, ${variant}: `)
  }),
]
console.log(sections.filter((section) => section !== '').join('\n\n'))
