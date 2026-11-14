<script setup lang="ts">
import { useNav } from '@slidev/client'
import OkTechLogo from './components/OkTechLogo.vue'

const { currentPage } = useNav()
</script>

<template>
  <!-- Brand strip on every slide; the title slide carries the large logo instead. -->
  <div class="oktech-stripe" aria-hidden="true"><i /><i /><i /></div>
  <footer v-if="currentPage > 1" class="oktech-footer">
    <OkTechLogo />
    <span>OKTECH</span>
  </footer>
</template>
