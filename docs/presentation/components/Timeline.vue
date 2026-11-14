<script setup lang="ts">
// Event timeline, 04:43Z to 07:30Z, as one Chart.js chart on a single count axis:
// bars are per-10-minute counts from the GitHub API (issues created, commits pushed
// in pull requests, pull requests merged); the line is our running HOT points total
// at the end of each 10-minute window, from the scoreboard log.
import {
  BarController,
  BarElement,
  CategoryScale,
  Chart,
  Legend,
  LinearScale,
  LineController,
  LineElement,
  type Plugin,
  PointElement,
  Tooltip,
} from 'chart.js'
import { onBeforeUnmount, onMounted, ref } from 'vue'

Chart.register(
  BarController,
  BarElement,
  LineController,
  LineElement,
  PointElement,
  CategoryScale,
  LinearScale,
  Legend,
  Tooltip,
)

const START = 283 // 04:43Z in minutes
const SPAN = 167
function clock(m: number) {
  const t = START + m
  return `${String(Math.floor(t / 60)).padStart(2, '0')}:${String(t % 60).padStart(2, '0')}`
}
const labels = Array.from({ length: 17 }, (_, i) => clock(i * 10))
const windowLabel = (i: number) => `${clock(i * 10)}–${clock(Math.min(i * 10 + 10, SPAN))}Z`

const issues = [0, 0, 0, 0, 1, 0, 17, 1, 0, 2, 0, 0, 3, 6, 1, 0, 1]
const commits = [0, 0, 0, 0, 0, 1, 1, 4, 2, 8, 9, 13, 10, 8, 27, 14, 6]
const merged = [0, 0, 0, 0, 0, 0, 0, 2, 0, 3, 6, 5, 4, 1, 10, 6, 3]
const points = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 6, 15, 16]

const INK = '#f3ecd8'
const MUTED = '#b9b2cf'
const FONT = "'Nunito Sans', ui-sans-serif, system-ui, sans-serif"

// Direct label at the end of the points line, so the headline number needs no lookup.
const endLabel: Plugin = {
  id: 'endLabel',
  afterDatasetsDraw(chart) {
    const meta = chart.getDatasetMeta(3)
    const last = meta.data[meta.data.length - 1]
    if (!last || meta.hidden) return
    const { ctx } = chart
    ctx.save()
    ctx.fillStyle = INK
    ctx.font = `600 13px ${FONT}`
    ctx.textAlign = 'right'
    ctx.fillText('16 points', last.x - 10, last.y - 10)
    ctx.restore()
  },
}

const canvas = ref<HTMLCanvasElement>()
let chart: Chart | undefined

onMounted(() => {
  if (!canvas.value) return
  const bar = (label: string, data: number[], color: string) => ({
    type: 'bar' as const,
    label,
    data,
    backgroundColor: color,
    borderRadius: 3,
    borderSkipped: 'bottom' as const,
    categoryPercentage: 0.82,
    barPercentage: 0.9,
    order: 2,
  })
  chart = new Chart(canvas.value, {
    data: {
      labels,
      datasets: [
        bar('Issues created (32)', issues, '#3987e5'),
        bar('Commits in pull requests (103)', commits, '#d95926'),
        bar('Pull requests merged (40)', merged, '#199e70'),
        {
          type: 'line',
          label: 'HOT points, running total (16)',
          data: points,
          borderColor: '#c98500',
          backgroundColor: '#c98500',
          borderWidth: 2,
          stepped: 'middle',
          pointRadius: 4,
          pointHoverRadius: 6,
          pointBorderColor: '#17132e',
          pointBorderWidth: 2,
          order: 1,
        },
      ],
    },
    options: {
      // Fixed size: Slidev scales the whole slide with a CSS transform, which throws
      // off Chart.js's responsive measuring.
      responsive: false,
      devicePixelRatio: 2,

      animation: { duration: 500 },
      interaction: { mode: 'index', intersect: false },
      plugins: {
        legend: {
          position: 'top',
          align: 'start',
          labels: { color: INK, font: { family: FONT, size: 13 }, boxWidth: 14, boxHeight: 14 },
        },
        tooltip: {
          titleFont: { family: FONT },
          bodyFont: { family: FONT },
          callbacks: { title: (items) => windowLabel(items[0].dataIndex) },
        },
      },
      scales: {
        x: {
          title: { display: true, text: '10-minute windows, UTC', color: MUTED, font: { family: FONT } },
          ticks: { color: MUTED, font: { family: FONT, size: 11 }, maxRotation: 0 },
          grid: { display: false },
          border: { color: MUTED },
        },
        y: {
          beginAtZero: true,
          title: { display: true, text: 'Count', color: MUTED, font: { family: FONT } },
          ticks: { color: MUTED, font: { family: FONT, size: 11 }, stepSize: 5 },
          grid: { color: '#b9b2cf2e' },
          border: { display: false },
        },
      },
    },
    plugins: [endLabel],
  })
})

onBeforeUnmount(() => chart?.destroy())
</script>

<template>
  <div class="timeline">
    <canvas
      width="832"
      height="372"
      ref="canvas"
      role="img"
      aria-label="Per ten minutes: issues created, commits in pull requests and pull requests merged, with the running total of HOT points"
    />
  </div>
</template>

<style scoped>
.timeline {
  position: relative;
  width: 866px;
  height: 398px;
  padding: 12px 16px 8px;
  border: 1px solid #f2c14e59;
  border-radius: 8px;
  background: #17132e;
}
</style>
