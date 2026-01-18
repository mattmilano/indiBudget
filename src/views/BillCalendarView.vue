<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { format, startOfMonth, endOfMonth, eachDayOfInterval, isSameMonth, isToday, addMonths, subMonths, parseISO, isSameDay } from 'date-fns';
import * as api from '../services/api';
import type { UpcomingRecurring, BillReminder } from '../types';

const loading = ref(false);
const currentMonth = ref(new Date());
const upcomingRecurring = ref<UpcomingRecurring[]>([]);
const billReminders = ref<BillReminder[]>([]);

// Calendar days
const calendarDays = computed(() => {
  const start = startOfMonth(currentMonth.value);
  const end = endOfMonth(currentMonth.value);

  // Get days in month
  const days = eachDayOfInterval({ start, end });

  // Pad with days from previous month to start on Sunday
  const startPadding = start.getDay();
  const previousMonthDays: Date[] = [];
  for (let i = startPadding; i > 0; i--) {
    const date = new Date(start);
    date.setDate(date.getDate() - i);
    previousMonthDays.push(date);
  }

  // Pad with days from next month to complete the grid
  const totalDays = previousMonthDays.length + days.length;
  const endPadding = totalDays % 7 === 0 ? 0 : 7 - (totalDays % 7);
  const nextMonthDays: Date[] = [];
  for (let i = 1; i <= endPadding; i++) {
    const date = new Date(end);
    date.setDate(date.getDate() + i);
    nextMonthDays.push(date);
  }

  return [...previousMonthDays, ...days, ...nextMonthDays];
});

// Get bills for a specific date
const getBillsForDate = (date: Date) => {
  return upcomingRecurring.value.filter(item => {
    const billDate = parseISO(item.next_date);
    return isSameDay(billDate, date);
  });
};

// Summary stats
const monthSummary = computed(() => {
  const monthBills = upcomingRecurring.value.filter(item => {
    const billDate = parseISO(item.next_date);
    return isSameMonth(billDate, currentMonth.value);
  });

  const totalAmount = monthBills.reduce((sum, item) => sum + (parseFloat(item.recurring.amount) || 0), 0);
  const expenseBills = monthBills.filter(b => b.recurring.transaction_type === 'expense');
  const incomeBills = monthBills.filter(b => b.recurring.transaction_type === 'income');

  return {
    totalBills: monthBills.length,
    totalAmount,
    expenseAmount: expenseBills.reduce((sum, b) => sum + (parseFloat(b.recurring.amount) || 0), 0),
    incomeAmount: incomeBills.reduce((sum, b) => sum + (parseFloat(b.recurring.amount) || 0), 0),
  };
});

// Upcoming bills list (next 30 days)
const upcomingBillsList = computed(() => {
  return upcomingRecurring.value
    .filter(item => item.days_until <= 30 && item.days_until >= 0)
    .sort((a, b) => a.days_until - b.days_until);
});

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

const previousMonth = () => {
  currentMonth.value = subMonths(currentMonth.value, 1);
};

const nextMonth = () => {
  currentMonth.value = addMonths(currentMonth.value, 1);
};

const goToToday = () => {
  currentMonth.value = new Date();
};

async function fetchData() {
  loading.value = true;
  try {
    const [recurring, reminders] = await Promise.all([
      api.getUpcomingRecurring(90), // Get 90 days out for calendar
      api.getBillReminders(30),
    ]);
    upcomingRecurring.value = recurring;
    billReminders.value = reminders;
  } catch (e) {
    console.error('Failed to fetch bill data:', e);
  } finally {
    loading.value = false;
  }
}

