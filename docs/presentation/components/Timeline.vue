<script setup lang="ts">
// Event timeline, 04:43Z to 07:30Z. Two charts share one time axis:
// merged PRs per 10 minutes (GitHub merged_at) and our HOT points (scoreboard log).
const W = 860
const L = 44
const R = 16
const span = 167 // minutes from start to end
const x = (m: number) => L + (m / span) * (W - L - R)

const merged = [0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 6, 5, 4, 1, 10, 6, 3]
const barTop = 28
const barH = 120
const barMax = 10
const bars = merged.map((n, i) => {
  const from = i * 10
  const to = Math.min(from + 10, span)
  return {
    n,
    x: x(from) + 1,
    w: x(to) - x(from) - 2,
    h: (n / barMax) * barH,
    label: `${clock(from)}–${clock(to)}: ${n} PR${n === 1 ? '' : 's'} merged`,
  }
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
const ptTop = 208
const ptH = 120
const ptMax = 16
const y = (p: number) => ptTop + ptH - (p / ptMax) * ptH
let path = `M${x(0)},${y(0)}`
let prev = 0
for (const [m, p] of steps) {
  path += ` L${x(m)},${y(prev)} L${x(m)},${y(p)}`
  prev = p
}
path += ` L${x(span)},${y(prev)}`

const ticks = [77, 137, 167]
function clock(m: number) {
  const t = 283 + m
  return `${String(Math.floor(t / 60)).padStart(2, '0')}:${String(Math.round(t % 60)).padStart(2, '0')}`
}
</script>

<template>
  <svg :viewBox="`0 0 ${W} 372`" role="img" aria-label="Merged pull requests per ten minutes and HOT points over the event" class="timeline">
    <rect x="0" y="0" :width="W" height="372" rx="8" fill="#fcfcfb" />

    <text :x="L" y="18" class="title">Pull requests merged per 10 minutes (40 in total)</text>
    <line :x1="L" :x2="W - R" :y1="barTop + barH" :y2="barTop + barH" class="axis" />
    <line :x1="L" :x2="W - R" :y1="barTop" :y2="barTop" class="grid" />
    <text :x="L - 6" :y="barTop + 4" class="tick end">10</text>
    <text :x="L - 6" :y="barTop + barH + 4" class="tick end">0</text>
    <g v-for="b in bars" :key="b.x">
      <rect v-if="b.n" :x="b.x" :y="barTop + barH - b.h" :width="b.w" :height="b.h" rx="3" fill="#2a78d6">
        <title>{{ b.label }}</title>
      </rect>
    </g>
    <text :x="x(145)" :y="barTop - 4" class="note mid">10</text>

    <text :x="L" :y="ptTop - 12" class="title">HOT points (16 at the end)</text>
    <line :x1="L" :x2="W - R" :y1="ptTop + ptH" :y2="ptTop + ptH" class="axis" />
    <line :x1="L" :x2="W - R" :y1="y(16)" :y2="y(16)" class="grid" />
    <line :x1="L" :x2="W - R" :y1="y(8)" :y2="y(8)" class="grid" />
    <text :x="L - 6" :y="y(16) + 4" class="tick end">16</text>
    <text :x="L - 6" :y="y(8) + 4" class="tick end">8</text>
    <text :x="L - 6" :y="y(0) + 4" class="tick end">0</text>
    <path :d="path" fill="none" stroke="#eb6834" stroke-width="2" stroke-linejoin="round" />
    <g v-for="[m, p, name] in steps" :key="name">
      <circle :cx="x(m)" :cy="y(p)" r="4" fill="#eb6834" stroke="#fcfcfb" stroke-width="2" />
      <circle :cx="x(m)" :cy="y(p)" r="10" fill="transparent">
        <title>{{ clock(m) }}Z: {{ name }}, {{ p }} points</title>
      </circle>
    </g>
    <text :x="x(104.4)" :y="y(1) - 10" class="note mid">Deploy passes, 06:27</text>
    <text :x="x(140)" :y="y(10)" class="note end">10 checks in 19 minutes</text>

    <g v-for="t in ticks" :key="t">
      <line :x1="x(t)" :x2="x(t)" :y1="ptTop + ptH" :y2="ptTop + ptH + 5" class="axis" />
      <text :x="x(t)" :y="ptTop + ptH + 20" class="tick" :class="t === span ? 'end' : 'mid'">{{ clock(t) }}Z</text>
    </g>
    <text :x="x(0)" :y="ptTop + ptH + 20" class="tick">start 04:43Z</text>
  </svg>
</template>

<style scoped>
.timeline {
  width: 100%;
  height: auto;
  font-family: inherit;
}
.title {
  font-size: 13px;
  font-weight: 600;
  fill: #0b0b0b;
}
.tick,
.note {
  font-size: 11px;
  fill: #52514e;
}
.note {
  fill: #0b0b0b;
}
.mid {
  text-anchor: middle;
}
.end {
  text-anchor: end;
}
.axis {
  stroke: #52514e;
  stroke-width: 1;
}
.grid {
  stroke: #52514e;
  stroke-width: 1;
  stroke-opacity: 0.18;
}
</style>
