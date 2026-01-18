<script setup lang="ts">
import { ref, onMounted, computed, watch } from 'vue';
import FullCalendar from '@fullcalendar/vue3';
import dayGridPlugin from '@fullcalendar/daygrid';
import interactionPlugin from '@fullcalendar/interaction';
import listPlugin from '@fullcalendar/list';
import { format, startOfMonth, endOfMonth, parseISO, isSameDay, isSameMonth } from 'date-fns';
import { useCalendarStore, useCategoriesStore } from '../stores';
import * as api from '../services/api';
import type { UpcomingRecurring, BillReminder, CalendarEvent } from '../types';

const calendarStore = useCalendarStore();
const categoriesStore = useCategoriesStore();

const loading = ref(false);
const currentMonth = ref(new Date());
const upcomingRecurring = ref<UpcomingRecurring[]>([]);
const billReminders = ref<BillReminder[]>([]);
const calendarRef = ref<InstanceType<typeof FullCalendar> | null>(null);

// Modal state
const showEventModal = ref(false);
const selectedDate = ref<string | null>(null);
const selectedEvents = ref<CalendarEvent[]>([]);

// View mode: 'transactions' shows actual transactions, 'bills' shows recurring bills
const viewMode = ref<'transactions' | 'bills'>('transactions');

// Track expanded dates for collapsed view
const expandedDates = ref<Set<string>>(new Set());
const displayMode = ref<'collapsed' | 'expanded'>('collapsed');

// Group events by date
const eventsByDate = computed(() => {
  const grouped: Record<string, CalendarEvent[]> = {};
  for (const event of calendarStore.events) {
    if (!grouped[event.date]) {
      grouped[event.date] = [];
    }
    grouped[event.date].push(event);
  }
  return grouped;
});

// Generate summary events for collapsed transaction view
const transactionEvents = computed(() => {
  const events: any[] = [];

  for (const [date, dayEvents] of Object.entries(eventsByDate.value)) {
    const isExpanded = expandedDates.value.has(date) || displayMode.value === 'expanded';

    if (isExpanded) {
      // Add collapse header when manually expanded (not in "Show All" mode)
      if (expandedDates.value.has(date)) {
        const expenses = dayEvents.filter(e => e.transaction_type === 'expense');
        const income = dayEvents.filter(e => e.transaction_type === 'income');
        const totalExpense = expenses.reduce((sum, e) => sum + (parseFloat(e.amount) || 0), 0);
        const totalIncome = income.reduce((sum, e) => sum + (parseFloat(e.amount) || 0), 0);
        const summaryParts = [];
        if (expenses.length > 0) summaryParts.push(`${expenses.length} bill${expenses.length > 1 ? 's' : ''}: $${totalExpense.toFixed(0)}`);
        if (income.length > 0) summaryParts.push(`${income.length} deposit${income.length > 1 ? 's' : ''}: $${totalIncome.toFixed(0)}`);

        events.push({
          id: `collapse-header-${date}`,
          title: `▼ ${summaryParts.join(' | ')}`,
          start: date,
          order: -1,
          backgroundColor: '#6366f1',
          borderColor: '#6366f1',
          extendedProps: {
            isCollapseHeader: true,
            summaryDate: date,
          },
        });
      }

      // Show individual events when expanded
      for (let i = 0; i < dayEvents.length; i++) {
        const event = dayEvents[i];
        events.push({
          id: event.id,
          title: `${event.transaction_type === 'income' ? '+' : '-'}$${parseFloat(event.amount).toFixed(0)} ${event.title}`,
          start: event.date,
          order: i + 1,
          backgroundColor: event.category_color || (event.transaction_type === 'income' ? '#22c55e' : '#ef4444'),
          borderColor: event.category_color || (event.transaction_type === 'income' ? '#22c55e' : '#ef4444'),
          extendedProps: {
            transaction_type: event.transaction_type,
            amount: event.amount,
            category_name: event.category_name,
            is_recurring: event.is_recurring,
            account_name: event.account_name,
            isExpanded: true,
            isSummary: false,
          },
        });
      }
    } else {
      // Show collapsed summary
      const expenses = dayEvents.filter(e => e.transaction_type === 'expense');
      const income = dayEvents.filter(e => e.transaction_type === 'income');

      if (expenses.length > 0) {
        const totalExpense = expenses.reduce((sum, e) => sum + (parseFloat(e.amount) || 0), 0);
        const expenseLabel = expenses.length === 1 ? `1 bill` : `${expenses.length} bills`;

        events.push({
          id: `summary-expense-${date}`,
          title: `${expenseLabel}: $${totalExpense.toFixed(0)}`,
          start: date,
          order: 0,
          backgroundColor: '#ef4444',
          borderColor: '#ef4444',
          extendedProps: {
            transaction_type: 'expense',
            isSummary: true,
            summaryDate: date,
            count: expenses.length,
            total: totalExpense,
          },
        });
      }

      if (income.length > 0) {
        const totalIncome = income.reduce((sum, e) => sum + (parseFloat(e.amount) || 0), 0);
        const incomeLabel = income.length === 1 ? `1 deposit` : `${income.length} deposits`;

        events.push({
          id: `summary-income-${date}`,
          title: `${incomeLabel}: $${totalIncome.toFixed(0)}`,
          start: date,
          order: 1,
          backgroundColor: '#22c55e',
          borderColor: '#22c55e',
          extendedProps: {
            transaction_type: 'income',
            isSummary: true,
            summaryDate: date,
            count: income.length,
            total: totalIncome,
          },
        });
      }
    }
  }

  return events;
});

