<script setup lang="ts">
import { ref, onMounted, watch, computed } from 'vue';
import FullCalendar from '@fullcalendar/vue3';
import dayGridPlugin from '@fullcalendar/daygrid';
import interactionPlugin from '@fullcalendar/interaction';
import listPlugin from '@fullcalendar/list';
import { useCalendarStore, useCategoriesStore } from '../stores';
import { format, startOfMonth, endOfMonth } from 'date-fns';
import type { CalendarEvent } from '../types';

const calendarStore = useCalendarStore();
const categoriesStore = useCategoriesStore();

const calendarRef = ref<InstanceType<typeof FullCalendar> | null>(null);
const selectedDate = ref<string | null>(null);
const selectedEvents = ref<CalendarEvent[]>([]);
const showEventModal = ref(false);

// Track expanded dates for collapsed view
const expandedDates = ref<Set<string>>(new Set());
const viewMode = ref<'collapsed' | 'expanded'>('collapsed');

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

// Generate summary events for collapsed view
const summaryEvents = computed(() => {
  const events: any[] = [];

  for (const [date, dayEvents] of Object.entries(eventsByDate.value)) {
    const isExpanded = expandedDates.value.has(date) || viewMode.value === 'expanded';

    if (isExpanded) {
      // Show individual events when expanded
      for (const event of dayEvents) {
        events.push({
          id: event.id,
          title: `${event.transaction_type === 'income' ? '+' : '-'}$${parseFloat(event.amount).toFixed(0)} ${event.title}`,
          start: event.date,
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

      // Summary for expenses
      if (expenses.length > 0) {
        const totalExpense = expenses.reduce((sum, e) => sum + (parseFloat(e.amount) || 0), 0);
        const expenseLabel = expenses.length === 1
          ? `1 bill`
          : `${expenses.length} bills`;

        events.push({
          id: `summary-expense-${date}`,
          title: `${expenseLabel}: $${totalExpense.toFixed(0)}`,
          start: date,
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

      // Summary for income
      if (income.length > 0) {
        const totalIncome = income.reduce((sum, e) => sum + (parseFloat(e.amount) || 0), 0);
        const incomeLabel = income.length === 1
          ? `1 deposit`
          : `${income.length} deposits`;

        events.push({
          id: `summary-income-${date}`,
          title: `${incomeLabel}: $${totalIncome.toFixed(0)}`,
          start: date,
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
    const classes = [type === 'income' ? 'event-income' : 'event-expense'];
    if (isSummary) classes.push('event-summary');
    return classes;
  },
});

function renderEventContent(arg: any) {
  const isSummary = arg.event.extendedProps.isSummary;
  const date = arg.event.extendedProps.summaryDate || arg.event.startStr;
  const isExpanded = expandedDates.value.has(date);

  if (isSummary) {
    // Render summary with expand triangle
    return {
      html: `
        <div class="event-content-wrapper">
          <span class="expand-triangle ${isExpanded ? 'expanded' : ''}">${isExpanded ? '▼' : '▶'}</span>
          <span class="event-title">${arg.event.title}</span>
        </div>
      `
    };
  }

  // Regular event (expanded view)
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
  // Force reactivity update
  expandedDates.value = new Set(expandedDates.value);
  updateCalendarEvents();
}

function handleDateClick(info: any) {
  selectedDate.value = info.dateStr;
  selectedEvents.value = calendarStore.getEventsForDate(info.dateStr);
  showEventModal.value = selectedEvents.value.length > 0;
}

function handleEventClick(info: any) {
  const isSummary = info.event.extendedProps.isSummary;
  const summaryDate = info.event.extendedProps.summaryDate;

  if (isSummary && summaryDate) {
    // Toggle expansion for this date
    toggleDateExpansion(summaryDate);
    return;
  }

  // For individual events, show the modal
  selectedDate.value = info.event.startStr;
  selectedEvents.value = calendarStore.getEventsForDate(info.event.startStr);
  showEventModal.value = true;
}

async function handleDatesSet(info: any) {
  const start = format(info.start, 'yyyy-MM-dd');
  const end = format(info.end, 'yyyy-MM-dd');
  await calendarStore.fetchCalendarEvents(start, end);
  updateCalendarEvents();
}

function updateCalendarEvents() {
  calendarOptions.value.events = summaryEvents.value;
}

function toggleViewMode() {
  if (viewMode.value === 'collapsed') {
    viewMode.value = 'expanded';
  } else {
    viewMode.value = 'collapsed';
    expandedDates.value.clear();
  }
  updateCalendarEvents();
}

function collapseAll() {
  expandedDates.value.clear();
  updateCalendarEvents();
}

const formatCurrency = (value: string | number) => {
  const num = typeof value === 'string' ? parseFloat(value) : value;
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(num || 0);
};

onMounted(async () => {
  await categoriesStore.fetchCategories();
  const today = new Date();
  const start = format(startOfMonth(today), 'yyyy-MM-dd');
  const end = format(endOfMonth(today), 'yyyy-MM-dd');
  await calendarStore.fetchCalendarEvents(start, end);
  updateCalendarEvents();
});

watch(() => calendarStore.events, updateCalendarEvents);
watch([expandedDates, viewMode], updateCalendarEvents, { deep: true });
</script>

<template>
  <div class="p-6">
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Calendar</h1>
      <div class="flex items-center gap-3">
        <button
          v-if="expandedDates.size > 0"
          @click="collapseAll"
          class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 rounded-lg hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
        >
          Collapse All
        </button>
        <button
          @click="toggleViewMode"
          class="px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors flex items-center gap-2"
        >
          <svg v-if="viewMode === 'collapsed'" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
          </svg>
          <svg v-else class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 9h4.5M15 9V4.5M15 9l5.25-5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
          </svg>
          {{ viewMode === 'collapsed' ? 'Show All' : 'Show Summaries' }}
        </button>
      </div>
    </div>

    <div class="bg-blue-50 dark:bg-blue-900/30 border border-blue-200 dark:border-blue-800 rounded-lg p-3 mb-4">
      <div class="flex items-center gap-2 text-sm text-blue-800 dark:text-blue-200">
        <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <span>
          <strong>Tip:</strong> Click the <span class="font-mono">▶</span> triangle on a summary to expand and see individual transactions. Click a date to see details.
        </span>
      </div>
    </div>

    <div class="bg-white dark:bg-gray-800 rounded-lg shadow p-4">
      <FullCalendar ref="calendarRef" :options="calendarOptions" />
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

.expand-triangle.expanded {
  transform: rotate(0deg);
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
