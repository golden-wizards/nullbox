<script setup lang="ts">
import { useNav } from '@slidev/client'

const { currentPage } = useNav()
</script>

<template>
  <!-- Gold rule on every slide; a small team mark from the second slide on. -->
  <div class="gw-rule" aria-hidden="true" />
  <footer v-if="currentPage > 1" class="gw-footer"><span aria-hidden="true">✦</span> Golden Wizards</footer>
</template>