// Generate events from recurring bills
const billEvents = computed(() => {
  return upcomingRecurring.value.map(item => ({
    id: `bill-${item.recurring.id}-${item.next_date}`,
    title: item.recurring.description,
    start: item.next_date,
    backgroundColor: item.recurring.transaction_type === 'expense' ? '#ef4444' : '#22c55e',
    borderColor: item.recurring.transaction_type === 'expense' ? '#ef4444' : '#22c55e',
    extendedProps: {
      transaction_type: item.recurring.transaction_type,
      amount: item.recurring.amount,
      account_name: item.account_name,
      category_name: item.category_name,
      is_recurring: true,
      isBill: true,
    },
  }));
});

// Combined events based on view mode
const calendarEvents = computed(() => {
  return viewMode.value === 'transactions' ? transactionEvents.value : billEvents.value;
});

const calendarOptions = ref({
  plugins: [dayGridPlugin, interactionPlugin, listPlugin],
  initialView: 'dayGridMonth',
  headerToolbar: {
    left: 'prev,next today',
    center: 'title',
    right: 'dayGridMonth,listMonth',
  },
  events: [] as any[],
  dateClick: handleDateClick,
  eventClick: handleEventClick,
  datesSet: handleDatesSet,
  eventContent: renderEventContent,
  eventClassNames: (arg: any) => {
    const type = arg.event.extendedProps.transaction_type;
    const isSummary = arg.event.extendedProps.isSummary;
    const isCollapseHeader = arg.event.extendedProps.isCollapseHeader;
    const classes: string[] = [];
    if (isCollapseHeader) {
      classes.push('event-collapse-header');
    } else {
      classes.push(type === 'income' ? 'event-income' : 'event-expense');
    }
    if (isSummary || isCollapseHeader) classes.push('event-summary');
    return classes;
  },
});

function renderEventContent(arg: any) {
  const isSummary = arg.event.extendedProps.isSummary;
  const isCollapseHeader = arg.event.extendedProps.isCollapseHeader;
  const isBill = arg.event.extendedProps.isBill;

  if (isCollapseHeader) {
    return {
      html: `<span class="event-title collapse-header-title">${arg.event.title}</span>`
    };
  }

  if (isSummary) {
    return {
      html: `
        <div class="event-content-wrapper">
          <span class="expand-triangle">▶</span>
          <span class="event-title">${arg.event.title}</span>
        </div>
      `
    };
  }

  if (isBill) {
    const amount = parseFloat(arg.event.extendedProps.amount);
    const type = arg.event.extendedProps.transaction_type;
    return {
      html: `<span class="event-title">${type === 'income' ? '+' : '-'}$${amount.toFixed(0)} ${arg.event.title}</span>`
    };
  }

  return {
    html: `<span class="event-title">${arg.event.title}</span>`
  };
}

function toggleDateExpansion(date: string) {
  if (expandedDates.value.has(date)) {
    expandedDates.value.delete(date);
  } else {
    expandedDates.value.add(date);
  }
  expandedDates.value = new Set(expandedDates.value);
  updateCalendarEvents();
}

function handleDateClick(info: any) {
  selectedDate.value = info.dateStr;
  if (viewMode.value === 'transactions') {
    selectedEvents.value = calendarStore.getEventsForDate(info.dateStr);
  } else {
    // For bills view, show bills for that date
    const dayBills = upcomingRecurring.value.filter(item =>
      isSameDay(parseISO(item.next_date), parseISO(info.dateStr))
    );
    selectedEvents.value = dayBills.map(item => ({
      id: item.recurring.id,
      title: item.recurring.description,
      date: item.next_date,
      amount: item.recurring.amount,
      transaction_type: item.recurring.transaction_type,
      category_name: item.category_name,
      account_name: item.account_name,
      is_recurring: true,
    } as CalendarEvent));
  }
  showEventModal.value = selectedEvents.value.length > 0;
}

