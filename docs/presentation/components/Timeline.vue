<script setup lang="ts">
// Event timeline, 04:43Z to 07:30Z. Four small charts share one time axis:
// issues created, commits pushed in pull requests, pull requests merged (all per
// 10 minutes, from the GitHub API) and our HOT points (from the scoreboard log).
const W = 860
const L = 44
const R = 16
const span = 167 // minutes from start to end
const x = (m: number) => L + (m / span) * (W - L - R)

const ROW = 84 // vertical distance between rows
const PLOT = 50 // plot height inside a row
const TOP = 26 // first plot's top edge

interface CountRow {
  title: string
  unit: string
  color: string
  counts: number[]
}

const countRows: CountRow[] = [
  {
    title: 'Issues created (32)',
    unit: 'issue',
    color: '#3987e5',
    counts: [0, 0, 0, 0, 1, 0, 17, 1, 0, 2, 0, 0, 3, 6, 1, 0, 1],
  },
  {
    title: 'Commits pushed in pull requests (103)',
    unit: 'commit',
    color: '#d95926',
    counts: [0, 0, 0, 0, 0, 1, 1, 4, 2, 8, 9, 13, 10, 8, 27, 14, 6],
  },
  {
    title: 'Pull requests merged (40)',
    unit: 'PR',
    color: '#199e70',
    counts: [0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 6, 5, 4, 1, 10, 6, 3],
  },
]

const rows = countRows.map((row, r) => {
  const top = TOP + r * ROW
  const max = Math.max(...row.counts)
  const peak = row.counts.indexOf(max)
  const bars = row.counts.map((n, i) => {
    const from = i * 10
    const to = Math.min(from + 10, span)
    const h = (n / max) * PLOT
    return {
      n,
      x: x(from) + 1,
      w: x(to) - x(from) - 2,
      y: top + PLOT - h,
      h,
      label: `${clock(from)}–${clock(to)}: ${n} ${row.unit}${n === 1 ? '' : 's'}`,
    }
  })
  return { ...row, top, max, bars, peakX: bars[peak].x + bars[peak].w / 2 }
})

const steps: [number, number, string][] = [
  [104.4, 1, 'deploy'],
  [144.6, 2, 'seo-agentic'],
  [145.6, 4, 'ai-mcp'],
  [148.3, 5, 'seo'],
  [148.8, 6, 'firewall'],
  [150.9, 7, 'secret rotation'],
  [152.2, 9, 'observability'],
  [154.1, 10, 'login-protected'],
  [157.7, 13, 'payments'],
  [159.5, 15, 'ci-cd'],
  [163.5, 16, 'secret removal'],
]
const ptTop = TOP + countRows.length * ROW
const ptMax = 16
const y = (p: number) => ptTop + PLOT - (p / ptMax) * PLOT
let path = `M${x(0)},${y(0)}`
let prev = 0
for (const [m, p] of steps) {
  path += ` L${x(m)},${y(prev)} L${x(m)},${y(p)}`
  prev = p
}
path += ` L${x(span)},${y(prev)}`

const axisY = ptTop + PLOT
const H = axisY + 30
const ticks = [77, 137, 167]
function clock(m: number) {
  const t = 283 + m
  return `${String(Math.floor(t / 60)).padStart(2, '0')}:${String(Math.round(t % 60)).padStart(2, '0')}`
}
</script>

<template>
  <svg
    :viewBox="`0 0 ${W} ${H}`"
    role="img"
    aria-label="Issues created, commits, merged pull requests per ten minutes, and HOT points over the event"
    class="timeline"
  >
    <rect x="0" y="0" :width="W" :height="H" rx="8" fill="#17132e" stroke="#f2c14e" stroke-opacity="0.35" />

    <g v-for="row in rows" :key="row.title">
      <text :x="L" :y="row.top - 8" class="title">{{ row.title }}</text>
      <line :x1="L" :x2="W - R" :y1="row.top" :y2="row.top" class="grid" />
      <line :x1="L" :x2="W - R" :y1="row.top + PLOT" :y2="row.top + PLOT" class="axis" />
      <text :x="L - 6" :y="row.top + 4" class="tick end">{{ row.max }}</text>
      <text :x="L - 6" :y="row.top + PLOT + 4" class="tick end">0</text>
      <g v-for="b in row.bars" :key="b.x">
        <rect v-if="b.n" :x="b.x" :y="b.y" :width="b.w" :height="b.h" rx="3" :fill="row.color">
          <title>{{ b.label }}</title>
        </rect>
      </g>
      <text :x="row.peakX" :y="row.top - 4" class="note mid">{{ row.max }}</text>
    </g>

    <text :x="L" :y="ptTop - 8" class="title">HOT points (16 at the end)</text>
    <line :x1="L" :x2="W - R" :y1="y(16)" :y2="y(16)" class="grid" />
    <line :x1="L" :x2="W - R" :y1="axisY" :y2="axisY" class="axis" />
    <text :x="L - 6" :y="y(16) + 4" class="tick end">16</text>
    <text :x="L - 6" :y="axisY + 4" class="tick end">0</text>
    <path :d="path" fill="none" stroke="#c98500" stroke-width="2" stroke-linejoin="round" />
    <g v-for="[m, p, name] in steps" :key="name">
      <circle :cx="x(m)" :cy="y(p)" r="4" fill="#c98500" stroke="#17132e" stroke-width="2" />
      <circle :cx="x(m)" :cy="y(p)" r="10" fill="transparent">
        <title>{{ clock(m) }}Z: {{ name }}, {{ p }} points</title>
      </circle>
    </g>
    <text :x="x(104.4)" :y="y(1) - 9" class="note mid">Deploy passes, 06:27</text>
    <text :x="x(141)" :y="y(11)" class="note end">10 checks in 19 minutes</text>

    <g v-for="t in ticks" :key="t">
      <line :x1="x(t)" :x2="x(t)" :y1="axisY" :y2="axisY + 5" class="axis" />
      <text :x="x(t)" :y="axisY + 20" class="tick" :class="t === span ? 'end' : 'mid'">{{ clock(t) }}Z</text>
    </g>
    <text :x="x(0)" :y="axisY + 20" class="tick">start 04:43Z</text>
  </svg>
</template>

<style scoped>
.timeline {
  width: 100%;
  height: auto;
  font-family: inherit;
}
.title {
  font-size: 12px;
  font-weight: 600;
  fill: #f3ecd8;
}
.tick,
.note {
  font-size: 11px;
  fill: #b9b2cf;
}
.note {
  fill: #f3ecd8;
}
.mid {
  text-anchor: middle;
}
.end {
  text-anchor: end;
}
.axis {
  stroke: #b9b2cf;
  stroke-width: 1;
}
.grid {
  stroke: #b9b2cf;
  stroke-width: 1;
  stroke-opacity: 0.18;
}
</style>
