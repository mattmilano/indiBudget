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
import { jsPDF } from 'jspdf';
import autoTable from 'jspdf-autotable';
import * as api from '../services/api';
import { useAccountsStore } from '../stores';
import type { SpendingByCategory, MonthlyTrend, CashFlowReport } from '../types';
import { format, subMonths, subYears, startOfMonth, endOfMonth } from 'date-fns';

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

const accountsStore = useAccountsStore();
const loading = ref(false);
const spendingByCategory = ref<SpendingByCategory[]>([]);
const monthlyTrends = ref<MonthlyTrend[]>([]);
const cashFlowReport = ref<CashFlowReport | null>(null);
const lastYearCashFlow = ref<CashFlowReport | null>(null);

// Net worth history tracking
interface NetWorthSnapshot {
  date: string;
  assets: number;
  liabilities: number;
  netWorth: number;
}
const netWorthHistory = ref<NetWorthSnapshot[]>([]);
const NET_WORTH_STORAGE_KEY = 'indibudget_networth_history';

const selectedPeriod = ref<'month' | 'quarter' | 'year' | 'all'>('month');

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
    case 'all':
      // Return empty dates to get all transactions
      return {
        start: '1900-01-01',
        end: format(endOfMonth(now), 'yyyy-MM-dd'),
      };
  }
});

// Last year same period for comparison
const lastYearPeriodDates = computed(() => {
  const now = new Date();
  const lastYear = subYears(now, 1);
  switch (selectedPeriod.value) {
    case 'month':
      return {
        start: format(startOfMonth(lastYear), 'yyyy-MM-dd'),
        end: format(endOfMonth(lastYear), 'yyyy-MM-dd'),
      };
    case 'quarter':
      return {
        start: format(subMonths(startOfMonth(lastYear), 2), 'yyyy-MM-dd'),
        end: format(endOfMonth(lastYear), 'yyyy-MM-dd'),
      };
    case 'year':
      return {
        start: format(subMonths(startOfMonth(lastYear), 11), 'yyyy-MM-dd'),
        end: format(endOfMonth(lastYear), 'yyyy-MM-dd'),
      };
    case 'all':
      // For "all time", compare to all historical data as well (no real comparison)
      return {
        start: '1900-01-01',
        end: format(endOfMonth(lastYear), 'yyyy-MM-dd'),
      };
  }
});

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

// Net worth breakdown
const netWorthBreakdown = computed(() => {
  const assets = accountsStore.accounts
    .filter(a => ['checking', 'savings', 'cash', 'investment'].includes(a.account_type))
    .reduce((sum, a) => sum + parseFloat(a.balance || '0'), 0);

  const liabilities = accountsStore.accounts
    .filter(a => ['credit_card', 'loan'].includes(a.account_type))
    .reduce((sum, a) => sum + parseFloat(a.balance || '0'), 0);

  return {
    assets,
    liabilities,
    netWorth: assets - liabilities,
  };
});

// Year-over-year comparison
const yoyComparison = computed(() => {
  if (!cashFlowReport.value || !lastYearCashFlow.value) return null;

  const currentExpenses = parseFloat(cashFlowReport.value.total_expenses) || 0;
  const lastYearExpenses = parseFloat(lastYearCashFlow.value.total_expenses) || 0;
  const currentIncome = parseFloat(cashFlowReport.value.total_income) || 0;
  const lastYearIncome = parseFloat(lastYearCashFlow.value.total_income) || 0;

  const expenseChange = lastYearExpenses > 0
    ? ((currentExpenses - lastYearExpenses) / lastYearExpenses) * 100
    : 0;
  const incomeChange = lastYearIncome > 0
    ? ((currentIncome - lastYearIncome) / lastYearIncome) * 100
    : 0;

  return {
    currentExpenses,
    lastYearExpenses,
    expenseChange,
    currentIncome,
    lastYearIncome,
    incomeChange,
  };
});

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

