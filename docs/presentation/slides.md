---
theme: default
title: 'Golden Wizards: Nullbox in 167 minutes'
info: Retro for the fasthacks hackathon, team-06. Full report in docs/RETRO.md.
routerMode: hash
colorSchema: dark
transition: fade
mdc: true
fonts:
  sans: Nunito Sans
  serif: Cinzel
  weights: '400,600,700'
---

<!-- markdownlint-disable MD025 MD041 -->

<div class="grid grid-cols-[1fr_300px] gap-10 items-center h-full">
<div>

<div class="gw-eyebrow">✦ Golden Wizards · team-06</div>

# Nullbox in 167 minutes

<div class="big">16 points · 2nd of 8</div>

</div>
<img src="/golden-wizards.png" alt="Golden Wizards: the team's GitHub organisation image, four wizards in golden robes" class="gw-portrait" />
</div>

---

# Our approach

<div class="steps">
<div><b>1</b>Write the issues and the context first</div>
<div><b>2</b>One issue, one claim, one PR</div>
<div><b>3</b>Let Claude land everything, in parallel</div>
</div>

<div class="big pt-10">40 pull requests merged in 91 minutes</div>

---

# When things landed

<Timeline />

---

# What tripped us up

<div class="big pt-6 pb-6">Checks took too long.</div>

- CI needed about 8 minutes per push
- Near the end we stopped waiting for it and merged
- The last 2 points needed a pipeline under 5 minutes

---

# Next time

<div class="steps">
<div><b>1</b>Fast CI and automatic deploys first</div>
<div><b>2</b>Read the check texts before building</div>
<div><b>3</b>Merged is not scored until it is live</div>
</div>

<div class="pt-10 muted">Still with the judges: up to 8 more points.</div>