function handleEventClick(info: any) {
  const isSummary = info.event.extendedProps.isSummary;
  const isCollapseHeader = info.event.extendedProps.isCollapseHeader;
  const summaryDate = info.event.extendedProps.summaryDate;

  if ((isSummary || isCollapseHeader) && summaryDate) {
    toggleDateExpansion(summaryDate);
    return;
  }

  // For individual events, show the modal
  selectedDate.value = info.event.startStr;
  if (viewMode.value === 'transactions') {
    selectedEvents.value = calendarStore.getEventsForDate(info.event.startStr);
  } else {
    const dayBills = upcomingRecurring.value.filter(item =>
      isSameDay(parseISO(item.next_date), parseISO(info.event.startStr))
    );
    selectedEvents.value = dayBills.map(item => ({
      id: item.recurring.id,
      title: item.recurring.description,
      date: item.next_date,
      amount: item.recurring.amount,
      transaction_type: item.recurring.transaction_type,
      category_name: item.category_name,
      account_name: item.account_name,
      is_recurring: true,
    } as CalendarEvent));
  }
  showEventModal.value = true;
}

async function handleDatesSet(info: any) {
  const start = format(info.start, 'yyyy-MM-dd');
  const end = format(info.end, 'yyyy-MM-dd');
  currentMonth.value = info.view.currentStart;
  await Promise.all([
    calendarStore.fetchCalendarEvents(start, end),
    fetchBillData(),
  ]);
  updateCalendarEvents();
}

function updateCalendarEvents() {
  calendarOptions.value.events = calendarEvents.value;
}

function toggleDisplayMode() {
  if (displayMode.value === 'collapsed') {
    displayMode.value = 'expanded';
  } else {
    displayMode.value = 'collapsed';
    expandedDates.value.clear();
  }
  updateCalendarEvents();
}

function collapseAll() {
  expandedDates.value.clear();
  updateCalendarEvents();
}

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

const goToToday = () => {
  currentMonth.value = new Date();
  const calendarApi = calendarRef.value?.getApi();
  calendarApi?.today();
};

async function fetchBillData() {
  try {
    const [recurring, reminders] = await Promise.all([
      api.getUpcomingRecurring(90),
      api.getBillReminders(30),
    ]);
    upcomingRecurring.value = recurring;
    billReminders.value = reminders;
  } catch (e) {
    console.error('Failed to fetch bill data:', e);
  }
}

async function fetchData() {
  loading.value = true;
  try {
    await categoriesStore.fetchCategories();
    const today = new Date();
    const start = format(startOfMonth(today), 'yyyy-MM-dd');
    const end = format(endOfMonth(today), 'yyyy-MM-dd');
    await Promise.all([
      calendarStore.fetchCalendarEvents(start, end),
      fetchBillData(),
    ]);
    updateCalendarEvents();
  } catch (e) {
    console.error('Failed to fetch data:', e);
  } finally {
    loading.value = false;
  }
}

watch([viewMode, expandedDates, displayMode], updateCalendarEvents, { deep: true });
watch(() => calendarStore.events, updateCalendarEvents);