// Net worth history chart data
const netWorthChartData = computed(() => ({
  labels: netWorthHistory.value.map(h => format(new Date(h.date), 'MMM d')),
  datasets: [
    {
      label: 'Net Worth',
      data: netWorthHistory.value.map(h => h.netWorth),
      borderColor: '#8b5cf6',
      backgroundColor: 'rgba(139, 92, 246, 0.1)',
      fill: true,
      tension: 0.4,
    },
    {
      label: 'Assets',
      data: netWorthHistory.value.map(h => h.assets),
      borderColor: '#22c55e',
      backgroundColor: 'transparent',
      borderDash: [5, 5],
      tension: 0.4,
    },
    {
      label: 'Liabilities',
      data: netWorthHistory.value.map(h => h.liabilities),
      borderColor: '#ef4444',
      backgroundColor: 'transparent',
      borderDash: [5, 5],
      tension: 0.4,
    },
  ],
}));

// Net worth history functions
function loadNetWorthHistory() {
  try {
    const stored = localStorage.getItem(NET_WORTH_STORAGE_KEY);
    if (stored) {
      netWorthHistory.value = JSON.parse(stored);
    }
  } catch (e) {
    console.error('Failed to load net worth history:', e);
    netWorthHistory.value = [];
  }
}

function saveNetWorthHistory() {
  try {
    localStorage.setItem(NET_WORTH_STORAGE_KEY, JSON.stringify(netWorthHistory.value));
  } catch (e) {
    console.error('Failed to save net worth history:', e);
  }
}

function recordNetWorthSnapshot() {
  const today = format(new Date(), 'yyyy-MM-dd');

  // Check if we already have a snapshot for today
  const existingIndex = netWorthHistory.value.findIndex(h => h.date === today);

  const snapshot: NetWorthSnapshot = {
    date: today,
    assets: netWorthBreakdown.value.assets,
    liabilities: netWorthBreakdown.value.liabilities,
    netWorth: netWorthBreakdown.value.netWorth,
  };

  if (existingIndex >= 0) {
    // Update existing snapshot for today
    netWorthHistory.value[existingIndex] = snapshot;
  } else {
    // Add new snapshot
    netWorthHistory.value.push(snapshot);
  }

  // Sort by date and keep last 365 days
  netWorthHistory.value.sort((a, b) => a.date.localeCompare(b.date));
  if (netWorthHistory.value.length > 365) {
    netWorthHistory.value = netWorthHistory.value.slice(-365);
  }

  saveNetWorthHistory();
}

// Net worth change calculations
const netWorthChange = computed(() => {
  if (netWorthHistory.value.length < 2) return null;

  const latest = netWorthHistory.value[netWorthHistory.value.length - 1];
  const weekAgo = netWorthHistory.value.find(h => {
    const diff = (new Date(latest.date).getTime() - new Date(h.date).getTime()) / (1000 * 60 * 60 * 24);
    return diff >= 7;
  });
  const monthAgo = netWorthHistory.value.find(h => {
    const diff = (new Date(latest.date).getTime() - new Date(h.date).getTime()) / (1000 * 60 * 60 * 24);
    return diff >= 30;
  });

  return {
    weekChange: weekAgo ? latest.netWorth - weekAgo.netWorth : null,
    weekPercent: weekAgo && weekAgo.netWorth !== 0
      ? ((latest.netWorth - weekAgo.netWorth) / Math.abs(weekAgo.netWorth)) * 100
      : null,
    monthChange: monthAgo ? latest.netWorth - monthAgo.netWorth : null,
    monthPercent: monthAgo && monthAgo.netWorth !== 0
      ? ((latest.netWorth - monthAgo.netWorth) / Math.abs(monthAgo.netWorth)) * 100
      : null,
  };
});

// Savings rate analytics
const savingsRateData = computed(() => {
  return monthlyTrends.value.map(trend => {
    const income = parseFloat(trend.income) || 0;
    const expenses = parseFloat(trend.expenses) || 0;
    const net = parseFloat(trend.net) || 0;
    const savingsRate = income > 0 ? (net / income) * 100 : 0;
    return {
      month: trend.month,
      year: trend.year,
      income,
      expenses,
      net,
      savingsRate,
    };
  });
});

