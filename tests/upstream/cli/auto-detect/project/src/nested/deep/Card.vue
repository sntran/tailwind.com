<template>
  <div :class="{ 'shadow-lg': raised, 'opacity-50': disabled }" class="rounded-xl border border-gray-200">
    <slot />
  </div>
</template>
<script setup>
defineProps({ raised: Boolean, disabled: Boolean })
</script>
