<script setup lang="ts">
import { ref, onMounted, computed, watch, nextTick } from 'vue';
import { Doughnut, Bar, Line } from 'vue-chartjs';
import {
  Chart as ChartJS,
  ArcElement,
  Tooltip,
  Legend,
  CategoryScale,
  LinearScale,
  BarElement,
  PointElement,
  LineElement,
  Filler,
} from 'chart.js';
import * as api from '../services/api';
import type { SpendingByCategory, MonthlyTrend, CashFlowReport } from '../types';
import { format, subMonths, startOfMonth, endOfMonth } from 'date-fns';

ChartJS.register(
  ArcElement,
  Tooltip,
  Legend,
  CategoryScale,
  LinearScale,
  BarElement,
  PointElement,
  LineElement,
  Filler
);

const loading = ref(false);
const spendingByCategory = ref<SpendingByCategory[]>([]);
const monthlyTrends = ref<MonthlyTrend[]>([]);
const cashFlowReport = ref<CashFlowReport | null>(null);

const selectedPeriod = ref<'month' | 'quarter' | 'year'>('month');

const periodDates = computed(() => {
  const now = new Date();
  switch (selectedPeriod.value) {
    case 'month':
      return {
        start: format(startOfMonth(now), 'yyyy-MM-dd'),
        end: format(endOfMonth(now), 'yyyy-MM-dd'),
      };
    case 'quarter':
      return {
        start: format(subMonths(startOfMonth(now), 2), 'yyyy-MM-dd'),
        end: format(endOfMonth(now), 'yyyy-MM-dd'),
      };
    case 'year':
      return {
        start: format(subMonths(startOfMonth(now), 11), 'yyyy-MM-dd'),
        end: format(endOfMonth(now), 'yyyy-MM-dd'),
      };
  }
});

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

const categoryChartData = computed(() => ({
  labels: spendingByCategory.value.map(s => s.category_name),
  datasets: [
    {
      data: spendingByCategory.value.map(s => parseFloat(s.total)),
      backgroundColor: spendingByCategory.value.map(s => s.category_color),
      borderWidth: 0,
    },
  ],
}));

const trendsChartData = computed(() => ({
  labels: monthlyTrends.value.map(t => `${t.month} ${t.year}`),
  datasets: [
    {
      label: 'Income',
      data: monthlyTrends.value.map(t => parseFloat(t.income)),
      backgroundColor: '#22c55e',
    },
    {
      label: 'Expenses',
      data: monthlyTrends.value.map(t => parseFloat(t.expenses)),
      backgroundColor: '#ef4444',
    },
  ],
}));

const netChartData = computed(() => ({
  labels: monthlyTrends.value.map(t => `${t.month} ${t.year}`),
  datasets: [
    {
      label: 'Net Income',
      data: monthlyTrends.value.map(t => parseFloat(t.net)),
      borderColor: '#3b82f6',
      backgroundColor: 'rgba(59, 130, 246, 0.1)',
      fill: true,
      tension: 0.4,
    },
  ],
}));

const chartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      position: 'bottom' as const,
    },
  },
};

const barChartOptions = {
  ...chartOptions,
  scales: {
    y: {
      beginAtZero: true,
      ticks: {
        callback: function(value: string | number) {
          return formatCurrency(value);
        },
      },
    },
  },
};

async function fetchReports() {
  loading.value = true;
  // Ensure loading overlay renders before heavy operations
  await nextTick();
  try {
    const [spending, trends, cashFlow] = await Promise.all([
      api.getSpendingByCategory(periodDates.value.start, periodDates.value.end),
      api.getMonthlyTrends(12),
      api.getCashFlowReport(periodDates.value.start, periodDates.value.end),
    ]);
    spendingByCategory.value = spending;
    monthlyTrends.value = trends;
    cashFlowReport.value = cashFlow;
  } catch (e) {
    console.error('Failed to fetch reports:', e);
  } finally {
    loading.value = false;
  }
}

watch(selectedPeriod, fetchReports);
onMounted(fetchReports);
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Reports</h1>
      <div class="flex gap-2">
        <button
          v-for="period in ['month', 'quarter', 'year']"
          :key="period"
          @click="selectedPeriod = period as 'month' | 'quarter' | 'year'"
          :class="[
            'px-4 py-2 rounded-lg transition-colors capitalize',
            selectedPeriod === period
              ? 'bg-blue-600 text-white'
              : 'bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600'
          ]"
        >
          {{ period }}
        </button>
      </div>
    </div>

    <!-- Summary Cards -->
    <div v-if="cashFlowReport" class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Income</h3>
        <p class="text-2xl font-bold text-green-600 mt-2">
          {{ formatCurrency(cashFlowReport.total_income) }}
        </p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Expenses</h3>
        <p class="text-2xl font-bold text-red-600 mt-2">
          {{ formatCurrency(cashFlowReport.total_expenses) }}
        </p>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Net Cash Flow</h3>
        <p
          :class="[
            'text-2xl font-bold mt-2',
            parseFloat(cashFlowReport.net_cash_flow) >= 0 ? 'text-green-600' : 'text-red-600'
          ]"
        >
          {{ formatCurrency(cashFlowReport.net_cash_flow) }}
        </p>
      </div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <!-- Spending by Category -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Spending by Category</h2>
        </div>
        <div class="p-4">
          <div v-if="spendingByCategory.length === 0" class="text-center text-gray-500 py-8">
            No spending data available
          </div>
          <div v-else class="h-64">
            <Doughnut :data="categoryChartData" :options="chartOptions" />
          </div>
          <div class="mt-4 space-y-2">
            <div
              v-for="cat in spendingByCategory.slice(0, 5)"
              :key="cat.category_id"
              class="flex items-center justify-between text-sm"
            >
              <div class="flex items-center gap-2">
                <div class="w-3 h-3 rounded-full" :style="{ backgroundColor: cat.category_color }" />
                <span class="text-gray-700 dark:text-gray-300">{{ cat.category_name }}</span>
              </div>
              <span class="font-medium text-gray-900 dark:text-white">
                {{ formatCurrency(cat.total) }} ({{ cat.percentage.toFixed(1) }}%)
              </span>
            </div>
          </div>
        </div>
      </div>

      <!-- Income vs Expenses -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Income vs Expenses</h2>
        </div>
        <div class="p-4">
          <div v-if="monthlyTrends.length === 0" class="text-center text-gray-500 py-8">
            No trend data available
          </div>
          <div v-else class="h-64">
            <Bar :data="trendsChartData" :options="barChartOptions" />
          </div>
        </div>
      </div>

      <!-- Net Income Trend -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow lg:col-span-2">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Net Income Trend</h2>
        </div>
        <div class="p-4">
          <div v-if="monthlyTrends.length === 0" class="text-center text-gray-500 py-8">
            No trend data available
          </div>
          <div v-else class="h-64">
            <Line :data="netChartData" :options="barChartOptions" />
          </div>
        </div>
      </div>
    </div>

    <!-- Loading Overlay -->
    <div
      v-if="loading"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
    >
      <div class="bg-white dark:bg-gray-800 rounded-xl shadow-2xl p-8 max-w-sm w-full mx-4 text-center">
        <div class="relative w-16 h-16 mx-auto mb-4">
          <svg class="animate-spin w-16 h-16 text-blue-600" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
          </svg>
        </div>
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Loading Reports...</h3>
        <p class="text-sm text-gray-500 dark:text-gray-400">
          Analyzing your transactions and generating reports.
        </p>
      </div>
    </div>
  </div>
</template>
