<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
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
  eventClassNames: (arg: any) => {
    const type = arg.event.extendedProps.transaction_type;
    return type === 'income' ? ['event-income'] : ['event-expense'];
  },
});

function handleDateClick(info: any) {
  selectedDate.value = info.dateStr;
  selectedEvents.value = calendarStore.getEventsForDate(info.dateStr);
  showEventModal.value = selectedEvents.value.length > 0;
}

function handleEventClick(info: any) {
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
  calendarOptions.value.events = calendarStore.events.map(event => ({
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
    },
  }));
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
</script>

<template>
  <div class="p-6">
    <h1 class="text-2xl font-bold text-gray-900 dark:text-white mb-6">Calendar</h1>

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