const savingsRateChartData = computed(() => ({
  labels: savingsRateData.value.map(d => `${d.month} ${d.year}`),
  datasets: [
    {
      label: 'Savings Rate %',
      data: savingsRateData.value.map(d => d.savingsRate),
      borderColor: '#8b5cf6',
      backgroundColor: savingsRateData.value.map(d =>
        d.savingsRate >= 20 ? 'rgba(34, 197, 94, 0.5)' :
        d.savingsRate >= 10 ? 'rgba(234, 179, 8, 0.5)' :
        d.savingsRate >= 0 ? 'rgba(251, 146, 60, 0.5)' :
        'rgba(239, 68, 68, 0.5)'
      ),
      tension: 0.4,
    },
  ],
}));

const savingsRateSummary = computed(() => {
  const rates = savingsRateData.value.filter(d => d.income > 0);
  if (rates.length === 0) return null;

  const avgRate = rates.reduce((sum, d) => sum + d.savingsRate, 0) / rates.length;
  const totalSaved = rates.reduce((sum, d) => sum + d.net, 0);
  const totalIncome = rates.reduce((sum, d) => sum + d.income, 0);
  const currentRate = rates.length > 0 ? rates[rates.length - 1].savingsRate : 0;

  // Find months with positive savings
  const positiveSavingsMonths = rates.filter(d => d.net > 0).length;

  // Best and worst months
  const sortedByRate = [...rates].sort((a, b) => b.savingsRate - a.savingsRate);
  const bestMonth = sortedByRate[0];
  const worstMonth = sortedByRate[sortedByRate.length - 1];

  return {
    avgRate,
    currentRate,
    totalSaved,
    totalIncome,
    positiveSavingsMonths,
    totalMonths: rates.length,
    bestMonth,
    worstMonth,
  };
});

const savingsRateChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      display: false,
    },
    tooltip: {
      callbacks: {
        label: function(context: any) {
          return `Savings Rate: ${context.raw.toFixed(1)}%`;
        },
      },
    },
  },
  scales: {
    y: {
      beginAtZero: true,
      ticks: {
        callback: function(value: string | number) {
          return `${value}%`;
        },
      },
    },
  },
};

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

// CSV helper to properly escape fields
function escapeCsvField(field: string): string {
  if (field.includes('"') || field.includes(',') || field.includes('\n')) {
    return `"${field.replace(/"/g, '""')}"`;
  }
  return field;
}

// Export functions
function exportToCSV() {
  const lines: string[] = [];
  const periodLabel = selectedPeriod.value.charAt(0).toUpperCase() + selectedPeriod.value.slice(1);

  // Header
  lines.push(`indiBudget Financial Report - ${periodLabel}`);
  lines.push(`Period: ${periodDates.value.start} to ${periodDates.value.end}`);
  lines.push(`Generated: ${format(new Date(), 'yyyy-MM-dd HH:mm')}`);
  lines.push('');

  // Net Worth Summary
  lines.push('NET WORTH SUMMARY');
  lines.push('Type,Amount');
  lines.push(`Assets,${netWorthBreakdown.value.assets.toFixed(2)}`);
  lines.push(`Liabilities,${netWorthBreakdown.value.liabilities.toFixed(2)}`);
  lines.push(`Net Worth,${netWorthBreakdown.value.netWorth.toFixed(2)}`);
  lines.push('');

  // Cash Flow Summary
  if (cashFlowReport.value) {
    lines.push('CASH FLOW SUMMARY');
    lines.push('Metric,Amount');
    lines.push(`Total Income,${parseFloat(cashFlowReport.value.total_income).toFixed(2)}`);
    lines.push(`Total Expenses,${parseFloat(cashFlowReport.value.total_expenses).toFixed(2)}`);
    lines.push(`Net Cash Flow,${parseFloat(cashFlowReport.value.net_cash_flow).toFixed(2)}`);
    lines.push('');
  }

  // Spending by Category
  if (spendingByCategory.value.length > 0) {
    lines.push('SPENDING BY CATEGORY');
    lines.push('Category,Amount,Percentage');
    spendingByCategory.value.forEach(cat => {
      lines.push(`${escapeCsvField(cat.category_name)},${parseFloat(cat.total).toFixed(2)},${cat.percentage.toFixed(1)}%`);
    });
    lines.push('');
  }

  // Monthly Trends
  if (monthlyTrends.value.length > 0) {
    lines.push('MONTHLY TRENDS');
    lines.push('Month,Year,Income,Expenses,Net');
    monthlyTrends.value.forEach(trend => {
      lines.push(`${trend.month},${trend.year},${parseFloat(trend.income).toFixed(2)},${parseFloat(trend.expenses).toFixed(2)},${parseFloat(trend.net).toFixed(2)}`);
    });
  }

  // Create and download file
  const csvContent = lines.join('\n');
  const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
  const link = document.createElement('a');
  link.href = URL.createObjectURL(blob);
  link.download = `indibudget-report-${periodDates.value.start}-to-${periodDates.value.end}.csv`;
  link.click();
  URL.revokeObjectURL(link.href);
}