onMounted(fetchData);
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Bill Calendar</h1>
      <button
        @click="goToToday"
        class="px-4 py-2 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
      >
        Today
      </button>
    </div>

    <!-- Summary Cards -->
    <div class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
      <div class="bg-gradient-to-r from-blue-600 to-blue-700 rounded-lg shadow-lg p-5 text-white">
        <h3 class="text-sm font-medium opacity-90">Bills This Month</h3>
        <p class="text-3xl font-bold mt-1">{{ monthSummary.totalBills }}</p>
      </div>
      <div class="bg-gradient-to-r from-red-600 to-red-700 rounded-lg shadow-lg p-5 text-white">
        <h3 class="text-sm font-medium opacity-90">Total Expenses</h3>
        <p class="text-3xl font-bold mt-1">{{ formatCurrency(monthSummary.expenseAmount) }}</p>
      </div>
      <div class="bg-gradient-to-r from-green-600 to-green-700 rounded-lg shadow-lg p-5 text-white">
        <h3 class="text-sm font-medium opacity-90">Expected Income</h3>
        <p class="text-3xl font-bold mt-1">{{ formatCurrency(monthSummary.incomeAmount) }}</p>
      </div>
      <div class="bg-gradient-to-r from-purple-600 to-purple-700 rounded-lg shadow-lg p-5 text-white">
        <h3 class="text-sm font-medium opacity-90">Net Expected</h3>
        <p class="text-3xl font-bold mt-1">{{ formatCurrency(monthSummary.incomeAmount - monthSummary.expenseAmount) }}</p>
      </div>
    </div>

    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <!-- Calendar -->
      <div class="lg:col-span-2 bg-white dark:bg-gray-800 rounded-lg shadow">
        <!-- Calendar Header -->
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <button
            @click="previousMonth"
            class="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
          >
            <svg class="w-5 h-5 text-gray-600 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">
            {{ format(currentMonth, 'MMMM yyyy') }}
          </h2>
          <button
            @click="nextMonth"
            class="p-2 hover:bg-gray-100 dark:hover:bg-gray-700 rounded-lg transition-colors"
          >
            <svg class="w-5 h-5 text-gray-600 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
            </svg>
          </button>
        </div>

        <!-- Day Headers -->
        <div class="grid grid-cols-7 border-b border-gray-200 dark:border-gray-700">
          <div v-for="day in ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']" :key="day"
            class="py-2 text-center text-sm font-medium text-gray-500 dark:text-gray-400"
          >
            {{ day }}
          </div>
        </div>

        <!-- Calendar Grid -->
        <div class="grid grid-cols-7">
          <div
            v-for="(day, index) in calendarDays"
            :key="index"
            :class="[
              'min-h-24 p-1 border-b border-r border-gray-100 dark:border-gray-700',
              !isSameMonth(day, currentMonth) ? 'bg-gray-50 dark:bg-gray-800/50' : '',
              isToday(day) ? 'bg-blue-50 dark:bg-blue-900/20' : ''
            ]"
          >
            <div class="flex items-center justify-between mb-1">
              <span
                :class="[
                  'text-sm font-medium w-6 h-6 flex items-center justify-center rounded-full',
                  isToday(day) ? 'bg-blue-600 text-white' : '',
                  !isSameMonth(day, currentMonth) ? 'text-gray-400' : 'text-gray-700 dark:text-gray-300'
                ]"
              >
                {{ format(day, 'd') }}
              </span>
            </div>
            <div class="space-y-1">
              <div
                v-for="bill in getBillsForDate(day).slice(0, 3)"
                :key="bill.recurring.id"
                :class="[
                  'text-xs px-1.5 py-0.5 rounded truncate',
                  bill.recurring.transaction_type === 'expense'
                    ? 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400'
                    : 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400'
                ]"
                :title="`${bill.recurring.description}: ${formatCurrency(bill.recurring.amount)}`"
              >
                {{ bill.recurring.description }}
              </div>
              <div
                v-if="getBillsForDate(day).length > 3"
                class="text-xs text-gray-500 dark:text-gray-400 px-1"
              >
                +{{ getBillsForDate(day).length - 3 }} more
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Upcoming Bills Sidebar -->
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Upcoming Bills</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400">Next 30 days</p>
        </div>
        <div class="p-4 max-h-96 overflow-y-auto">
          <div v-if="loading" class="text-center py-8 text-gray-500">Loading...</div>
          <div v-else-if="upcomingBillsList.length === 0" class="text-center py-8">
            <svg class="w-12 h-12 mx-auto text-gray-300 dark:text-gray-600 mb-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            <p class="text-gray-500 dark:text-gray-400">No upcoming bills</p>
          </div>
          <div v-else class="space-y-3">
            <div
              v-for="bill in upcomingBillsList"
              :key="bill.recurring.id"
              :class="[
                'p-3 rounded-lg border-l-4',
                bill.recurring.transaction_type === 'expense'
                  ? 'border-l-red-500 bg-red-50 dark:bg-red-900/10'
                  : 'border-l-green-500 bg-green-50 dark:bg-green-900/10'
              ]"
            >
              <div class="flex items-center justify-between">
                <div class="flex-1 min-w-0">
                  <p class="font-medium text-gray-900 dark:text-white truncate">
                    {{ bill.recurring.description }}
                  </p>
                  <p class="text-sm text-gray-500 dark:text-gray-400">
                    {{ bill.account_name }}
                    <span v-if="bill.category_name"> &middot; {{ bill.category_name }}</span>
                  </p>
                </div>
                <div class="text-right ml-4">
                  <p :class="[
                    'font-semibold',
                    bill.recurring.transaction_type === 'expense' ? 'text-red-600' : 'text-green-600'
                  ]">
                    {{ bill.recurring.transaction_type === 'expense' ? '-' : '+' }}{{ formatCurrency(bill.recurring.amount) }}
                  </p>
                  <p class="text-sm text-gray-500 dark:text-gray-400">
                    <span v-if="bill.days_until === 0" class="text-orange-600 font-medium">Today</span>
                    <span v-else-if="bill.days_until === 1">Tomorrow</span>
                    <span v-else>{{ bill.days_until }} days</span>
                  </p>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Legend -->
        <div class="p-4 border-t border-gray-200 dark:border-gray-700">
          <div class="flex items-center gap-4 text-sm">
            <div class="flex items-center gap-2">
              <div class="w-3 h-3 rounded bg-red-500"></div>
              <span class="text-gray-600 dark:text-gray-400">Expense</span>
            </div>
            <div class="flex items-center gap-2">
              <div class="w-3 h-3 rounded bg-green-500"></div>
              <span class="text-gray-600 dark:text-gray-400">Income</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