onMounted(fetchData);
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Calendar</h1>
      <div class="flex items-center gap-3">
        <!-- View Mode Toggle -->
        <div class="flex bg-gray-100 dark:bg-gray-700 rounded-lg p-1">
          <button
            @click="viewMode = 'transactions'"
            :class="[
              'px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
              viewMode === 'transactions'
                ? 'bg-white dark:bg-gray-600 text-gray-900 dark:text-white shadow'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
            ]"
          >
            Transactions
          </button>
          <button
            @click="viewMode = 'bills'"
            :class="[
              'px-3 py-1.5 text-sm font-medium rounded-md transition-colors',
              viewMode === 'bills'
                ? 'bg-white dark:bg-gray-600 text-gray-900 dark:text-white shadow'
                : 'text-gray-600 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white'
            ]"
          >
            Upcoming Bills
          </button>
        </div>

        <button
          v-if="viewMode === 'transactions' && expandedDates.size > 0"
          @click="collapseAll"
          class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          Collapse All
        </button>
        <button
          v-if="viewMode === 'transactions'"
          @click="toggleDisplayMode"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
        >
          <svg v-if="displayMode === 'collapsed'" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
          </svg>
          <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
          </svg>
          {{ displayMode === 'collapsed' ? 'Show All' : 'Show Summaries' }}
        </button>
        <button
          @click="goToToday"
          class="px-4 py-2 border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          Today
        </button>
      </div>
    </div>

    <!-- Tip for transaction view -->
    <div v-if="viewMode === 'transactions'" class="bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800 rounded-lg p-3 mb-4">
      <div class="flex items-center gap-2 text-sm text-blue-800 dark:text-blue-200">
        <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <span>
          <strong>Tip:</strong> Click <span class="font-mono">▶</span> to expand and see individual transactions, or <span class="font-mono">▼</span> to collapse. Click a date to see details.
        </span>
      </div>
    </div>

    <!-- Summary Cards (Bills view) -->
    <div v-if="viewMode === 'bills'" class="grid grid-cols-1 md:grid-cols-4 gap-4 mb-6">
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
      <div :class="viewMode === 'bills' ? 'lg:col-span-2' : 'lg:col-span-3'">
        <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
          <FullCalendar ref="calendarRef" :options="calendarOptions" />
        </div>
      </div>

      <!-- Upcoming Bills Sidebar (only in bills view) -->
      <div v-if="viewMode === 'bills'" class="bg-white dark:bg-gray-800 rounded-lg shadow">
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

    <!-- Event Details Modal -->
    <div
      v-if="showEventModal"
      class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
      @click.self="showEventModal = false"
    >
      <div class="bg-white dark:bg-gray-800 rounded-lg shadow-xl max-w-md w-full mx-4">
        <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex justify-between items-center">
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">
            {{ selectedDate }}
          </h3>
          <button
            @click="showEventModal = false"
            class="text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          >
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
        <div class="p-4 max-h-96 overflow-y-auto">
          <div v-if="selectedEvents.length === 0" class="text-center text-gray-500 py-4">
            No transactions on this date
          </div>
          <div v-else class="space-y-3">
            <div
              v-for="event in selectedEvents"
              :key="event.id"
              class="p-3 bg-gray-50 dark:bg-gray-700 rounded-lg"
            >
              <div class="flex items-center justify-between">
                <div>
                  <p class="font-medium text-gray-900 dark:text-white">{{ event.title }}</p>
                  <p class="text-sm text-gray-500 dark:text-gray-400">
                    {{ event.category_name || 'Uncategorized' }}
                    <span v-if="event.is_recurring" class="ml-2 text-blue-500">(Recurring)</span>
                  </p>
                  <p class="text-xs text-gray-400">{{ event.account_name }}</p>
                </div>
                <p
                  :class="[
                    'font-bold text-lg',
                    event.transaction_type === 'income' ? 'text-green-600' : 'text-red-600'
                  ]"
                >
                  {{ event.transaction_type === 'income' ? '+' : '-' }}{{ formatCurrency(event.amount) }}
                </p>
              </div>
            </div>
          </div>
        </div>
        <div class="p-4 border-t border-gray-200 dark:border-gray-700">
          <div class="flex justify-between text-sm">
            <span class="text-gray-600 dark:text-gray-400">Daily Total:</span>
            <span class="font-semibold text-gray-900 dark:text-white">
              {{
                formatCurrency(
                  selectedEvents.reduce((sum, e) => {
                    const amt = parseFloat(e.amount) || 0;
                    return e.transaction_type === 'income' ? sum + amt : sum - amt;
                  }, 0)
                )
              }}
            </span>
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
        <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Loading Calendar...</h3>
        <p class="text-sm text-gray-500 dark:text-gray-400">
          Fetching your transactions and bills.
        </p>
      </div>
    </div>
  </div>
</template>

<style>
.event-income {
  background-color: #22c55e !important;
  border-color: #22c55e !important;
}

.event-expense {
  background-color: #ef4444 !important;
  border-color: #ef4444 !important;
}

.event-collapse-header {
  background-color: #6366f1 !important;
  border-color: #6366f1 !important;
}

.collapse-header-title {
  font-weight: 600;
}

.event-summary {
  cursor: pointer;
}

.event-summary:hover {
  opacity: 0.9;
}

.event-content-wrapper {
  display: flex;
  align-items: center;
  gap: 4px;
  width: 100%;
  overflow: hidden;
}

.expand-triangle {
  font-size: 8px;
  flex-shrink: 0;
  transition: transform 0.2s ease;
}

.event-title {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.fc {
  --fc-border-color: #e5e7eb;
  --fc-button-bg-color: #3b82f6;
  --fc-button-border-color: #3b82f6;
  --fc-button-hover-bg-color: #2563eb;
  --fc-button-hover-border-color: #2563eb;
  --fc-button-active-bg-color: #1d4ed8;
  --fc-button-active-border-color: #1d4ed8;
  --fc-today-bg-color: #dbeafe;
}

.dark .fc {
  --fc-border-color: #374151;
  --fc-page-bg-color: #1f2937;
  --fc-neutral-bg-color: #374151;
  --fc-today-bg-color: #1e3a5f;
}

.fc .fc-daygrid-day-number {
  color: inherit;
}

.fc-event {
  font-size: 0.75rem;
  padding: 2px 4px;
  cursor: pointer;
}

.fc-event-title {
  font-weight: 500;
}
</style>