function exportToPDF() {
  const doc = new jsPDF();
  const periodLabel = selectedPeriod.value.charAt(0).toUpperCase() + selectedPeriod.value.slice(1);
  let yPos = 20;

  // Title
  doc.setFontSize(20);
  doc.setTextColor(30, 64, 175); // Blue color
  doc.text('indiBudget Financial Report', 105, yPos, { align: 'center' });
  yPos += 10;

  // Subtitle
  doc.setFontSize(12);
  doc.setTextColor(100, 100, 100);
  doc.text(`${periodLabel}ly Report: ${periodDates.value.start} to ${periodDates.value.end}`, 105, yPos, { align: 'center' });
  yPos += 8;
  doc.text(`Generated: ${format(new Date(), 'MMMM d, yyyy')}`, 105, yPos, { align: 'center' });
  yPos += 15;

  // Net Worth Section
  doc.setFontSize(14);
  doc.setTextColor(0, 0, 0);
  doc.text('Net Worth Summary', 14, yPos);
  yPos += 5;

  autoTable(doc, {
    startY: yPos,
    head: [['Description', 'Amount']],
    body: [
      ['Total Assets', formatCurrency(netWorthBreakdown.value.assets)],
      ['Total Liabilities', formatCurrency(netWorthBreakdown.value.liabilities)],
      ['Net Worth', formatCurrency(netWorthBreakdown.value.netWorth)],
    ],
    theme: 'striped',
    headStyles: { fillColor: [30, 64, 175] },
    margin: { left: 14 },
  });

  yPos = (doc as any).lastAutoTable.finalY + 15;

  // Cash Flow Summary
  if (cashFlowReport.value) {
    doc.setFontSize(14);
    doc.text('Cash Flow Summary', 14, yPos);
    yPos += 5;

    const savingsRate = parseFloat(cashFlowReport.value.total_income) > 0
      ? ((parseFloat(cashFlowReport.value.net_cash_flow) / parseFloat(cashFlowReport.value.total_income)) * 100).toFixed(1)
      : '0';

    autoTable(doc, {
      startY: yPos,
      head: [['Metric', 'Amount']],
      body: [
        ['Total Income', formatCurrency(cashFlowReport.value.total_income)],
        ['Total Expenses', formatCurrency(cashFlowReport.value.total_expenses)],
        ['Net Cash Flow', formatCurrency(cashFlowReport.value.net_cash_flow)],
        ['Savings Rate', `${savingsRate}%`],
      ],
      theme: 'striped',
      headStyles: { fillColor: [30, 64, 175] },
      margin: { left: 14 },
    });

    yPos = (doc as any).lastAutoTable.finalY + 15;
  }

  // Spending by Category
  if (spendingByCategory.value.length > 0) {
    // Check if we need a new page
    if (yPos > 230) {
      doc.addPage();
      yPos = 20;
    }

    doc.setFontSize(14);
    doc.text('Spending by Category', 14, yPos);
    yPos += 5;

    autoTable(doc, {
      startY: yPos,
      head: [['Category', 'Amount', 'Percentage']],
      body: spendingByCategory.value.map(cat => [
        cat.category_name,
        formatCurrency(cat.total),
        `${cat.percentage.toFixed(1)}%`,
      ]),
      theme: 'striped',
      headStyles: { fillColor: [30, 64, 175] },
      margin: { left: 14 },
    });

    yPos = (doc as any).lastAutoTable.finalY + 15;
  }

  // Monthly Trends
  if (monthlyTrends.value.length > 0) {
    // Check if we need a new page
    if (yPos > 180) {
      doc.addPage();
      yPos = 20;
    }

    doc.setFontSize(14);
    doc.text('Monthly Trends (Last 12 Months)', 14, yPos);
    yPos += 5;

    autoTable(doc, {
      startY: yPos,
      head: [['Month', 'Income', 'Expenses', 'Net']],
      body: monthlyTrends.value.map(trend => [
        `${trend.month} ${trend.year}`,
        formatCurrency(trend.income),
        formatCurrency(trend.expenses),
        formatCurrency(trend.net),
      ]),
      theme: 'striped',
      headStyles: { fillColor: [30, 64, 175] },
      margin: { left: 14 },
    });
  }

  // Footer
  const pageCount = doc.getNumberOfPages();
  for (let i = 1; i <= pageCount; i++) {
    doc.setPage(i);
    doc.setFontSize(10);
    doc.setTextColor(150, 150, 150);
    doc.text(`Page ${i} of ${pageCount}`, 105, 290, { align: 'center' });
    doc.text('Generated by indiBudget', 14, 290);
  }

  // Save the PDF
  doc.save(`indibudget-report-${periodDates.value.start}-to-${periodDates.value.end}.pdf`);
}

async function fetchReports() {
  loading.value = true;
  await nextTick();
  try {
    const [spending, trends, cashFlow, lastYearCf, _accounts] = await Promise.all([
      api.getSpendingByCategory(periodDates.value.start, periodDates.value.end),
      api.getMonthlyTrends(12),
      api.getCashFlowReport(periodDates.value.start, periodDates.value.end),
      api.getCashFlowReport(lastYearPeriodDates.value.start, lastYearPeriodDates.value.end),
      accountsStore.fetchAccounts(),
    ]);
    spendingByCategory.value = spending;
    monthlyTrends.value = trends;
    cashFlowReport.value = cashFlow;
    lastYearCashFlow.value = lastYearCf;
  } catch (e) {
    console.error('Failed to fetch reports:', e);
  } finally {
    loading.value = false;
  }
}

watch(selectedPeriod, fetchReports);
onMounted(async () => {
  loadNetWorthHistory();
  await fetchReports();
  // Record a snapshot after fetching data
  recordNetWorthSnapshot();
});
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Reports</h1>
      <div class="flex items-center gap-4">
        <!-- Period selector -->
        <div class="flex gap-2">
          <button
            v-for="period in ['month', 'quarter', 'year', 'all']"
            :key="period"
            @click="selectedPeriod = period as 'month' | 'quarter' | 'year' | 'all'"
            :class="[
              'px-4 py-2 rounded-lg transition-colors capitalize',
              selectedPeriod === period
                ? 'bg-blue-600 text-white'
                : 'bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300 hover:bg-gray-300 dark:hover:bg-gray-600'
            ]"
          >
            {{ period === 'all' ? 'All Time' : period }}
          </button>
        </div>

        <!-- Export buttons -->
        <div class="flex gap-2 border-l border-gray-300 dark:border-gray-600 pl-4">
          <button
            @click="exportToCSV"
            class="flex items-center gap-2 px-3 py-2 text-sm font-medium text-gray-700 dark:text-gray-300 bg-white dark:bg-gray-700 border border-gray-300 dark:border-gray-600 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-600 transition-colors"
            title="Export to CSV"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
            CSV
          </button>
          <button
            @click="exportToPDF"
            class="flex items-center gap-2 px-3 py-2 text-sm font-medium text-white bg-red-600 hover:bg-red-700 rounded-lg transition-colors"
            title="Export to PDF"
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
            </svg>
            PDF
          </button>
        </div>
      </div>
    </div>

    <!-- Net Worth Card -->
    <div class="bg-gradient-to-r from-indigo-600 to-purple-600 rounded-lg shadow-lg p-6 mb-6 text-white">
      <div class="flex flex-col md:flex-row md:items-center md:justify-between">
        <div>
          <h2 class="text-lg opacity-90 mb-1">Net Worth</h2>
          <p class="text-4xl font-bold">{{ formatCurrency(netWorthBreakdown.netWorth) }}</p>
          <div v-if="netWorthChange" class="mt-2 flex gap-4 text-sm">
            <div v-if="netWorthChange.weekChange !== null" class="flex items-center gap-1">
              <svg
                :class="['w-4 h-4', netWorthChange.weekChange >= 0 ? 'text-green-300' : 'text-red-300']"
                fill="none" stroke="currentColor" viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  :d="netWorthChange.weekChange >= 0 ? 'M5 10l7-7m0 0l7 7m-7-7v18' : 'M19 14l-7 7m0 0l-7-7m7 7V3'"
                />
              </svg>
              <span class="opacity-90">
                {{ netWorthChange.weekChange >= 0 ? '+' : '' }}{{ formatCurrency(netWorthChange.weekChange) }} (7d)
              </span>
            </div>
            <div v-if="netWorthChange.monthChange !== null" class="flex items-center gap-1">
              <svg
                :class="['w-4 h-4', netWorthChange.monthChange >= 0 ? 'text-green-300' : 'text-red-300']"
                fill="none" stroke="currentColor" viewBox="0 0 24 24"
              >
                <path
                  stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                  :d="netWorthChange.monthChange >= 0 ? 'M5 10l7-7m0 0l7 7m-7-7v18' : 'M19 14l-7 7m0 0l-7-7m7 7V3'"
                />
              </svg>
              <span class="opacity-90">
                {{ netWorthChange.monthChange >= 0 ? '+' : '' }}{{ formatCurrency(netWorthChange.monthChange) }} (30d)
              </span>
            </div>
          </div>
        </div>
        <div class="flex gap-8 mt-4 md:mt-0">
          <div class="text-center">
            <p class="text-sm opacity-75">Assets</p>
            <p class="text-xl font-semibold">{{ formatCurrency(netWorthBreakdown.assets) }}</p>
          </div>
          <div class="text-center">
            <p class="text-sm opacity-75">Liabilities</p>
            <p class="text-xl font-semibold">{{ formatCurrency(netWorthBreakdown.liabilities) }}</p>
          </div>
        </div>
      </div>
    </div>

    <!-- Net Worth History Chart -->
    <div v-if="netWorthHistory.length > 1" class="bg-white dark:bg-gray-800 rounded-lg shadow p-6 mb-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Net Worth History</h2>
        <span class="text-sm text-gray-500 dark:text-gray-400">{{ netWorthHistory.length }} data points</span>
      </div>
      <div class="h-64">
        <Line :data="netWorthChartData" :options="barChartOptions" />
      </div>
    </div>

    <!-- Summary Cards with YoY Comparison -->
    <div v-if="cashFlowReport" class="grid grid-cols-1 md:grid-cols-3 gap-4 mb-6">
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Income</h3>
        <p class="text-2xl font-bold text-green-600 mt-2">
          {{ formatCurrency(cashFlowReport.total_income) }}
        </p>
        <div v-if="yoyComparison && yoyComparison.lastYearIncome > 0" class="mt-2 flex items-center gap-1">
          <svg
            :class="['w-4 h-4', yoyComparison.incomeChange >= 0 ? 'text-green-500' : 'text-red-500']"
            fill="none" stroke="currentColor" viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              :d="yoyComparison.incomeChange >= 0 ? 'M5 10l7-7m0 0l7 7m-7-7v18' : 'M19 14l-7 7m0 0l-7-7m7 7V3'"
            />
          </svg>
          <span :class="['text-sm', yoyComparison.incomeChange >= 0 ? 'text-green-600' : 'text-red-600']">
            {{ Math.abs(yoyComparison.incomeChange).toFixed(1) }}% vs last year
          </span>
        </div>
      </div>
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Expenses</h3>
        <p class="text-2xl font-bold text-red-600 mt-2">
          {{ formatCurrency(cashFlowReport.total_expenses) }}
        </p>
        <div v-if="yoyComparison && yoyComparison.lastYearExpenses > 0" class="mt-2 flex items-center gap-1">
          <svg
            :class="['w-4 h-4', yoyComparison.expenseChange <= 0 ? 'text-green-500' : 'text-red-500']"
            fill="none" stroke="currentColor" viewBox="0 0 24 24"
          >
            <path
              stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              :d="yoyComparison.expenseChange >= 0 ? 'M5 10l7-7m0 0l7 7m-7-7v18' : 'M19 14l-7 7m0 0l-7-7m7 7V3'"
            />
          </svg>
          <span :class="['text-sm', yoyComparison.expenseChange <= 0 ? 'text-green-600' : 'text-red-600']">
            {{ Math.abs(yoyComparison.expenseChange).toFixed(1) }}% vs last year
          </span>
        </div>
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
        <p class="text-sm text-gray-500 mt-2">
          {{ parseFloat(cashFlowReport.total_income) > 0
            ? ((parseFloat(cashFlowReport.net_cash_flow) / parseFloat(cashFlowReport.total_income)) * 100).toFixed(1)
            : 0 }}% savings rate
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

    <!-- Savings Rate Analytics Section -->
    <div v-if="savingsRateSummary" class="mt-6">
      <h2 class="text-xl font-bold text-gray-900 dark:text-white mb-4">Savings Rate Analytics</h2>

      <!-- Savings Rate Summary Cards -->
      <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-5">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Average Savings Rate</h3>
          <p :class="[
            'text-3xl font-bold mt-1',
            savingsRateSummary.avgRate >= 20 ? 'text-green-600' :
            savingsRateSummary.avgRate >= 10 ? 'text-yellow-600' :
            savingsRateSummary.avgRate >= 0 ? 'text-orange-600' : 'text-red-600'
          ]">
            {{ savingsRateSummary.avgRate.toFixed(1) }}%
          </p>
          <p class="text-xs text-gray-400 mt-1">
            {{ savingsRateSummary.avgRate >= 20 ? 'Excellent!' :
               savingsRateSummary.avgRate >= 15 ? 'Great progress!' :
               savingsRateSummary.avgRate >= 10 ? 'Good start' : 'Room to improve' }}
          </p>
        </div>
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-5">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Current Month Rate</h3>
          <p :class="[
            'text-3xl font-bold mt-1',
            savingsRateSummary.currentRate >= 0 ? 'text-green-600' : 'text-red-600'
          ]">
            {{ savingsRateSummary.currentRate.toFixed(1) }}%
          </p>
        </div>
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-5">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Total Saved (12mo)</h3>
          <p :class="[
            'text-3xl font-bold mt-1',
            savingsRateSummary.totalSaved >= 0 ? 'text-green-600' : 'text-red-600'
          ]">
            {{ formatCurrency(savingsRateSummary.totalSaved) }}
          </p>
        </div>
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-5">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400">Positive Months</h3>
          <p class="text-3xl font-bold mt-1 text-gray-900 dark:text-white">
            {{ savingsRateSummary.positiveSavingsMonths }}/{{ savingsRateSummary.totalMonths }}
          </p>
          <p class="text-xs text-gray-400 mt-1">
            {{ ((savingsRateSummary.positiveSavingsMonths / savingsRateSummary.totalMonths) * 100).toFixed(0) }}% success rate
          </p>
        </div>
      </div>

      <!-- Savings Rate Chart and Best/Worst Months -->
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <!-- Savings Rate Chart -->
        <div class="lg:col-span-2 bg-white dark:bg-gray-800 rounded-lg shadow">
          <div class="p-4 border-b border-gray-200 dark:border-gray-700">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Savings Rate Trend</h2>
            <p class="text-sm text-gray-500">Monthly savings as % of income</p>
          </div>
          <div class="p-4">
            <div class="h-64">
              <Bar :data="savingsRateChartData" :options="savingsRateChartOptions" />
            </div>
            <div class="flex justify-center gap-4 mt-4 text-xs">
              <div class="flex items-center gap-1">
                <div class="w-3 h-3 rounded" style="background-color: rgba(34, 197, 94, 0.5)"></div>
                <span class="text-gray-500">20%+ (Excellent)</span>
              </div>
              <div class="flex items-center gap-1">
                <div class="w-3 h-3 rounded" style="background-color: rgba(234, 179, 8, 0.5)"></div>
                <span class="text-gray-500">10-20% (Good)</span>
              </div>
              <div class="flex items-center gap-1">
                <div class="w-3 h-3 rounded" style="background-color: rgba(251, 146, 60, 0.5)"></div>
                <span class="text-gray-500">0-10% (Fair)</span>
              </div>
              <div class="flex items-center gap-1">
                <div class="w-3 h-3 rounded" style="background-color: rgba(239, 68, 68, 0.5)"></div>
                <span class="text-gray-500">Negative (Loss)</span>
              </div>
            </div>
          </div>
        </div>

        <!-- Best & Worst Months -->
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
          <div class="p-4 border-b border-gray-200 dark:border-gray-700">
            <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Performance Highlights</h2>
          </div>
          <div class="p-4 space-y-4">
            <div v-if="savingsRateSummary.bestMonth" class="bg-green-50 dark:bg-green-900/20 rounded-lg p-4">
              <p class="text-sm font-medium text-green-800 dark:text-green-400">Best Month</p>
              <p class="text-lg font-bold text-green-700 dark:text-green-300 mt-1">
                {{ savingsRateSummary.bestMonth.month }} {{ savingsRateSummary.bestMonth.year }}
              </p>
              <div class="flex justify-between mt-2 text-sm">
                <span class="text-green-600 dark:text-green-400">{{ savingsRateSummary.bestMonth.savingsRate.toFixed(1) }}% saved</span>
                <span class="text-green-600 dark:text-green-400">{{ formatCurrency(savingsRateSummary.bestMonth.net) }}</span>
              </div>
            </div>

            <div v-if="savingsRateSummary.worstMonth" class="bg-red-50 dark:bg-red-900/20 rounded-lg p-4">
              <p class="text-sm font-medium text-red-800 dark:text-red-400">Lowest Month</p>
              <p class="text-lg font-bold text-red-700 dark:text-red-300 mt-1">
                {{ savingsRateSummary.worstMonth.month }} {{ savingsRateSummary.worstMonth.year }}
              </p>
              <div class="flex justify-between mt-2 text-sm">
                <span class="text-red-600 dark:text-red-400">{{ savingsRateSummary.worstMonth.savingsRate.toFixed(1) }}% saved</span>
                <span class="text-red-600 dark:text-red-400">{{ formatCurrency(savingsRateSummary.worstMonth.net) }}</span>
              </div>
            </div>

            <!-- Savings Goal Progress -->
            <div class="bg-purple-50 dark:bg-purple-900/20 rounded-lg p-4">
              <p class="text-sm font-medium text-purple-800 dark:text-purple-400">Recommended Target</p>
              <p class="text-lg font-bold text-purple-700 dark:text-purple-300 mt-1">20% Savings Rate</p>
              <div class="mt-2">
                <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                  <div
                    class="bg-purple-600 h-2 rounded-full transition-all"
                    :style="{ width: `${Math.min(100, (savingsRateSummary.avgRate / 20) * 100)}%` }"
                  ></div>
                </div>
                <p class="text-xs text-purple-600 dark:text-purple-400 mt-1">
                  {{ ((savingsRateSummary.avgRate / 20) * 100).toFixed(0) }}% of target
                </p>
              </div>
            </div>
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
